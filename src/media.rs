use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    CurrentSessionChangedEventArgs, GlobalSystemMediaTransportControlsSession,
    GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus, MediaPropertiesChangedEventArgs,
    PlaybackInfoChangedEventArgs, TimelinePropertiesChangedEventArgs,
};
use windows::Storage::Streams::{DataReader, IInputStream};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICBitmapSource, IWICImagingFactory,
    WICBitmapDitherTypeNone, WICBitmapInterpolationModeFant, WICBitmapPaletteTypeMedianCut,
    WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::Interface;

use crate::state::{MediaDisplayState, TimelineState, current_windows_filetime_ticks};

pub const WM_APP_MEDIA_UPDATE: u32 = WM_APP + 3;

const MAX_THUMBNAIL_BYTES: u64 = 8 * 1024 * 1024;
const TARGET_ARTWORK_PX: u32 = 96;

#[derive(Debug, Clone)]
pub struct DecodedArtwork {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum ArtworkUpdate {
    Unchanged,
    Clear,
    Set(DecodedArtwork),
}

#[derive(Debug, Clone)]
pub struct MediaUpdate {
    pub display: MediaDisplayState,
    pub artwork: ArtworkUpdate,
}

#[derive(Debug, Clone, Copy)]
pub enum MediaCommand {
    Refresh { check_artwork: bool },
    TogglePlayPause,
    Next,
    Previous,
    Seek(i64),
    Stop,
}

pub struct MediaWorker {
    cmd_tx: Sender<MediaCommand>,
    update_rx: Receiver<MediaUpdate>,
    handle: Option<JoinHandle<()>>,
}

impl MediaWorker {
    pub fn start(hwnd: HWND) -> Self {
        let hwnd_raw = hwnd.0 as isize;
        let (cmd_tx, cmd_rx) = mpsc::channel::<MediaCommand>();
        let (update_tx, update_rx) = mpsc::channel::<MediaUpdate>();
        let refresh_tx = cmd_tx.clone();

        let handle = thread::Builder::new()
            .name(String::from("gsmtc-media-worker"))
            .spawn(move || {
                media_worker_loop(hwnd_raw, cmd_rx, refresh_tx, update_tx);
            })
            .ok();

        Self {
            cmd_tx,
            update_rx,
            handle,
        }
    }

    pub fn send_command(&self, cmd: MediaCommand) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn drain_latest_update(&self) -> Option<MediaUpdate> {
        let mut latest: Option<MediaUpdate> = None;
        while let Ok(item) = self.update_rx.try_recv() {
            match (&mut latest, item) {
                (Some(prev), next) => {
                    prev.display = next.display;
                    match next.artwork {
                        ArtworkUpdate::Unchanged => {}
                        other => prev.artwork = other,
                    }
                }
                (None, next) => latest = Some(next),
            }
        }
        latest
    }
}

impl Drop for MediaWorker {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(MediaCommand::Stop);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

struct WinRtGuard(bool);

impl WinRtGuard {
    fn init() -> Self {
        let ok = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok();
        Self(ok)
    }
}

impl Drop for WinRtGuard {
    fn drop(&mut self) {
        if self.0 {
            unsafe { RoUninitialize() };
        }
    }
}

struct SessionTracker {
    session: Option<GlobalSystemMediaTransportControlsSession>,
    props_token: i64,
    playback_token: i64,
    timeline_token: i64,
}

impl SessionTracker {
    fn new() -> Self {
        Self {
            session: None,
            props_token: 0,
            playback_token: 0,
            timeline_token: 0,
        }
    }

    fn detach(&mut self) {
        if let Some(sess) = self.session.take() {
            if self.props_token != 0 {
                let _ = sess.RemoveMediaPropertiesChanged(self.props_token);
                self.props_token = 0;
            }
            if self.playback_token != 0 {
                let _ = sess.RemovePlaybackInfoChanged(self.playback_token);
                self.playback_token = 0;
            }
            if self.timeline_token != 0 {
                let _ = sess.RemoveTimelinePropertiesChanged(self.timeline_token);
                self.timeline_token = 0;
            }
        }
    }

    fn attach(
        &mut self,
        new_session: Option<GlobalSystemMediaTransportControlsSession>,
        cmd_tx: &Sender<MediaCommand>,
    ) -> bool {
        let same = match (&self.session, &new_session) {
            (Some(a), Some(b)) => a == b,
            (None, None) => true,
            _ => false,
        };
        if same && self.session.is_some() {
            return false;
        }

        self.detach();
        let Some(sess) = new_session else {
            return true;
        };

        let tx1 = cmd_tx.clone();
        if let Ok(token) = sess.MediaPropertiesChanged(&TypedEventHandler::<
            GlobalSystemMediaTransportControlsSession,
            MediaPropertiesChangedEventArgs,
        >::new(move |_, _| {
            let _ = tx1.send(MediaCommand::Refresh { check_artwork: true });
            Ok(())
        })) {
            self.props_token = token;
        }

        let tx2 = cmd_tx.clone();
        if let Ok(token) = sess.PlaybackInfoChanged(&TypedEventHandler::<
            GlobalSystemMediaTransportControlsSession,
            PlaybackInfoChangedEventArgs,
        >::new(move |_, _| {
            let _ = tx2.send(MediaCommand::Refresh {
                check_artwork: false,
            });
            Ok(())
        })) {
            self.playback_token = token;
        }

        let tx3 = cmd_tx.clone();
        if let Ok(token) = sess.TimelinePropertiesChanged(&TypedEventHandler::<
            GlobalSystemMediaTransportControlsSession,
            TimelinePropertiesChangedEventArgs,
        >::new(move |_, _| {
            let _ = tx3.send(MediaCommand::Refresh {
                check_artwork: false,
            });
            Ok(())
        })) {
            self.timeline_token = token;
        }

        self.session = Some(sess);
        true
    }
}

impl Drop for SessionTracker {
    fn drop(&mut self) {
        self.detach();
    }
}

fn media_worker_loop(
    hwnd_raw: isize,
    cmd_rx: Receiver<MediaCommand>,
    cmd_tx: Sender<MediaCommand>,
    update_tx: Sender<MediaUpdate>,
) {
    let _winrt = WinRtGuard::init();
    let wic_factory: Option<IWICImagingFactory> =
        unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }.ok();

    let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
        .ok()
        .and_then(|op| op.join().ok());

    let mut manager_token = 0i64;
    if let Some(mgr) = &manager {
        let tx = cmd_tx.clone();
        if let Ok(token) = mgr.CurrentSessionChanged(&TypedEventHandler::<
            GlobalSystemMediaTransportControlsSessionManager,
            CurrentSessionChangedEventArgs,
        >::new(move |_, _| {
            let _ = tx.send(MediaCommand::Refresh { check_artwork: true });
            Ok(())
        })) {
            manager_token = token;
        }
    }

    let mut tracker = SessionTracker::new();
    let mut last_display = MediaDisplayState::default();
    let mut last_artwork_hash: Option<u64> = None;

    if let Some(mgr) = &manager {
        push_snapshot(
            hwnd_raw,
            mgr,
            &mut tracker,
            &cmd_tx,
            &update_tx,
            wic_factory.as_ref(),
            &mut last_display,
            &mut last_artwork_hash,
            true,
        );
    }

    while let Ok(first_cmd) = cmd_rx.recv() {
        let mut should_stop = matches!(first_cmd, MediaCommand::Stop);
        let mut need_refresh = matches!(first_cmd, MediaCommand::Refresh { .. });
        let mut check_artwork = matches!(
            first_cmd,
            MediaCommand::Refresh {
                check_artwork: true
            }
        );
        let mut pending_control = match first_cmd {
            MediaCommand::TogglePlayPause
            | MediaCommand::Next
            | MediaCommand::Previous
            | MediaCommand::Seek(_) => Some(first_cmd),
            _ => None,
        };

        while let Ok(next_cmd) = cmd_rx.try_recv() {
            match next_cmd {
                MediaCommand::Stop => {
                    should_stop = true;
                    break;
                }
                MediaCommand::Refresh { check_artwork: art } => {
                    need_refresh = true;
                    check_artwork |= art;
                }
                MediaCommand::TogglePlayPause
                | MediaCommand::Next
                | MediaCommand::Previous
                | MediaCommand::Seek(_) => {
                    pending_control = Some(next_cmd);
                }
            }
        }

        if should_stop {
            break;
        }

        if let Some(ctrl_cmd) = pending_control
            && let Some(sess) = tracker
                .session
                .clone()
                .or_else(|| manager.as_ref().and_then(|m| m.GetCurrentSession().ok()))
        {
            execute_control_command(&sess, ctrl_cmd);
            need_refresh = true;
        }

        if need_refresh && let Some(mgr) = &manager {
            push_snapshot(
                hwnd_raw,
                mgr,
                &mut tracker,
                &cmd_tx,
                &update_tx,
                wic_factory.as_ref(),
                &mut last_display,
                &mut last_artwork_hash,
                check_artwork,
            );
        }
    }

    tracker.detach();
    if let Some(mgr) = &manager
        && manager_token != 0
    {
        let _ = mgr.RemoveCurrentSessionChanged(manager_token);
    }
}

fn execute_control_command(
    session: &GlobalSystemMediaTransportControlsSession,
    cmd: MediaCommand,
) {
    match cmd {
        MediaCommand::TogglePlayPause => {
            if let Ok(op) = session.TryTogglePlayPauseAsync() {
                let _ = op.join();
            }
        }
        MediaCommand::Next => {
            if let Ok(op) = session.TrySkipNextAsync() {
                let _ = op.join();
            }
        }
        MediaCommand::Previous => {
            if let Ok(op) = session.TrySkipPreviousAsync() {
                let _ = op.join();
            }
        }
        MediaCommand::Seek(ticks) => {
            if let Ok(op) = session.TryChangePlaybackPositionAsync(ticks.max(0)) {
                let _ = op.join();
            }
        }
        MediaCommand::Refresh { .. } | MediaCommand::Stop => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn push_snapshot(
    hwnd_raw: isize,
    manager: &GlobalSystemMediaTransportControlsSessionManager,
    tracker: &mut SessionTracker,
    cmd_tx: &Sender<MediaCommand>,
    update_tx: &Sender<MediaUpdate>,
    wic: Option<&IWICImagingFactory>,
    last_display: &mut MediaDisplayState,
    last_artwork_hash: &mut Option<u64>,
    force_check_artwork: bool,
) {
    let current = manager.GetCurrentSession().ok();
    let session_changed = tracker.attach(current, cmd_tx);

    let Some(session) = tracker.session.as_ref() else {
        let empty = MediaDisplayState::default();
        if *last_display != empty || last_artwork_hash.is_some() {
            *last_display = empty.clone();
            *last_artwork_hash = None;
            let _ = update_tx.send(MediaUpdate {
                display: empty,
                artwork: ArtworkUpdate::Clear,
            });
            notify_ui(hwnd_raw);
        }
        return;
    };

    let raw_aumid = session
        .SourceAppUserModelId()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    let source_app = friendly_source_name(&raw_aumid);

    let is_playing = session
        .GetPlaybackInfo()
        .and_then(|info| info.PlaybackStatus())
        .is_ok_and(|st| st == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing);

    let timeline = read_timeline_state(session);

    let props = session
        .TryGetMediaPropertiesAsync()
        .ok()
        .and_then(|op| op.join().ok());

    let raw_title = props
        .as_ref()
        .and_then(|p| p.Title().ok())
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    let raw_artist = props
        .as_ref()
        .and_then(|p| p.Artist().ok())
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();

    let title = if raw_title.trim().is_empty() {
        if source_app.is_empty() {
            String::from("Đang phát phương tiện")
        } else {
            source_app.clone()
        }
    } else {
        raw_title.trim().to_string()
    };
    let artist = if raw_artist.trim().is_empty() {
        if source_app.is_empty() {
            String::from("Không rõ nghệ sĩ")
        } else {
            source_app.clone()
        }
    } else {
        raw_artist.trim().to_string()
    };

    let track_meta_changed = last_display.title != title
        || last_display.artist != artist
        || last_display.source_app != source_app
        || !last_display.has_session;

    let next_display = MediaDisplayState {
        has_session: true,
        is_playing,
        title,
        artist,
        source_app,
        timeline,
    };

    let should_check_art =
        session_changed || track_meta_changed || force_check_artwork || last_artwork_hash.is_none();

    let artwork = if should_check_art {
        match (wic, props.as_ref()) {
            (Some(factory), Some(media_props)) => {
                fetch_artwork_if_changed(factory, media_props, last_artwork_hash)
            }
            _ => {
                if last_artwork_hash.take().is_some() {
                    ArtworkUpdate::Clear
                } else {
                    ArtworkUpdate::Unchanged
                }
            }
        }
    } else {
        ArtworkUpdate::Unchanged
    };

    let artwork_changed = !matches!(artwork, ArtworkUpdate::Unchanged);
    if *last_display != next_display || artwork_changed {
        *last_display = next_display.clone();
        let _ = update_tx.send(MediaUpdate {
            display: next_display,
            artwork,
        });
        notify_ui(hwnd_raw);
    }
}

fn read_timeline_state(session: &GlobalSystemMediaTransportControlsSession) -> TimelineState {
    let Ok(tl) = session.GetTimelineProperties() else {
        return TimelineState::default();
    };
    let start = tl.StartTime().map(|t| t.Duration).unwrap_or(0);
    let end = tl.EndTime().map(|t| t.Duration).unwrap_or(0);
    let pos = tl.Position().map(|t| t.Duration).unwrap_or(0);
    let updated = tl
        .LastUpdatedTime()
        .map(|d| d.UniversalTime)
        .unwrap_or(0);

    let dur = end - start;
    if dur <= 0 {
        return TimelineState::default();
    }

    let last_updated_filetime_ticks = if updated > 0 {
        updated
    } else {
        current_windows_filetime_ticks()
    };

    TimelineState {
        available: true,
        start_ticks: start,
        end_ticks: end,
        base_position_ticks: pos.clamp(start, end),
        last_updated_filetime_ticks,
    }
}

fn notify_ui(hwnd_raw: isize) {
    let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_APP_MEDIA_UPDATE, WPARAM(0), LPARAM(0));
    }
}

fn fetch_artwork_if_changed(
    wic: &IWICImagingFactory,
    props: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties,
    last_hash: &mut Option<u64>,
) -> ArtworkUpdate {
    let Some(bytes) = read_thumbnail_bytes(props) else {
        if last_hash.take().is_some() {
            return ArtworkUpdate::Clear;
        }
        return ArtworkUpdate::Unchanged;
    };

    let hash = fnv1a64(&bytes);
    if *last_hash == Some(hash) {
        return ArtworkUpdate::Unchanged;
    }

    match decode_thumbnail_bytes(wic, &bytes) {
        Some(art) => {
            *last_hash = Some(hash);
            ArtworkUpdate::Set(art)
        }
        None => {
            if last_hash.take().is_some() {
                ArtworkUpdate::Clear
            } else {
                ArtworkUpdate::Unchanged
            }
        }
    }
}

fn read_thumbnail_bytes(
    props: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties,
) -> Option<Vec<u8>> {
    let thumb_ref = props.Thumbnail().ok()?;
    let stream = thumb_ref.OpenReadAsync().ok()?.join().ok()?;
    let size = stream.Size().ok()?;
    if size == 0 || size > MAX_THUMBNAIL_BYTES {
        return None;
    }

    let size_u32 = size as u32;
    let input_stream: IInputStream = stream.cast().ok()?;
    let reader = DataReader::CreateDataReader(&input_stream).ok()?;
    let loaded = reader.LoadAsync(size_u32).ok()?.join().ok()?;
    if loaded == 0 {
        return None;
    }

    let mut bytes = vec![0u8; loaded as usize];
    reader.ReadBytes(&mut bytes).ok()?;
    Some(bytes)
}

fn decode_thumbnail_bytes(wic: &IWICImagingFactory, bytes: &[u8]) -> Option<DecodedArtwork> {
    let mem_stream = unsafe { wic.CreateStream() }.ok()?;
    unsafe { mem_stream.InitializeFromMemory(bytes) }.ok()?;

    let decoder = unsafe {
        wic.CreateDecoderFromStream(
            &mem_stream,
            std::ptr::null(),
            WICDecodeMetadataCacheOnDemand,
        )
    }
    .ok()?;
    let frame = unsafe { decoder.GetFrame(0) }.ok()?;

    let mut src_w = 0u32;
    let mut src_h = 0u32;
    unsafe { frame.GetSize(&mut src_w, &mut src_h) }.ok()?;
    if src_w == 0 || src_h == 0 {
        return None;
    }

    let (dst_w, dst_h) = compute_scaled_dimensions(src_w, src_h, TARGET_ARTWORK_PX);
    let source: IWICBitmapSource = if dst_w != src_w || dst_h != src_h {
        let scaler = unsafe { wic.CreateBitmapScaler() }.ok()?;
        unsafe { scaler.Initialize(&frame, dst_w, dst_h, WICBitmapInterpolationModeFant) }.ok()?;
        scaler.cast().ok()?
    } else {
        frame.cast().ok()?
    };

    let converter = unsafe { wic.CreateFormatConverter() }.ok()?;
    unsafe {
        converter.Initialize(
            &source,
            &GUID_WICPixelFormat32bppPBGRA,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeMedianCut,
        )
    }
    .ok()?;

    let stride = dst_w.checked_mul(4)?;
    let buf_len = (stride as usize).checked_mul(dst_h as usize)?;
    let mut bgra = vec![0u8; buf_len];
    unsafe { converter.CopyPixels(std::ptr::null(), stride, &mut bgra) }.ok()?;

    Some(DecodedArtwork {
        width: dst_w,
        height: dst_h,
        bgra,
    })
}

pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub fn compute_scaled_dimensions(src_w: u32, src_h: u32, max_dim: u32) -> (u32, u32) {
    if src_w == 0 || src_h == 0 || max_dim == 0 {
        return (1, 1);
    }
    if src_w <= max_dim && src_h <= max_dim {
        return (src_w, src_h);
    }
    if src_w >= src_h {
        let h = ((src_h as u64 * max_dim as u64) / src_w as u64).max(1) as u32;
        (max_dim, h)
    } else {
        let w = ((src_w as u64 * max_dim as u64) / src_h as u64).max(1) as u32;
        (w, max_dim)
    }
}

pub fn friendly_source_name(aumid: &str) -> String {
    let trimmed = aumid.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("spotify") {
        return String::from("Spotify");
    }
    if lower.contains("chrome") {
        return String::from("Google Chrome");
    }
    if lower.contains("msedge") || lower.contains("edge") {
        return String::from("Microsoft Edge");
    }
    if lower.contains("firefox") {
        return String::from("Firefox");
    }
    if lower.contains("vlc") {
        return String::from("VLC Media Player");
    }
    if lower.contains("foobar") {
        return String::from("foobar2000");
    }
    if lower.contains("zune") || lower.contains("mediaplayer") {
        return String::from("Media Player");
    }
    if lower.contains("coccoc") {
        return String::from("Cốc Cốc");
    }

    let tail = trimmed.rsplit('!').next().unwrap_or(trimmed);
    let base = tail.split('_').next().unwrap_or(tail);
    let clean = base
        .strip_suffix(".exe")
        .or_else(|| base.strip_suffix(".EXE"))
        .unwrap_or(base)
        .trim();
    clean.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friendly_names_map_common_players() {
        assert_eq!(
            friendly_source_name("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"),
            "Spotify"
        );
        assert_eq!(friendly_source_name("chrome.exe"), "Google Chrome");
        assert_eq!(friendly_source_name("MSEdge"), "Microsoft Edge");
        assert_eq!(friendly_source_name("vlc.exe"), "VLC Media Player");
        assert_eq!(friendly_source_name("CustomApp.exe"), "CustomApp");
        assert_eq!(friendly_source_name(""), "");
    }

    #[test]
    fn scaled_dimensions_respect_max_bound_and_aspect_ratio() {
        assert_eq!(compute_scaled_dimensions(1920, 1080, 96), (96, 54));
        assert_eq!(compute_scaled_dimensions(600, 1200, 96), (48, 96));
        assert_eq!(compute_scaled_dimensions(64, 64, 96), (64, 64));
        assert_eq!(compute_scaled_dimensions(0, 0, 96), (1, 1));
    }

    #[test]
    fn fnv1a_detects_thumbnail_bytes_changes() {
        let h1 = fnv1a64(b"chrome-icon-bytes");
        let h2 = fnv1a64(b"youtube-video-thumb-bytes");
        assert_ne!(h1, h2);
        assert_eq!(h1, fnv1a64(b"chrome-icon-bytes"));
    }
}
