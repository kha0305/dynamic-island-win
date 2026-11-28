use std::mem::size_of;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateRectRgn, CreateRoundRectRgn, DeleteObject, HGDIOBJ, ScreenToClient, SetWindowRgn,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Power::{
    HPOWERNOTIFY, POWERBROADCAST_SETTING, RegisterPowerSettingNotification,
    UnregisterPowerSettingNotification,
};
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DEVICE_NOTIFY_WINDOW_HANDLE, DefWindowProcW,
    DestroyWindow, GWLP_USERDATA, GetCursorPos, GetWindowLongPtrW, HCURSOR, IDC_ARROW, IDC_HAND,
    IsWindow, IsWindowVisible, KillTimer, LoadCursorW, MA_NOACTIVATE, PBT_APMPOWERSTATUSCHANGE,
    PBT_POWERSETTINGCHANGE, PostQuitMessage, RegisterClassExW, SW_HIDE, SW_SHOWNOACTIVATE,
    SWP_NOACTIVATE, SWP_NOZORDER, SetCursor, SetTimer, SetWindowLongPtrW, SetWindowPos,
    ShowWindow, UnregisterClassW, WM_APP, WM_CAPTURECHANGED, WM_COMMAND, WM_DESTROY,
    WM_DISPLAYCHANGE, WM_DPICHANGED, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCCREATE, WM_POWERBROADCAST, WM_RBUTTONUP,
    WM_SETCURSOR, WM_TIMER, WNDCLASSEXW, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WindowFromPoint,
};
use windows::core::PCWSTR;

use crate::config::{load_config, save_config, set_autostart_enabled};
use crate::hud::{
    AudioCommand, AudioMonitor, AudioOutputDevice, GUID_ACDC_POWER_SOURCE, HUD_EXPIRE_DELAY_MS,
    HudWorkerEvent, PowerStateTracker, VolumeSnapshot, WM_APP_HUD_EVENT, WheelVolumeAccumulator,
    apply_volume_step,
};
use crate::media::{MediaCommand, MediaWorker, WM_APP_MEDIA_UPDATE};
use crate::renderer::{IslandRenderer, RenderFrameInput};
use crate::state::{
    AppConfig, HudContent, IslandAnimator, IslandMode, IslandNoticeView,
    MAX_AUDIO_DEVICES_VISIBLE, MAX_SURFACE_HEIGHT_DIP, MAX_SURFACE_WIDTH_DIP, MediaDisplayState,
    NoticeKind, PillRectDip, PixelRect, UiAction, UiHitbox, UiRectDip, compute_pill_region_px,
    compute_surface_size_px, compute_window_origin_px, current_windows_filetime_ticks,
    effective_mode, fill_audio_devices_hitboxes, fill_interactive_hitboxes, fill_notice_hitboxes,
    fill_volume_hud_hitboxes, pill_contains_dip, should_collapse_expanded_on_blank_click,
    target_dimensions_for_state, target_top_for_mode, timeline_ratio_for_x,
    timeline_seek_hit_bounds, volume_seek_hit_bounds,
};
use crate::tray::{
    IDM_AUDIO_DEVICE_BASE, IDM_AUDIO_DEVICE_MAX, IDM_CHECK_UPDATE, IDM_EXIT, IDM_MONITOR_BASE,
    IDM_MONITOR_MAX, IDM_OPEN_SETTINGS, IDM_TOGGLE_AUTO_HIDE, IDM_TOGGLE_AUTOSTART,
    IDM_TOGGLE_EXPANDED, IDM_TOGGLE_FULLSCREEN, IDM_TOGGLE_HUD_POWER, IDM_TOGGLE_HUD_VOLUME,
    IDM_TOGGLE_HUD_WHEEL, IDM_TOGGLE_NOTCH_COLLAPSE, IDM_TOGGLE_VISIBLE, IDM_TOGGLE_VISUALIZER,
    TrayIcon, WM_APP_TRAY, to_wide_null,
};
use crate::updater::{
    APP_VERSION, UpdateReleaseInfo, UpdaterEvent, WM_APP_UPDATE_EVENT, check_for_updates_async,
    download_and_stage_update_async, spawn_updated_process,
};
use crate::vsync::{TickerMode, VSyncTicker, WM_APP_VSYNC_TICK};
use crate::watcher::{
    DisplayMonitor, ForegroundHookGuard, WM_APP_FOREGROUND_CHANGED, enumerate_monitors,
    is_foreground_fullscreen_on_monitor, select_monitor_or_default,
};

pub const WINDOW_CLASS_NAME: &str = "WinDynamicIslandWindowClass";
pub const WM_APP_WAKE_INSTANCE: u32 = WM_APP + 5;
const TIMER_AUTO_COLLAPSE_ID: usize = 1;
const TIMER_FULLSCREEN_CHECK_ID: usize = 2;
const TIMER_TIMELINE_TICK_ID: usize = 3;
const TIMER_STARTUP_UPDATE_ID: usize = 4;
const TIMER_COMPACT_TO_NOTCH_ID: usize = 5;
const TIMER_HUD_EXPIRE_ID: usize = 6;
const TIMER_NOTICE_EXPIRE_ID: usize = 7;
const AUTO_COLLAPSE_DELAY_MS: u32 = 5_000;
const COMPACT_TO_NOTCH_DELAY_MS: u32 = 4_000;
const NOTICE_AUTO_DISMISS_MS: u32 = 4_000;
const FULLSCREEN_CHECK_INTERVAL_MS: u32 = 800;
const TIMELINE_TICK_INTERVAL_MS: u32 = 1000;
const STARTUP_UPDATE_DELAY_MS: u32 = 10_000;

const TIMELINE_SEEK_HOLD_MS: u64 = 850;
const VOLUME_ECHO_HOLD_MS: u64 = 150;
const MK_LBUTTON_MASK: usize = 0x0001;
const MK_RBUTTON_MASK: usize = 0x0002;
const MK_SHIFT_MASK: usize = 0x0004;
const MK_CONTROL_MASK: usize = 0x0008;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CaptureChange {
    None,
    Acquire,
    Release,
}

enum NoticePrimaryRole {
    Dismiss,
    InstallUpdate(UpdateReleaseInfo),
}

enum NoticeSecondaryRole {
    Dismiss,
    RetryCheck,
    RetryDownload(UpdateReleaseInfo),
}

struct ActiveNotice {
    kind: NoticeKind,
    title: String,
    message: String,
    primary_label: &'static str,
    primary_role: NoticePrimaryRole,
    secondary_label: Option<&'static str>,
    secondary_role: NoticeSecondaryRole,
    return_mode: IslandMode,
    auto_expire: bool,
}

impl ActiveNotice {
    fn as_view(&self) -> IslandNoticeView<'_> {
        IslandNoticeView {
            kind: self.kind,
            title: &self.title,
            message: &self.message,
            primary_label: self.primary_label,
            secondary_label: self.secondary_label,
        }
    }
}

struct WindowController {
    hwnd: HWND,
    config: AppConfig,
    monitors: Vec<DisplayMonitor>,
    requested_mode: IslandMode,
    settings_return_mode: IslandMode,
    audio_devices_return_mode: IslandMode,
    media_state: MediaDisplayState,
    fullscreen_active: bool,
    mouse_tracked: bool,
    dragging_timeline: bool,
    timeline_drag_ratio: Option<f32>,
    dragging_volume: bool,
    volume_drag_ratio: Option<f32>,
    drag_render_dirty: bool,
    volume_hold_until: Option<Instant>,
    wheel_accum: WheelVolumeAccumulator,
    pointer_down_pos: Option<(f32, f32)>,
    pressed_action: Option<UiAction>,
    seek_hold_until: Option<Instant>,
    hovered_action: Option<UiAction>,
    dpi_scale: f32,
    animator: IslandAnimator,
    renderer: IslandRenderer,
    vsync: Option<VSyncTicker>,
    media_worker: Option<MediaWorker>,
    audio_monitor: Option<AudioMonitor>,
    audio_devices: Vec<AudioOutputDevice>,
    audio_device_scroll: usize,
    current_audio_device_name: Option<String>,
    volume_snapshot: VolumeSnapshot,
    power_tracker: PowerStateTracker,
    power_notify_handle: Option<HPOWERNOTIFY>,
    active_hud: Option<HudContent>,
    fg_hook: Option<ForegroundHookGuard>,
    tray: Option<TrayIcon>,
    last_region: Option<(PixelRect, i32)>,
    last_window_rect: Option<(i32, i32, i32, i32)>,
    hitbox_scratch: Vec<UiHitbox>,
    visualizer_phase: f32,
    cursor_arrow: HCURSOR,
    cursor_hand: HCURSOR,
    update_rx: Option<Receiver<UpdaterEvent>>,
    available_update: Option<UpdateReleaseInfo>,
    downloading_release: Option<UpdateReleaseInfo>,
    active_notice: Option<ActiveNotice>,
    update_in_progress: bool,
}

pub struct IslandWindow {
    hwnd: HWND,
    class_name: Vec<u16>,
    hinstance: HINSTANCE,
}

impl IslandWindow {
    pub fn create() -> windows::core::Result<Self> {
        let hmodule = unsafe { GetModuleHandleW(None) }?;
        let hinstance: HINSTANCE = hmodule.into();
        let class_name = to_wide_null(WINDOW_CLASS_NAME);
        let window_title = to_wide_null("Win Dynamic Island");
        let cursor_arrow = unsafe { LoadCursorW(None, IDC_ARROW) }?;
        let cursor_hand = unsafe { LoadCursorW(None, IDC_HAND) }?;

        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: hinstance,
            hCursor: cursor_arrow,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };

        let atom = unsafe { RegisterClassExW(&wc) };
        if atom == 0 {
            return Err(windows::core::Error::from_thread());
        }

        let config = load_config();
        let monitors = enumerate_monitors();
        let active_mon = select_monitor_or_default(&monitors, config.monitor_index);
        let (init_w, init_h) = compute_surface_size_px(active_mon.dpi_scale);
        let (init_x, init_y) = compute_window_origin_px(
            active_mon.monitor_rect.0,
            active_mon.monitor_rect.2,
            active_mon.work_top,
            init_w,
        );

        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP | WS_EX_NOACTIVATE,
                PCWSTR(class_name.as_ptr()),
                PCWSTR(window_title.as_ptr()),
                WS_POPUP,
                init_x,
                init_y,
                init_w,
                init_h,
                None,
                None,
                Some(hinstance),
                None,
            )
        };

        let Ok(hwnd) = hwnd else {
            unsafe {
                let _ = UnregisterClassW(PCWSTR(class_name.as_ptr()), Some(hinstance));
            }
            return Err(windows::core::Error::from_thread());
        };

        let renderer = match IslandRenderer::new(hwnd, active_mon.dpi_scale) {
            Ok(r) => r,
            Err(err) => {
                unsafe {
                    let _ = DestroyWindow(hwnd);
                    let _ = UnregisterClassW(PCWSTR(class_name.as_ptr()), Some(hinstance));
                }
                return Err(err);
            }
        };

        let vsync = VSyncTicker::start(hwnd);
        let media_worker = MediaWorker::start(hwnd);
        let audio_monitor = AudioMonitor::start(hwnd);
        let fg_hook = ForegroundHookGuard::install(hwnd);

        let power_tracker = PowerStateTracker::default();
        let power_notify_handle = unsafe {
            RegisterPowerSettingNotification(
                HANDLE(hwnd.0),
                &GUID_ACDC_POWER_SOURCE,
                DEVICE_NOTIFY_WINDOW_HANDLE,
            )
            .ok()
        };

        let tooltip = format!("Win Dynamic Island (GPU: {})", renderer.adapter_name());
        let tray = TrayIcon::new(hwnd, &tooltip).ok();

        let initial_mode = effective_mode(IslandMode::Slim, config, false, false, false);
        let controller = Box::new(WindowController {
            hwnd,
            config,
            monitors,
            requested_mode: IslandMode::Slim,
            settings_return_mode: IslandMode::Expanded,
            audio_devices_return_mode: IslandMode::Expanded,
            media_state: MediaDisplayState::default(),
            fullscreen_active: false,
            mouse_tracked: false,
            dragging_timeline: false,
            timeline_drag_ratio: None,
            dragging_volume: false,
            volume_drag_ratio: None,
            drag_render_dirty: false,
            volume_hold_until: None,
            wheel_accum: WheelVolumeAccumulator::default(),
            pointer_down_pos: None,
            pressed_action: None,
            seek_hold_until: None,
            hovered_action: None,
            dpi_scale: active_mon.dpi_scale,
            animator: IslandAnimator::new(initial_mode),
            renderer,
            vsync: Some(vsync),
            media_worker: Some(media_worker),
            audio_monitor: Some(audio_monitor),
            audio_devices: Vec::new(),
            audio_device_scroll: 0,
            current_audio_device_name: None,
            volume_snapshot: VolumeSnapshot::default(),
            power_tracker,
            power_notify_handle,
            active_hud: None,
            fg_hook: Some(fg_hook),
            tray,
            last_region: None,
            last_window_rect: None,
            hitbox_scratch: Vec::with_capacity(8),
            visualizer_phase: 0.0,
            cursor_arrow,
            cursor_hand,
            update_rx: None,
            available_update: None,
            downloading_release: None,
            active_notice: None,
            update_in_progress: false,
        });

        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(controller) as isize);
        }

        if let Some(ctrl) = get_controller_mut(hwnd) {
            ctrl.reposition_for_selected_monitor();
            ctrl.sync_fullscreen_timer();
            ctrl.refresh_fullscreen_state();
            ctrl.apply_target_and_render();
            ctrl.update_tray_tooltip();
            ctrl.schedule_startup_update_check();
        }

        Ok(Self {
            hwnd,
            class_name,
            hinstance,
        })
    }
}

impl Drop for IslandWindow {
    fn drop(&mut self) {
        if !self.hwnd.is_invalid() && unsafe { IsWindow(Some(self.hwnd)).as_bool() } {
            unsafe {
                let _ = DestroyWindow(self.hwnd);
            }
        }
        unsafe {
            let _ = UnregisterClassW(PCWSTR(self.class_name.as_ptr()), Some(self.hinstance));
        }
    }
}

impl WindowController {
    fn active_monitor(&self) -> DisplayMonitor {
        select_monitor_or_default(&self.monitors, self.config.monitor_index)
    }

    fn current_effective_mode(&self) -> IslandMode {
        effective_mode(
            self.requested_mode,
            self.config,
            self.fullscreen_active,
            self.media_state.has_session,
            self.active_hud.is_some(),
        )
    }

    fn refresh_monitors(&mut self) {
        self.monitors = enumerate_monitors();
        if !self.monitors.is_empty() {
            self.config.monitor_index %= self.monitors.len();
        }
        self.reposition_for_selected_monitor();
    }

    fn reposition_for_selected_monitor(&mut self) {
        let mon = self.active_monitor();
        if (mon.dpi_scale - self.dpi_scale).abs() >= 0.001 {
            self.dpi_scale = mon.dpi_scale;
            let _ = self.renderer.resize_for_dpi(mon.dpi_scale);
            self.last_region = None;
        }

        let (w, h) = compute_surface_size_px(self.dpi_scale);
        let (x, y) = compute_window_origin_px(mon.monitor_rect.0, mon.monitor_rect.2, mon.work_top, w);
        let next_rect = (x, y, w, h);
        if self.last_window_rect != Some(next_rect) {
            self.last_window_rect = Some(next_rect);
            unsafe {
                let _ = SetWindowPos(
                    self.hwnd,
                    None,
                    x,
                    y,
                    w,
                    h,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }
    }

    fn sync_fullscreen_timer(&self) {
        if self.config.visible && self.config.auto_hide_fullscreen {
            unsafe {
                let _ = SetTimer(
                    Some(self.hwnd),
                    TIMER_FULLSCREEN_CHECK_ID,
                    FULLSCREEN_CHECK_INTERVAL_MS,
                    None,
                );
            }
        } else {
            let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_FULLSCREEN_CHECK_ID) };
        }
    }

    fn reset_pointer_and_drag_state(&mut self) -> bool {
        let was_capturing = self.dragging_timeline || self.dragging_volume;
        self.mouse_tracked = false;
        self.hovered_action = None;
        self.pressed_action = None;
        self.pointer_down_pos = None;
        self.dragging_timeline = false;
        self.timeline_drag_ratio = None;
        self.dragging_volume = false;
        self.volume_drag_ratio = None;
        self.drag_render_dirty = false;
        self.wheel_accum.reset();
        was_capturing
    }

    fn is_cursor_actually_over_pill(&self) -> bool {
        if !unsafe { IsWindowVisible(self.hwnd) }.as_bool() {
            return false;
        }
        let mut pt = POINT::default();
        if unsafe { GetCursorPos(&mut pt) }.is_err() {
            return false;
        }
        let hit_hwnd = unsafe { WindowFromPoint(pt) };
        if hit_hwnd != self.hwnd {
            return false;
        }
        if !unsafe { ScreenToClient(self.hwnd, &mut pt) }.as_bool() {
            return false;
        }
        let scale = self.dpi_scale.max(0.5);
        pill_contains_dip(
            self.animator.current_pill(),
            pt.x as f32 / scale,
            pt.y as f32 / scale,
            2.0,
        )
    }

    fn sync_pointer_presence(&mut self) {
        if self.mouse_tracked
            && !self.dragging_timeline
            && !self.dragging_volume
            && !self.is_cursor_actually_over_pill()
        {
            self.handle_mouse_leave();
        }
    }

    fn refresh_fullscreen_state(&mut self) -> bool {
        let next = if self.config.auto_hide_fullscreen {
            let mon_rect = self.active_monitor().monitor_rect;
            is_foreground_fullscreen_on_monitor(self.hwnd, mon_rect)
        } else {
            false
        };
        if next != self.fullscreen_active {
            self.fullscreen_active = next;
            if next {
                // Ẩn ngay khi vào Fullscreen để không đè khung hình lên game hoặc video.
                let need_release = self.reset_pointer_and_drag_state();
                self.clear_hud_state_quiet();
                self.animator = IslandAnimator::new(IslandMode::Hidden);
                unsafe {
                    let _ = ShowWindow(self.hwnd, SW_HIDE);
                }
                self.apply_empty_region();
                self.update_vsync_activity();
                self.update_tray_tooltip();
                return need_release;
            } else {
                self.apply_target_and_render();
                self.update_tray_tooltip();
            }
        }
        false
    }

    fn toggle_visibility_from_tray(&mut self) {
        if self.current_effective_mode() == IslandMode::Hidden {
            self.config.visible = true;
            self.requested_mode = IslandMode::Compact;
            if !self.mouse_tracked
                && (self.config.notch_collapse_after_4s || !self.media_state.has_session)
            {
                self.schedule_compact_to_notch();
            }
        } else {
            self.cancel_compact_to_notch();
            let _ = self.reset_pointer_and_drag_state();
            self.config.visible = false;
        }
        self.sync_fullscreen_timer();
        self.apply_target_and_render();
        self.update_tray_tooltip();
        save_config(&self.config);
    }

    fn wake_from_duplicate_launch(&mut self) {
        self.config.visible = true;
        self.requested_mode = if self.media_state.has_session {
            self.cancel_compact_to_notch();
            IslandMode::Expanded
        } else {
            if !self.mouse_tracked {
                self.schedule_compact_to_notch();
            }
            IslandMode::Compact
        };
        if self.requested_mode == IslandMode::Expanded && !self.mouse_tracked {
            self.schedule_auto_collapse(AUTO_COLLAPSE_DELAY_MS);
        }
        self.sync_fullscreen_timer();
        self.apply_target_and_render();
        self.update_tray_tooltip();
    }

    fn apply_target_and_render(&mut self) {
        let was_animating = self.animator.is_animating();
        let mode = self.current_effective_mode();
        self.animator
            .set_target_mode_with_audio_count(mode, self.audio_devices.len());

        if mode != IslandMode::Hidden {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            }
        }

        if !was_animating && self.animator.is_animating() {
            // Vẽ ngay khung hình chuyển động đầu tiên thay vì chờ nhịp thread nền tiếp theo.
            let _ = self.animator.step(Instant::now());
            self.render_current_frame();
        } else if !self.animator.is_animating() || self.vsync.is_none() || mode == IslandMode::Hidden {
            self.render_current_frame();
        }
        if self.animator.should_hide_window() {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
            self.apply_empty_region();
        }
        self.update_vsync_activity();
        self.sync_timeline_timer();
    }

    fn request_smooth_render(&mut self) {
        if self.animator.is_animating() && self.vsync.is_some() {
            return;
        }
        if (self.dragging_timeline || self.dragging_volume) && self.vsync.is_some() {
            self.drag_render_dirty = true;
            self.update_vsync_activity();
            return;
        }
        self.render_current_frame();
    }

    fn sync_timeline_timer(&self) {
        let mode = self.current_effective_mode();
        let need_timeline = self.media_state.has_session
            && self.media_state.is_playing
            && self.media_state.timeline.available
            && (mode == IslandMode::Expanded
                || (matches!(mode, IslandMode::Slim | IslandMode::Compact)
                    && !self.config.visualizer_enabled
                    && self.active_hud.is_none()));
        if need_timeline {
            unsafe {
                let _ = SetTimer(
                    Some(self.hwnd),
                    TIMER_TIMELINE_TICK_ID,
                    TIMELINE_TICK_INTERVAL_MS,
                    None,
                );
            }
        } else {
            let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_TIMELINE_TICK_ID) };
        }
    }

    fn handle_vsync_tick(&mut self) {
        if let Some(vsync) = &self.vsync {
            vsync.clear_pending();
        }
        let was_animating = self.animator.is_animating();
        let still_animating = self.animator.step(Instant::now());
        if self.config.visualizer_enabled && self.media_state.is_playing {
            self.visualizer_phase = (self.visualizer_phase + 0.32) % std::f32::consts::TAU;
        }

        let is_dragging = self.dragging_timeline || self.dragging_volume;
        let should_render = still_animating
            || was_animating
            || (is_dragging && self.drag_render_dirty)
            || (!is_dragging && self.config.visualizer_enabled && self.media_state.is_playing);
        if should_render {
            self.drag_render_dirty = false;
            self.render_current_frame();
        }

        if self.animator.should_hide_window() {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
            self.apply_empty_region();
        }

        if was_animating && !still_animating {
            self.sync_pointer_presence();
        }
        if !still_animating {
            self.update_vsync_activity();
        }
    }

    fn update_vsync_activity(&self) {
        let ticker_mode =
            if self.animator.is_animating() || self.dragging_timeline || self.dragging_volume {
                TickerMode::VSync
            } else if self.config.visualizer_enabled
                && self.media_state.has_session
                && self.media_state.is_playing
                && matches!(
                    self.current_effective_mode(),
                    IslandMode::Compact | IslandMode::Slim
                )
            {
                TickerMode::VisualizerSlow
            } else {
                TickerMode::Off
            };
        if let Some(vsync) = &self.vsync {
            vsync.set_mode(ticker_mode);
        }
    }

    fn handle_media_update(&mut self) {
        let Some(worker) = &self.media_worker else {
            return;
        };
        let Some(update) = worker.drain_latest_update() else {
            return;
        };

        let was_has_session = self.media_state.has_session;
        let same_track = update.display.has_session
            && was_has_session
            && update.display.title == self.media_state.title
            && update.display.artist == self.media_state.artist;
        let track_changed = update.display.has_session && !same_track;
        let preserve_local_timeline = same_track
            && update.display.timeline.available
            && (self.dragging_timeline
                || self
                    .seek_hold_until
                    .is_some_and(|until| Instant::now() < until));
        let prev_timeline = self.media_state.timeline;
        self.media_state = update.display;
        if preserve_local_timeline {
            self.media_state.timeline = prev_timeline;
        } else if !same_track {
            self.seek_hold_until = None;
            self.dragging_timeline = false;
            self.timeline_drag_ratio = None;
        }
        self.renderer.update_artwork(update.artwork);

        if self.media_state.has_session {
            if track_changed
                && matches!(self.requested_mode, IslandMode::Slim | IslandMode::Compact)
            {
                // Bung Compact 4 giây khi có nhạc mới / đổi bài để nhìn tên bài rồi thu về dải Notch.
                self.requested_mode = IslandMode::Compact;
                if self.config.notch_collapse_after_4s && !self.mouse_tracked {
                    self.schedule_compact_to_notch();
                } else {
                    self.cancel_compact_to_notch();
                }
            } else if self.requested_mode == IslandMode::Slim
                && !self.config.notch_collapse_after_4s
            {
                self.requested_mode = IslandMode::Compact;
                self.cancel_compact_to_notch();
            }
        } else {
            self.cancel_compact_to_notch();
            if self.requested_mode == IslandMode::Compact && !self.mouse_tracked {
                self.requested_mode = IslandMode::Slim;
            }
        }

        self.apply_target_and_render();
        self.update_tray_tooltip();
    }

    fn render_current_frame(&mut self) {
        let mode = self.current_effective_mode();
        if mode == IslandMode::Hidden && self.animator.should_hide_window() {
            self.renderer.hide_backdrop();
            return;
        }

        let pill = self.animator.current_pill();
        let display_mode = if mode == IslandMode::Hidden {
            self.animator.target_mode
        } else {
            mode
        };
        let mon_label = self.active_monitor().short_label.clone();
        let notice_view = self.active_notice.as_ref().map(ActiveNotice::as_view);

        let frame = RenderFrameInput {
            pill,
            mode: display_mode,
            pill_opacity: self.animator.opacity.value,
            content_alpha: self.animator.content_alpha.value,
            config: self.config,
            media: &self.media_state,
            active_hud: if display_mode == IslandMode::Compact {
                self.active_hud
            } else {
                None
            },
            volume_scrub_ratio: if self.dragging_volume {
                self.volume_drag_ratio
            } else {
                None
            },
            hovered_action: self.hovered_action,
            timeline_scrub_ratio: if self.dragging_timeline {
                self.timeline_drag_ratio
            } else {
                None
            },
            monitor_label: &mon_label,
            visualizer_phase: self.visualizer_phase,
            audio_devices: &self.audio_devices,
            audio_device_scroll: self.audio_device_scroll,
            volume_snapshot: self.volume_snapshot,
            current_audio_device_name: self.current_audio_device_name.as_deref(),
            notice: if display_mode == IslandMode::Notice {
                notice_view
            } else {
                None
            },
            available_update_tag: self
                .available_update
                .as_ref()
                .map(|r| r.version_tag.as_str()),
        };
        let _ = self.renderer.render(&frame);

        if self.animator.opacity.value > 0.02 {
            self.update_window_region(pill);
        }
    }

    fn update_window_region(&mut self, pill: PillRectDip) {
        if self.animator.is_animating() {
            if self.animator.target_mode == IslandMode::Hidden {
                return;
            }
            let cur_rgn = compute_pill_region_px(pill, self.dpi_scale);
            // Chỉ tái sử dụng vùng chữ nhật đang animate (diameter == 0) để không bao giờ bị góc bo tròn cũ cắt xén.
            let already_covers_rect = self.last_region.is_some_and(|(prev, diam)| {
                diam == 0
                    && prev.left <= cur_rgn.0.left
                    && prev.top <= cur_rgn.0.top
                    && prev.right >= cur_rgn.0.right
                    && prev.bottom >= cur_rgn.0.bottom
            });
            if already_covers_rect {
                return;
            }
            // Vùng bao hình chữ nhật trong suốt lúc đang co giãn để không cắt góc bo tròn và không ép DWM đổi Rgn liên tục.
            let (tw, th, _, _) =
                target_dimensions_for_state(self.animator.target_mode, self.audio_devices.len());
            let t_top = target_top_for_mode(self.animator.target_mode);
            let t_left = (MAX_SURFACE_WIDTH_DIP - tw) * 0.5;
            let left = (pill.left.min(t_left) - 14.0).max(0.0);
            let top = (pill.top.min(t_top) - 6.0).max(0.0);
            let right = ((pill.left + pill.width).max(t_left + tw) + 14.0).min(MAX_SURFACE_WIDTH_DIP);
            let bottom = ((pill.top + pill.height).max(t_top + th) + 10.0).min(MAX_SURFACE_HEIGHT_DIP);
            let union_pill = PillRectDip {
                left,
                top,
                width: (right - left).max(24.0),
                height: (bottom - top).max(8.0),
                corner_radius: 1.0,
            };
            let (rect, _) = compute_pill_region_px(union_pill, self.dpi_scale);
            self.apply_rect_region(rect);
        } else {
            let exact = compute_pill_region_px(pill, self.dpi_scale);
            if self.last_region != Some(exact) {
                self.apply_round_region(exact);
            }
        }
    }

    fn apply_rect_region(&mut self, rect: PixelRect) {
        let rgn = unsafe { CreateRectRgn(rect.left, rect.top, rect.right, rect.bottom) };
        if !rgn.is_invalid() {
            if unsafe { SetWindowRgn(self.hwnd, Some(rgn), false) } == 0 {
                unsafe {
                    let _ = DeleteObject(HGDIOBJ(rgn.0));
                }
            } else {
                self.last_region = Some((rect, 0));
            }
        }
    }

    fn apply_round_region(&mut self, next_rgn: (PixelRect, i32)) {
        let (rect, diameter) = next_rgn;
        let rgn = unsafe {
            CreateRoundRectRgn(
                rect.left,
                rect.top,
                rect.right,
                rect.bottom,
                diameter,
                diameter,
            )
        };
        if !rgn.is_invalid() {
            if unsafe { SetWindowRgn(self.hwnd, Some(rgn), false) } == 0 {
                unsafe {
                    let _ = DeleteObject(HGDIOBJ(rgn.0));
                }
            } else {
                self.last_region = Some(next_rgn);
            }
        }
    }

    fn apply_empty_region(&mut self) {
        self.last_region = None;
        let rgn = unsafe { CreateRectRgn(0, 0, 0, 0) };
        if !rgn.is_invalid() && unsafe { SetWindowRgn(self.hwnd, Some(rgn), false) } == 0 {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(rgn.0));
            }
        }
    }

    fn update_tray_tooltip(&self) {
        if let Some(tray) = &self.tray {
            let status = if !self.config.visible {
                "Đang ẩn"
            } else if self.fullscreen_active {
                "Tạm ẩn (Toàn màn hình)"
            } else if self.media_state.has_session && self.media_state.is_playing {
                "Đang phát"
            } else {
                "Sẵn sàng"
            };
            let glass_tag = if self.renderer.has_blur_effect() {
                "Kính mờ Acrylic - "
            } else if self.renderer.has_glass_backdrop() {
                "Kính trong - "
            } else {
                ""
            };
            let update_tag = match &self.available_update {
                Some(rel) => format!(" - Có bản mới {}", rel.version_tag),
                None if self.update_in_progress => String::from(" - Đang kiểm tra cập nhật"),
                None => String::new(),
            };
            let msg = format!(
                "Win Dynamic Island - {}{} ({}{})",
                status,
                update_tag,
                glass_tag,
                self.renderer.adapter_name()
            );
            tray.update_tooltip(&msg);
        }
    }

    fn schedule_startup_update_check(&self) {
        unsafe {
            let _ = SetTimer(
                Some(self.hwnd),
                TIMER_STARTUP_UPDATE_ID,
                STARTUP_UPDATE_DELAY_MS,
                None,
            );
        }
    }

    fn trigger_update_check(&mut self, silent: bool) {
        if self.update_in_progress {
            return;
        }
        self.update_in_progress = true;
        self.update_rx = Some(check_for_updates_async(self.hwnd, silent));
        if !silent {
            self.update_tray_tooltip();
        }
    }

    fn schedule_notice_expire(&self) {
        unsafe {
            let _ = SetTimer(
                Some(self.hwnd),
                TIMER_NOTICE_EXPIRE_ID,
                NOTICE_AUTO_DISMISS_MS,
                None,
            );
        }
    }

    fn cancel_notice_expire(&self) {
        let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_NOTICE_EXPIRE_ID) };
    }

    fn dismiss_notice_state_quiet(&mut self) {
        self.cancel_notice_expire();
        self.active_notice = None;
    }

    #[allow(clippy::too_many_arguments)]
    fn show_notice(
        &mut self,
        kind: NoticeKind,
        title: impl Into<String>,
        message: impl Into<String>,
        primary_label: &'static str,
        primary_role: NoticePrimaryRole,
        secondary_label: Option<&'static str>,
        secondary_role: NoticeSecondaryRole,
        auto_expire: bool,
    ) {
        let return_mode = if let Some(prev) = &self.active_notice {
            prev.return_mode
        } else if matches!(
            self.requested_mode,
            IslandMode::Notice | IslandMode::Hidden
        ) {
            self.collapsed_rest_mode()
        } else {
            self.requested_mode
        };

        self.clear_hud_state_quiet();
        self.cancel_compact_to_notch();
        self.cancel_auto_collapse();
        self.cancel_notice_expire();
        self.hovered_action = None;
        self.active_notice = Some(ActiveNotice {
            kind,
            title: title.into(),
            message: message.into(),
            primary_label,
            primary_role,
            secondary_label,
            secondary_role,
            return_mode,
            auto_expire,
        });
        self.requested_mode = IslandMode::Notice;
        if auto_expire && !self.mouse_tracked {
            self.schedule_notice_expire();
        }
        self.apply_target_and_render();
    }

    fn dismiss_notice(&mut self) {
        let Some(notice) = self.active_notice.take() else {
            return;
        };
        self.cancel_notice_expire();
        self.hovered_action = None;
        let next = match notice.return_mode {
            IslandMode::Notice | IslandMode::Hidden => self.collapsed_rest_mode(),
            IslandMode::Compact => self.collapsed_rest_mode(),
            other => other,
        };
        self.requested_mode = next;
        if next == IslandMode::Expanded && !self.mouse_tracked {
            self.schedule_auto_collapse(AUTO_COLLAPSE_DELAY_MS);
        } else if next == IslandMode::Compact && !self.mouse_tracked && self.config.notch_collapse_after_4s {
            self.schedule_compact_to_notch();
        }
        self.apply_target_and_render();
    }

    fn show_update_available_notice(&mut self, release: UpdateReleaseInfo) {
        let msg = format!(
            "Đã có phiên bản {} (hiện tại v{APP_VERSION}).\nBạn có muốn tải về và cập nhật ngay không?",
            release.version_tag
        );
        self.show_notice(
            NoticeKind::UpdateAvailable,
            "Có bản cập nhật mới",
            msg,
            "Cập nhật ngay",
            NoticePrimaryRole::InstallUpdate(release),
            Some("Để sau"),
            NoticeSecondaryRole::Dismiss,
            false,
        );
    }

    fn start_update_download(&mut self, release: UpdateReleaseInfo) {
        if self.update_in_progress {
            return;
        }
        self.update_in_progress = true;
        self.downloading_release = Some(release.clone());
        let tag = release.version_tag.clone();
        self.update_rx = Some(download_and_stage_update_async(self.hwnd, release));
        self.update_tray_tooltip();
        self.show_notice(
            NoticeKind::Downloading,
            "Đang tải bản cập nhật",
            format!("Đang tải và xác thực tệp cài đặt cho {tag}..."),
            "Thu gọn",
            NoticePrimaryRole::Dismiss,
            None,
            NoticeSecondaryRole::Dismiss,
            false,
        );
    }

    fn handle_check_update_command(&mut self) {
        self.config.visible = true;
        self.sync_fullscreen_timer();
        if self.update_in_progress {
            if self.active_notice.is_some() {
                self.requested_mode = IslandMode::Notice;
                self.apply_target_and_render();
            } else {
                self.show_notice(
                    NoticeKind::Checking,
                    "Kiểm tra cập nhật",
                    "Đang kiểm tra hoặc tải bản cập nhật ở nền...",
                    "Thu gọn",
                    NoticePrimaryRole::Dismiss,
                    None,
                    NoticeSecondaryRole::Dismiss,
                    false,
                );
            }
            return;
        }
        if let Some(release) = self.available_update.clone() {
            self.show_update_available_notice(release);
            return;
        }
        self.show_notice(
            NoticeKind::Checking,
            "Kiểm tra cập nhật",
            format!("Đang kết nối GitHub Releases (hiện tại v{APP_VERSION})..."),
            "Thu gọn",
            NoticePrimaryRole::Dismiss,
            None,
            NoticeSecondaryRole::Dismiss,
            false,
        );
        self.trigger_update_check(false);
    }

    fn handle_update_event(&mut self) {
        let Some(rx) = self.update_rx.take() else {
            return;
        };
        let Ok(event) = rx.try_recv() else {
            self.update_rx = Some(rx);
            return;
        };
        self.update_in_progress = false;
        let failed_release = self.downloading_release.take();
        self.update_tray_tooltip();

        match event {
            UpdaterEvent::CheckUpToDate { silent } => {
                if !silent {
                    self.show_notice(
                        NoticeKind::UpToDate,
                        "Đã là bản mới nhất",
                        format!("Bạn đang sử dụng phiên bản mới nhất (v{APP_VERSION})."),
                        "Đã hiểu",
                        NoticePrimaryRole::Dismiss,
                        None,
                        NoticeSecondaryRole::Dismiss,
                        true,
                    );
                }
            }
            UpdaterEvent::CheckAvailable { release, silent } => {
                self.available_update = Some(release.clone());
                self.update_tray_tooltip();
                let allow_startup_popup = self.config.visible
                    && !self.fullscreen_active
                    && !matches!(
                        self.requested_mode,
                        IslandMode::Settings | IslandMode::AudioDevices | IslandMode::Notice
                    );
                if !silent || allow_startup_popup {
                    self.show_update_available_notice(release);
                }
            }
            UpdaterEvent::CheckError { message, silent } => {
                if !silent {
                    self.show_notice(
                        NoticeKind::Error,
                        "Không thể kiểm tra cập nhật",
                        message,
                        "Đóng",
                        NoticePrimaryRole::Dismiss,
                        Some("Thử lại"),
                        NoticeSecondaryRole::RetryCheck,
                        false,
                    );
                }
            }
            UpdaterEvent::DownloadReadyToRestart => {
                if let Err(err) = spawn_updated_process() {
                    let retry_role = failed_release.map_or(
                        NoticeSecondaryRole::RetryCheck,
                        NoticeSecondaryRole::RetryDownload,
                    );
                    self.show_notice(
                        NoticeKind::Error,
                        "Không thể khởi chạy bản mới",
                        err,
                        "Đóng",
                        NoticePrimaryRole::Dismiss,
                        Some("Thử lại"),
                        retry_role,
                        false,
                    );
                    return;
                }
                self.shutdown_and_destroy();
            }
            UpdaterEvent::DownloadError { message } => {
                let retry_role = failed_release.map_or(
                    NoticeSecondaryRole::RetryCheck,
                    NoticeSecondaryRole::RetryDownload,
                );
                self.show_notice(
                    NoticeKind::Error,
                    "Tải bản cập nhật thất bại",
                    message,
                    "Đóng",
                    NoticePrimaryRole::Dismiss,
                    Some("Thử lại"),
                    retry_role,
                    false,
                );
            }
        }
    }

    fn client_dip_from_lparam(&self, lparam: LPARAM) -> (f32, f32) {
        let px_x = (lparam.0 & 0xFFFF) as i16 as f32;
        let px_y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
        let scale = self.dpi_scale.max(0.5);
        (px_x / scale, px_y / scale)
    }

    fn hit_test_action(&mut self, x_dip: f32, y_dip: f32) -> Option<UiAction> {
        let pill = self.animator.current_pill();
        let mode = self.current_effective_mode();
        if mode == IslandMode::Compact && matches!(self.active_hud, Some(HudContent::Volume { .. }))
        {
            fill_volume_hud_hitboxes(pill, &mut self.hitbox_scratch);
        } else if mode == IslandMode::AudioDevices {
            let visible = self
                .audio_devices
                .len()
                .saturating_sub(self.audio_device_scroll)
                .min(MAX_AUDIO_DEVICES_VISIBLE);
            fill_audio_devices_hitboxes(pill, visible, &mut self.hitbox_scratch);
        } else if mode == IslandMode::Notice {
            let has_secondary = self
                .active_notice
                .as_ref()
                .is_some_and(|n| n.secondary_label.is_some());
            fill_notice_hitboxes(pill, has_secondary, &mut self.hitbox_scratch);
        } else {
            fill_interactive_hitboxes(pill, mode, &mut self.hitbox_scratch);
        }
        self.hitbox_scratch
            .iter()
            .find(|hb| hb.rect.contains(x_dip, y_dip))
            .map(|hb| hb.action)
    }

    fn current_volume_track_rect(&mut self) -> Option<UiRectDip> {
        if !matches!(self.active_hud, Some(HudContent::Volume { .. }))
            || self.current_effective_mode() != IslandMode::Compact
        {
            return None;
        }
        let pill = self.animator.current_pill();
        fill_volume_hud_hitboxes(pill, &mut self.hitbox_scratch);
        self.hitbox_scratch
            .iter()
            .find(|hb| hb.action == UiAction::SeekVolume)
            .map(|hb| hb.rect)
    }

    fn schedule_hud_expire(&self) {
        unsafe {
            let _ = SetTimer(
                Some(self.hwnd),
                TIMER_HUD_EXPIRE_ID,
                HUD_EXPIRE_DELAY_MS,
                None,
            );
        }
    }

    fn cancel_hud_expire(&self) {
        let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_HUD_EXPIRE_ID) };
    }

    fn clear_hud_state_quiet(&mut self) {
        self.cancel_hud_expire();
        self.active_hud = None;
        self.dragging_volume = false;
        self.volume_drag_ratio = None;
    }

    fn clear_hud(&mut self) {
        let had_hud = self.active_hud.is_some();
        self.clear_hud_state_quiet();
        if had_hud {
            self.sync_pointer_presence();
            self.hovered_action = None;
            if self.mouse_tracked {
                // Chuột đang nằm trên viên thuốc khi HUD tắt: giữ Compact để không tụt hẫng dưới con trỏ.
                if self.requested_mode == IslandMode::Slim {
                    self.requested_mode = IslandMode::Compact;
                }
            } else if self.requested_mode == IslandMode::Compact
                && (self.config.notch_collapse_after_4s || !self.media_state.has_session)
            {
                self.requested_mode = IslandMode::Slim;
            }
            self.apply_target_and_render();
        }
    }

    fn show_hud(&mut self, hud: HudContent) {
        if !self.config.visible || (self.config.auto_hide_fullscreen && self.fullscreen_active) {
            return;
        }
        if matches!(
            self.requested_mode,
            IslandMode::Expanded
                | IslandMode::Settings
                | IslandMode::AudioDevices
                | IslandMode::Notice
        ) {
            return;
        }
        self.cancel_compact_to_notch();
        self.active_hud = Some(hud);
        if self.dragging_volume {
            self.cancel_hud_expire();
        } else {
            // Luôn đặt hạn 2 giây để HUD tự thu về nhạc khi người dùng ngừng cuộn dù con trỏ đứng yên trên pill.
            self.schedule_hud_expire();
        }
        self.apply_target_and_render();
    }

    fn handle_hud_expire_timer(&mut self) {
        self.cancel_hud_expire();
        if self.dragging_volume {
            return;
        }
        self.clear_hud();
    }

    fn handle_hud_worker_event(&mut self) {
        let Some(mon) = self.audio_monitor.as_ref() else {
            return;
        };
        let mut latest_sync = None;
        let mut latest_devices: Option<Vec<AudioOutputDevice>> = None;
        let mut latest_power: Option<(crate::hud::PowerSnapshot, bool)> = None;
        let mut unavailable = false;
        while let Some(ev) = mon.try_recv() {
            match ev {
                HudWorkerEvent::VolumeSync {
                    level,
                    muted,
                    device_name,
                    switched_device,
                    show_hud,
                } => {
                    latest_sync = Some((level, muted, device_name, switched_device, show_hud));
                    unavailable = false;
                }
                HudWorkerEvent::DevicesChanged { devices } => {
                    latest_devices = Some(devices);
                }
                HudWorkerEvent::PowerSync { snap, initial } => {
                    latest_power = Some((snap, initial));
                }
                HudWorkerEvent::AudioUnavailable => {
                    unavailable = true;
                }
            }
        }
        if let Some(devices) = latest_devices {
            if let Some(def) = devices.iter().find(|d| d.is_default) {
                self.current_audio_device_name = Some(def.name.clone());
            } else if devices.is_empty() {
                self.current_audio_device_name = None;
            }
            let max_scroll = devices.len().saturating_sub(MAX_AUDIO_DEVICES_VISIBLE);
            self.audio_device_scroll = self.audio_device_scroll.min(max_scroll);
            self.audio_devices = devices;
            if latest_sync.is_none() {
                let mode = self.current_effective_mode();
                if mode == IslandMode::AudioDevices {
                    self.apply_target_and_render();
                } else if mode == IslandMode::Expanded && !self.animator.is_animating() {
                    self.render_current_frame();
                }
            }
        }
        if let Some((snap, initial)) = latest_power {
            if initial {
                if self.power_tracker.last_snapshot().is_none() {
                    let _ = self.power_tracker.evaluate_update(snap);
                }
            } else if let Some((next, low_battery)) = self.power_tracker.evaluate_update(snap)
                && self.config.hud_power_enabled
            {
                self.show_hud(HudContent::Power {
                    on_ac: next.on_ac,
                    percent: next.percent,
                    low_battery,
                });
            } else if let Some(HudContent::Power {
                on_ac,
                percent,
                low_battery,
            }) = self.active_hud
                && on_ac == snap.on_ac
                && percent != snap.percent
            {
                self.active_hud = Some(HudContent::Power {
                    on_ac: snap.on_ac,
                    percent: snap.percent,
                    low_battery,
                });
                self.request_smooth_render();
            }
        }
        if unavailable && latest_sync.is_none() {
            self.volume_snapshot.available = false;
            return;
        }
        if let Some((level, muted, device_name, switched_device, show_hud)) = latest_sync {
            if let Some(name) = device_name {
                self.current_audio_device_name = Some(name);
            }
            let holding = self.dragging_volume
                || self
                    .volume_hold_until
                    .is_some_and(|until| Instant::now() < until);
            if !holding {
                self.volume_snapshot = VolumeSnapshot {
                    level,
                    muted,
                    available: true,
                };
                if show_hud && self.config.hud_volume_enabled {
                    self.show_hud(HudContent::Volume {
                        level,
                        muted,
                        show_device_name: switched_device,
                    });
                } else if matches!(self.active_hud, Some(HudContent::Volume { .. })) {
                    self.active_hud = Some(HudContent::Volume {
                        level,
                        muted,
                        show_device_name: switched_device,
                    });
                    self.request_smooth_render();
                } else if matches!(
                    self.current_effective_mode(),
                    IslandMode::Expanded | IslandMode::AudioDevices
                ) && !self.animator.is_animating()
                {
                    self.render_current_frame();
                }
            } else {
                self.volume_snapshot.available = true;
            }
        }
    }

    fn select_audio_device_at(&mut self, index: usize) {
        let Some(target) = self.audio_devices.get(index).cloned() else {
            return;
        };
        if target.is_default {
            return;
        }
        for (idx, dev) in self.audio_devices.iter_mut().enumerate() {
            dev.is_default = idx == index;
        }
        self.current_audio_device_name = Some(target.name);
        if let Some(mon) = &self.audio_monitor {
            mon.send_command(AudioCommand::SetDefaultDevice(target.id));
        }
        if matches!(
            self.current_effective_mode(),
            IslandMode::AudioDevices | IslandMode::Expanded
        ) && !self.animator.is_animating()
        {
            self.render_current_frame();
        }
    }

    fn handle_power_ac_hint(&mut self, on_ac: bool) {
        if let Some((next, low_battery)) = self.power_tracker.evaluate_ac_hint(on_ac)
            && self.config.hud_power_enabled
        {
            self.show_hud(HudContent::Power {
                on_ac: next.on_ac,
                percent: next.percent,
                low_battery,
            });
        }
    }

    fn request_power_refresh(&self) {
        if let Some(mon) = &self.audio_monitor {
            mon.send_command(AudioCommand::RefreshPower);
        }
    }

    fn apply_local_volume(&mut self, ratio: f32, unmute_if_positive: bool) {
        let clamped = ratio.clamp(0.0, 1.0);
        if unmute_if_positive && clamped > 0.004 && self.volume_snapshot.muted {
            self.volume_snapshot.muted = false;
        }
        self.volume_snapshot.level = clamped;
        self.volume_hold_until = Some(Instant::now() + Duration::from_millis(VOLUME_ECHO_HOLD_MS));
        self.active_hud = Some(HudContent::Volume {
            level: self.volume_snapshot.level,
            muted: self.volume_snapshot.muted,
            show_device_name: false,
        });
        if let Some(mon) = &self.audio_monitor {
            mon.send_command(AudioCommand::SetVolume(clamped));
        }
    }

    fn handle_mouse_wheel(&mut self, wparam: WPARAM, lparam: LPARAM) -> bool {
        let mods = wparam.0 & 0xFFFF;
        if (mods & (MK_CONTROL_MASK | MK_SHIFT_MASK | MK_LBUTTON_MASK | MK_RBUTTON_MASK)) != 0 {
            return false;
        }

        let mut pt = POINT {
            x: (lparam.0 & 0xFFFF) as i16 as i32,
            y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
        };
        if !unsafe { ScreenToClient(self.hwnd, &mut pt) }.as_bool() {
            return false;
        }
        let scale = self.dpi_scale.max(0.5);
        let x_dip = pt.x as f32 / scale;
        let y_dip = pt.y as f32 / scale;
        if !pill_contains_dip(self.animator.current_pill(), x_dip, y_dip, 2.0) {
            return false;
        }

        let mode = self.current_effective_mode();
        let delta = ((wparam.0 >> 16) & 0xFFFF) as i16;
        if mode == IslandMode::AudioDevices {
            self.ensure_mouse_tracked();
            let max_scroll = self
                .audio_devices
                .len()
                .saturating_sub(MAX_AUDIO_DEVICES_VISIBLE);
            if max_scroll > 0 && delta != 0 {
                let next_scroll = if delta < 0 {
                    (self.audio_device_scroll + 1).min(max_scroll)
                } else {
                    self.audio_device_scroll.saturating_sub(1)
                };
                if next_scroll != self.audio_device_scroll {
                    self.audio_device_scroll = next_scroll;
                    self.hovered_action = self.hit_test_action(x_dip, y_dip);
                    if !self.animator.is_animating() {
                        self.render_current_frame();
                    }
                }
            }
            return true;
        }

        if !self.config.hud_volume_enabled
            || !self.config.hud_wheel_enabled
            || !self.volume_snapshot.available
        {
            return false;
        }
        if !matches!(mode, IslandMode::Slim | IslandMode::Compact)
            || matches!(
                self.requested_mode,
                IslandMode::Expanded
                    | IslandMode::Settings
                    | IslandMode::AudioDevices
                    | IslandMode::Notice
            )
        {
            return false;
        }

        self.ensure_mouse_tracked();
        let steps = self.wheel_accum.push_delta(delta);
        if steps != 0 {
            let next = apply_volume_step(self.volume_snapshot.level, steps);
            if steps > 0 && self.volume_snapshot.muted {
                self.volume_snapshot.muted = false;
            }
            self.volume_snapshot.level = next;
            self.volume_hold_until =
                Some(Instant::now() + Duration::from_millis(VOLUME_ECHO_HOLD_MS));
            if let Some(mon) = &self.audio_monitor {
                mon.send_command(AudioCommand::StepVolume(steps));
            }
            self.show_hud(HudContent::Volume {
                level: self.volume_snapshot.level,
                muted: self.volume_snapshot.muted,
                show_device_name: false,
            });
        } else if self.active_hud.is_some() {
            self.schedule_hud_expire();
        }
        true
    }

    fn collapsed_rest_mode(&self) -> IslandMode {
        if self.media_state.has_session {
            IslandMode::Compact
        } else {
            IslandMode::Slim
        }
    }

    fn schedule_auto_collapse(&self, delay_ms: u32) {
        unsafe {
            let _ = SetTimer(Some(self.hwnd), TIMER_AUTO_COLLAPSE_ID, delay_ms, None);
        }
    }

    fn cancel_auto_collapse(&self) {
        let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_AUTO_COLLAPSE_ID) };
    }

    fn schedule_compact_to_notch(&self) {
        unsafe {
            let _ = SetTimer(
                Some(self.hwnd),
                TIMER_COMPACT_TO_NOTCH_ID,
                COMPACT_TO_NOTCH_DELAY_MS,
                None,
            );
        }
    }

    fn cancel_compact_to_notch(&self) {
        let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_COMPACT_TO_NOTCH_ID) };
    }

    fn handle_compact_to_notch_timer(&mut self) {
        self.cancel_compact_to_notch();
        if !self.mouse_tracked
            && !self.dragging_timeline
            && self.requested_mode == IslandMode::Compact
            && (self.config.notch_collapse_after_4s || !self.media_state.has_session)
        {
            self.requested_mode = IslandMode::Slim;
            self.hovered_action = None;
            self.apply_target_and_render();
        }
    }

    fn ensure_mouse_tracked(&mut self) {
        if !self.mouse_tracked {
            let mut tme = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: self.hwnd,
                dwHoverTime: 0,
            };
            if unsafe { TrackMouseEvent(&mut tme) }.is_ok() {
                self.mouse_tracked = true;
            }
        }
    }

    fn current_timeline_track_rect(&mut self) -> Option<UiRectDip> {
        if !self.media_state.has_session || !self.media_state.timeline.available {
            return None;
        }
        let pill = self.animator.current_pill();
        let mode = self.current_effective_mode();
        fill_interactive_hitboxes(pill, mode, &mut self.hitbox_scratch);
        self.hitbox_scratch
            .iter()
            .find(|hb| hb.action == UiAction::SeekTimeline)
            .map(|hb| hb.rect)
    }

    fn handle_auto_collapse_timer(&mut self) {
        self.cancel_auto_collapse();
        if !self.mouse_tracked && !self.dragging_timeline && self.requested_mode == IslandMode::Expanded
        {
            self.requested_mode =
                if self.config.notch_collapse_after_4s || !self.media_state.has_session {
                    IslandMode::Slim
                } else {
                    self.collapsed_rest_mode()
                };
            self.hovered_action = None;
            self.apply_target_and_render();
        }
    }

    fn handle_left_button_down(&mut self, lparam: LPARAM) -> CaptureChange {
        let (x_dip, y_dip) = self.client_dip_from_lparam(lparam);
        let pill = self.animator.current_pill();
        if !pill_contains_dip(pill, x_dip, y_dip, 2.0) {
            return CaptureChange::None;
        }
        self.ensure_mouse_tracked();
        self.cancel_auto_collapse();
        self.cancel_compact_to_notch();
        self.cancel_hud_expire();
        self.pointer_down_pos = Some((x_dip, y_dip));
        self.pressed_action = None;

        if let Some(track) = self.current_volume_track_rect()
            && volume_seek_hit_bounds(track).contains(x_dip, y_dip)
        {
            let ratio = timeline_ratio_for_x(track, x_dip);
            self.dragging_volume = true;
            self.volume_drag_ratio = Some(ratio);
            self.drag_render_dirty = false;
            self.hovered_action = Some(UiAction::SeekVolume);
            self.apply_local_volume(ratio, ratio > 0.004);
            unsafe {
                SetCursor(Some(self.cursor_hand));
            }
            self.update_vsync_activity();
            self.render_current_frame();
            return CaptureChange::Acquire;
        }

        if self.current_effective_mode() == IslandMode::Expanded
            && let Some(track) = self.current_timeline_track_rect()
            && timeline_seek_hit_bounds(track).contains(x_dip, y_dip)
        {
            let ratio = timeline_ratio_for_x(track, x_dip);
            self.dragging_timeline = true;
            self.timeline_drag_ratio = Some(ratio);
            self.drag_render_dirty = false;
            self.hovered_action = Some(UiAction::SeekTimeline);
            unsafe {
                SetCursor(Some(self.cursor_hand));
            }
            self.update_vsync_activity();
            self.render_current_frame();
            return CaptureChange::Acquire;
        }

        self.pressed_action = self.hit_test_action(x_dip, y_dip);
        CaptureChange::None
    }

    fn handle_mouse_move(&mut self, lparam: LPARAM) {
        self.cancel_auto_collapse();
        self.cancel_compact_to_notch();
        self.cancel_notice_expire();
        let (x_dip, y_dip) = self.client_dip_from_lparam(lparam);

        if self.dragging_volume {
            self.cancel_hud_expire();
            if let Some(track) = self.current_volume_track_rect() {
                let ratio = timeline_ratio_for_x(track, x_dip);
                let changed = self.volume_drag_ratio.is_none_or(|prev| {
                    (prev - ratio).abs() >= 0.004 || ratio == 0.0 || ratio == 1.0
                });
                if changed {
                    self.volume_drag_ratio = Some(ratio);
                    self.apply_local_volume(ratio, ratio > 0.004);
                    self.hovered_action = Some(UiAction::SeekVolume);
                    unsafe {
                        SetCursor(Some(self.cursor_hand));
                    }
                    self.request_smooth_render();
                }
            }
            return;
        }

        if self.dragging_timeline {
            if let Some(track) = self.current_timeline_track_rect() {
                let ratio = timeline_ratio_for_x(track, x_dip);
                if self.timeline_drag_ratio != Some(ratio) {
                    self.timeline_drag_ratio = Some(ratio);
                    self.hovered_action = Some(UiAction::SeekTimeline);
                    unsafe {
                        SetCursor(Some(self.cursor_hand));
                    }
                    self.request_smooth_render();
                }
            }
            return;
        }

        if self.requested_mode == IslandMode::Slim
            && !pill_contains_dip(self.animator.current_pill(), x_dip, y_dip, 2.0)
        {
            return;
        }

        self.ensure_mouse_tracked();
        if self.active_hud.is_none() && self.requested_mode == IslandMode::Slim {
            self.requested_mode = IslandMode::Compact;
            self.apply_target_and_render();
        }

        let next_hover = self.hit_test_action(x_dip, y_dip);
        if next_hover != self.hovered_action {
            self.hovered_action = next_hover;
            if matches!(
                next_hover,
                Some(UiAction::ToggleVolumeMute | UiAction::SeekVolume)
            ) {
                self.schedule_hud_expire();
            }
            let cursor = if next_hover.is_some() {
                self.cursor_hand
            } else {
                self.cursor_arrow
            };
            unsafe {
                SetCursor(Some(cursor));
            }
            if !self.animator.is_animating() {
                self.render_current_frame();
            }
        }
    }

    fn handle_mouse_leave(&mut self) {
        if self.dragging_timeline || self.dragging_volume {
            return;
        }
        self.mouse_tracked = false;
        self.wheel_accum.reset();
        let hover_changed = self.hovered_action.take().is_some();
        if self.active_hud.is_some() {
            self.schedule_hud_expire();
            if hover_changed && !self.animator.is_animating() {
                self.render_current_frame();
            }
            return;
        }
        if self.requested_mode == IslandMode::Compact {
            if !self.media_state.has_session {
                self.cancel_compact_to_notch();
                self.requested_mode = IslandMode::Slim;
                self.apply_target_and_render();
            } else {
                if self.config.notch_collapse_after_4s {
                    self.schedule_compact_to_notch();
                }
                if hover_changed && !self.animator.is_animating() {
                    self.render_current_frame();
                }
            }
        } else if self.requested_mode == IslandMode::Expanded {
            self.schedule_auto_collapse(AUTO_COLLAPSE_DELAY_MS);
            if hover_changed && !self.animator.is_animating() {
                self.render_current_frame();
            }
        } else if self.requested_mode == IslandMode::Notice {
            if self.active_notice.as_ref().is_some_and(|n| n.auto_expire) {
                self.schedule_notice_expire();
            }
            if hover_changed && !self.animator.is_animating() {
                self.render_current_frame();
            }
        } else if hover_changed && !self.animator.is_animating() {
            self.render_current_frame();
        }
    }

    fn handle_capture_changed(&mut self) {
        if !self.dragging_timeline && !self.dragging_volume {
            return;
        }
        let was_hud = self.active_hud.is_some();
        self.dragging_timeline = false;
        self.timeline_drag_ratio = None;
        self.dragging_volume = false;
        self.volume_drag_ratio = None;
        self.drag_render_dirty = false;
        self.pointer_down_pos = None;
        self.pressed_action = None;
        self.mouse_tracked = false;
        self.hovered_action = None;
        if was_hud {
            self.schedule_hud_expire();
        } else if self.requested_mode == IslandMode::Expanded {
            self.schedule_auto_collapse(AUTO_COLLAPSE_DELAY_MS);
        }
        self.update_vsync_activity();
        self.render_current_frame();
    }

    fn handle_left_button_up(&mut self, lparam: LPARAM) -> (CaptureChange, bool) {
        let (x_dip, y_dip) = self.client_dip_from_lparam(lparam);
        let down_pos = self.pointer_down_pos.take();
        let down_action = self.pressed_action.take();

        if self.dragging_volume {
            self.dragging_volume = false;
            self.drag_render_dirty = false;
            let fallback_ratio = self.volume_drag_ratio.take().unwrap_or(self.volume_snapshot.level);
            let ratio = self
                .current_volume_track_rect()
                .map_or(fallback_ratio, |track| timeline_ratio_for_x(track, x_dip));
            self.apply_local_volume(ratio, ratio > 0.004);

            let pill = self.animator.current_pill();
            let inside_pill = pill_contains_dip(pill, x_dip, y_dip, 0.0);
            self.mouse_tracked = false;
            if inside_pill {
                self.hovered_action = self.hit_test_action(x_dip, y_dip);
            } else {
                self.hovered_action = None;
            }
            self.schedule_hud_expire();
            self.update_vsync_activity();
            self.render_current_frame();
            return (CaptureChange::Release, inside_pill);
        }

        if self.dragging_timeline {
            self.dragging_timeline = false;
            self.drag_render_dirty = false;
            let fallback_ratio = self.timeline_drag_ratio.take().unwrap_or(0.0);
            let ratio = self
                .current_timeline_track_rect()
                .map_or(fallback_ratio, |track| timeline_ratio_for_x(track, x_dip));
            self.commit_timeline_seek_ratio(ratio);

            let pill = self.animator.current_pill();
            let inside_pill = pill_contains_dip(pill, x_dip, y_dip, 0.0);
            self.mouse_tracked = false;
            if inside_pill {
                self.hovered_action = self.hit_test_action(x_dip, y_dip);
                self.cancel_auto_collapse();
            } else {
                self.hovered_action = None;
                if self.requested_mode == IslandMode::Expanded {
                    self.schedule_auto_collapse(AUTO_COLLAPSE_DELAY_MS);
                }
            }
            self.update_vsync_activity();
            self.render_current_frame();
            return (CaptureChange::Release, inside_pill);
        }

        let up_action = self.hit_test_action(x_dip, y_dip);
        if let Some(pressed) = down_action {
            if up_action == Some(pressed) {
                if pressed == UiAction::SeekTimeline {
                    self.handle_timeline_seek(x_dip);
                    return (CaptureChange::None, false);
                }
                if pressed == UiAction::SeekVolume {
                    if let Some(track) = self.current_volume_track_rect() {
                        let ratio = timeline_ratio_for_x(track, x_dip);
                        self.apply_local_volume(ratio, ratio > 0.004);
                        self.render_current_frame();
                    }
                    return (CaptureChange::None, false);
                }
                self.execute_ui_action(pressed);
            }
            // Nếu bấm vào một nút nhưng nhả chuột trượt ra ngoài nút -> bỏ qua, không thu gọn nhầm.
            return (CaptureChange::None, false);
        }

        if let Some(action) = up_action {
            if action == UiAction::SeekTimeline {
                self.handle_timeline_seek(x_dip);
                return (CaptureChange::None, false);
            }
            if action == UiAction::SeekVolume {
                if let Some(track) = self.current_volume_track_rect() {
                    let ratio = timeline_ratio_for_x(track, x_dip);
                    self.apply_local_volume(ratio, ratio > 0.004);
                    self.render_current_frame();
                }
                return (CaptureChange::None, false);
            }
            self.execute_ui_action(action);
            return (CaptureChange::None, false);
        }

        let pill = self.animator.current_pill();
        if !pill_contains_dip(pill, x_dip, y_dip, 2.0) {
            return (CaptureChange::None, false);
        }

        if self.active_hud.is_some() {
            self.clear_hud();
            return (CaptureChange::None, false);
        }

        if matches!(
            self.requested_mode,
            IslandMode::Settings | IslandMode::AudioDevices | IslandMode::Notice
        ) {
            // Không tự đóng bảng Cài đặt / Thiết bị âm thanh / Thông báo khi bấm trượt khoảng trống.
            return (CaptureChange::None, false);
        }

        if self.requested_mode == IslandMode::Expanded
            && !should_collapse_expanded_on_blank_click(
                pill,
                down_pos.unwrap_or((x_dip, y_dip)),
                (x_dip, y_dip),
            )
        {
            return (CaptureChange::None, false);
        }

        self.requested_mode = match self.requested_mode {
            IslandMode::Slim | IslandMode::Compact => {
                self.cancel_compact_to_notch();
                IslandMode::Expanded
            }
            IslandMode::Expanded => {
                self.cancel_auto_collapse();
                self.collapsed_rest_mode()
            }
            IslandMode::Settings => IslandMode::Settings,
            IslandMode::AudioDevices => IslandMode::AudioDevices,
            IslandMode::Notice => IslandMode::Notice,
            IslandMode::Hidden => IslandMode::Slim,
        };
        if self.requested_mode == IslandMode::Expanded && !self.mouse_tracked {
            self.schedule_auto_collapse(AUTO_COLLAPSE_DELAY_MS);
        } else if self.requested_mode == IslandMode::Compact
            && !self.mouse_tracked
            && self.config.notch_collapse_after_4s
        {
            self.schedule_compact_to_notch();
        }
        self.hovered_action = None;
        self.apply_target_and_render();
        (CaptureChange::None, false)
    }

    fn commit_timeline_seek_ratio(&mut self, ratio: f32) {
        let Some(target_ticks) = self
            .media_state
            .timeline
            .seek_ticks_for_ratio(ratio.clamp(0.0, 1.0))
        else {
            return;
        };
        self.media_state.timeline.base_position_ticks = target_ticks;
        self.media_state.timeline.last_updated_filetime_ticks = current_windows_filetime_ticks();
        self.seek_hold_until = Some(Instant::now() + Duration::from_millis(TIMELINE_SEEK_HOLD_MS));
        if let Some(worker) = &self.media_worker {
            worker.send_command(MediaCommand::Seek(target_ticks));
        }
    }

    fn handle_timeline_seek(&mut self, x_dip: f32) {
        let Some(track) = self.current_timeline_track_rect() else {
            return;
        };
        let ratio = timeline_ratio_for_x(track, x_dip);
        self.commit_timeline_seek_ratio(ratio);
        self.render_current_frame();
    }

    fn normalized_subview_return_mode(&self, saved: IslandMode) -> IslandMode {
        match saved {
            IslandMode::Expanded => IslandMode::Expanded,
            IslandMode::Settings => IslandMode::Settings,
            _ => self.collapsed_rest_mode(),
        }
    }

    fn apply_return_mode(&mut self, next: IslandMode) {
        self.requested_mode = next;
        if next == IslandMode::Expanded && !self.mouse_tracked {
            self.schedule_auto_collapse(AUTO_COLLAPSE_DELAY_MS);
        } else if next == IslandMode::Compact
            && !self.mouse_tracked
            && self.config.notch_collapse_after_4s
        {
            self.schedule_compact_to_notch();
        }
    }

    fn handle_right_click(&mut self) {
        self.clear_hud_state_quiet();
        self.cancel_compact_to_notch();
        if self.requested_mode == IslandMode::Settings {
            let next = self.normalized_subview_return_mode(self.settings_return_mode);
            self.apply_return_mode(next);
        } else {
            self.cancel_auto_collapse();
            self.settings_return_mode = match self.requested_mode {
                IslandMode::Expanded => IslandMode::Expanded,
                IslandMode::AudioDevices => {
                    self.normalized_subview_return_mode(self.audio_devices_return_mode)
                }
                _ => self.collapsed_rest_mode(),
            };
            self.requested_mode = IslandMode::Settings;
        }
        self.hovered_action = None;
        self.apply_target_and_render();
    }

    fn execute_ui_action(&mut self, action: UiAction) {
        match action {
            UiAction::OpenSettings => {
                self.clear_hud_state_quiet();
                self.dismiss_notice_state_quiet();
                self.cancel_compact_to_notch();
                self.cancel_auto_collapse();
                self.settings_return_mode = match self.requested_mode {
                    IslandMode::Expanded => IslandMode::Expanded,
                    IslandMode::AudioDevices => {
                        self.normalized_subview_return_mode(self.audio_devices_return_mode)
                    }
                    _ => self.collapsed_rest_mode(),
                };
                self.hovered_action = None;
                self.requested_mode = IslandMode::Settings;
            }
            UiAction::OpenAudioDevices => {
                self.clear_hud_state_quiet();
                self.dismiss_notice_state_quiet();
                self.cancel_compact_to_notch();
                self.cancel_auto_collapse();
                self.audio_devices_return_mode = match self.requested_mode {
                    IslandMode::Settings | IslandMode::Expanded => self.requested_mode,
                    _ => self.collapsed_rest_mode(),
                };
                self.hovered_action = None;
                self.requested_mode = IslandMode::AudioDevices;
            }
            UiAction::SelectAudioDevice(visible_idx) => {
                self.select_audio_device_at(self.audio_device_scroll + visible_idx);
                return;
            }
            UiAction::NoticePrimary => {
                let role = self
                    .active_notice
                    .as_ref()
                    .map(|n| match &n.primary_role {
                        NoticePrimaryRole::Dismiss => None,
                        NoticePrimaryRole::InstallUpdate(rel) => Some(rel.clone()),
                    })
                    .unwrap_or(None);
                match role {
                    Some(release) => self.start_update_download(release),
                    None => self.dismiss_notice(),
                }
                return;
            }
            UiAction::NoticeSecondary => {
                let role = self.active_notice.as_ref().map(|n| match &n.secondary_role {
                    NoticeSecondaryRole::Dismiss => (false, None),
                    NoticeSecondaryRole::RetryCheck => (true, None),
                    NoticeSecondaryRole::RetryDownload(rel) => (false, Some(rel.clone())),
                });
                match role {
                    Some((true, _)) => self.handle_check_update_command(),
                    Some((false, Some(rel))) => self.start_update_download(rel),
                    _ => self.dismiss_notice(),
                }
                return;
            }
            UiAction::BackToMedia => {
                self.hovered_action = None;
                let saved = match self.requested_mode {
                    IslandMode::AudioDevices => self.audio_devices_return_mode,
                    IslandMode::Settings => self.settings_return_mode,
                    _ => self.collapsed_rest_mode(),
                };
                let next = self.normalized_subview_return_mode(saved);
                self.apply_return_mode(next);
            }
            UiAction::ToggleVolumeMute => {
                self.volume_snapshot.muted = !self.volume_snapshot.muted;
                if let Some(mon) = &self.audio_monitor {
                    mon.send_command(AudioCommand::ToggleMute);
                }
                self.show_hud(HudContent::Volume {
                    level: self.volume_snapshot.level,
                    muted: self.volume_snapshot.muted,
                    show_device_name: false,
                });
                return;
            }
            UiAction::ToggleHudVolume => {
                self.config.hud_volume_enabled = !self.config.hud_volume_enabled;
                if !self.config.hud_volume_enabled
                    && matches!(self.active_hud, Some(HudContent::Volume { .. }))
                {
                    self.clear_hud_state_quiet();
                }
                save_config(&self.config);
            }
            UiAction::ToggleHudWheel => {
                self.config.hud_wheel_enabled = !self.config.hud_wheel_enabled;
                self.wheel_accum.reset();
                save_config(&self.config);
            }
            UiAction::ToggleHudPower => {
                self.config.hud_power_enabled = !self.config.hud_power_enabled;
                if !self.config.hud_power_enabled
                    && matches!(self.active_hud, Some(HudContent::Power { .. }))
                {
                    self.clear_hud_state_quiet();
                }
                save_config(&self.config);
            }
            UiAction::ToggleNotchCollapse => {
                self.config.notch_collapse_after_4s = !self.config.notch_collapse_after_4s;
                if self.config.notch_collapse_after_4s {
                    if self.requested_mode == IslandMode::Compact && !self.mouse_tracked {
                        self.schedule_compact_to_notch();
                    }
                } else {
                    self.cancel_compact_to_notch();
                    if self.requested_mode == IslandMode::Slim && self.media_state.has_session {
                        self.requested_mode = IslandMode::Compact;
                    }
                }
                save_config(&self.config);
            }
            UiAction::ToggleAutoHideIdle => {
                self.config.auto_hide_when_idle = !self.config.auto_hide_when_idle;
                save_config(&self.config);
            }
            UiAction::ToggleAutoHideFullscreen => {
                self.config.auto_hide_fullscreen = !self.config.auto_hide_fullscreen;
                self.sync_fullscreen_timer();
                self.refresh_fullscreen_state();
                save_config(&self.config);
            }
            UiAction::ToggleVisualizer => {
                self.config.visualizer_enabled = !self.config.visualizer_enabled;
                save_config(&self.config);
            }
            UiAction::ToggleAutostart => {
                let target = !self.config.autostart;
                if set_autostart_enabled(target) {
                    self.config.autostart = target;
                }
            }
            UiAction::CheckUpdate => {
                self.handle_check_update_command();
                return;
            }
            UiAction::CycleMonitor => {
                if !self.monitors.is_empty() {
                    self.config.monitor_index =
                        (self.config.monitor_index + 1) % self.monitors.len();
                    self.reposition_for_selected_monitor();
                    self.refresh_fullscreen_state();
                    save_config(&self.config);
                }
            }
            UiAction::HideIsland => {
                self.clear_hud_state_quiet();
                self.cancel_compact_to_notch();
                let _ = self.reset_pointer_and_drag_state();
                self.config.visible = false;
                self.sync_fullscreen_timer();
                self.update_tray_tooltip();
                save_config(&self.config);
            }
            UiAction::ExitApp => {
                self.shutdown_and_destroy();
                return;
            }
            UiAction::MediaPrev => {
                if self.media_state.has_session
                    && let Some(worker) = &self.media_worker
                {
                    worker.send_command(MediaCommand::Previous);
                }
                return;
            }
            UiAction::MediaPlayPause => {
                if self.media_state.has_session
                    && let Some(worker) = &self.media_worker
                {
                    worker.send_command(MediaCommand::TogglePlayPause);
                }
                return;
            }
            UiAction::MediaNext => {
                if self.media_state.has_session
                    && let Some(worker) = &self.media_worker
                {
                    worker.send_command(MediaCommand::Next);
                }
                return;
            }
            UiAction::SeekTimeline | UiAction::SeekVolume => return,
        }
        self.hovered_action = None;
        self.apply_target_and_render();
    }

    fn release_resources(&mut self) {
        self.cancel_auto_collapse();
        self.cancel_compact_to_notch();
        self.cancel_hud_expire();
        self.cancel_notice_expire();
        let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_FULLSCREEN_CHECK_ID) };
        let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_TIMELINE_TICK_ID) };
        let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_STARTUP_UPDATE_ID) };
        if let Some(handle) = self.power_notify_handle.take() {
            unsafe {
                let _ = UnregisterPowerSettingNotification(handle);
            }
        }
        self.fg_hook.take();
        self.audio_monitor.take();
        self.media_worker.take();
        self.vsync.take();
    }

    fn shutdown_and_destroy(&mut self) {
        self.release_resources();
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

impl Drop for WindowController {
    fn drop(&mut self) {
        self.release_resources();
    }
}

fn get_controller_mut<'a>(hwnd: HWND) -> Option<&'a mut WindowController> {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut WindowController;
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { &mut *ptr })
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCCREATE => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        WM_APP_VSYNC_TICK => {
            if let Some(ctrl) = get_controller_mut(hwnd) {
                ctrl.handle_vsync_tick();
            }
            LRESULT(0)
        }
        WM_APP_MEDIA_UPDATE => {
            if let Some(ctrl) = get_controller_mut(hwnd) {
                ctrl.handle_media_update();
            }
            LRESULT(0)
        }
        WM_APP_HUD_EVENT => {
            if let Some(ctrl) = get_controller_mut(hwnd) {
                ctrl.handle_hud_worker_event();
            }
            LRESULT(0)
        }
        WM_POWERBROADCAST => {
            let event = wparam.0 as u32;
            if event == PBT_POWERSETTINGCHANGE && lparam.0 != 0 {
                let pbs = unsafe { &*(lparam.0 as *const POWERBROADCAST_SETTING) };
                if pbs.PowerSetting == GUID_ACDC_POWER_SOURCE && pbs.DataLength >= 1 {
                    let on_ac = pbs.Data[0] != 1;
                    if let Some(ctrl) = get_controller_mut(hwnd) {
                        ctrl.handle_power_ac_hint(on_ac);
                        ctrl.request_power_refresh();
                    }
                }
            } else if event == PBT_APMPOWERSTATUSCHANGE
                && let Some(ctrl) = get_controller_mut(hwnd)
            {
                ctrl.request_power_refresh();
            }
            LRESULT(1)
        }
        WM_APP_FOREGROUND_CHANGED => {
            let release_capture =
                get_controller_mut(hwnd).is_some_and(|ctrl| ctrl.refresh_fullscreen_state());
            if release_capture {
                unsafe {
                    let _ = ReleaseCapture();
                }
            }
            LRESULT(0)
        }
        WM_APP_WAKE_INSTANCE => {
            if let Some(ctrl) = get_controller_mut(hwnd) {
                ctrl.wake_from_duplicate_launch();
            }
            LRESULT(0)
        }
        WM_APP_UPDATE_EVENT => {
            if let Some(ctrl) = get_controller_mut(hwnd) {
                ctrl.handle_update_event();
            }
            LRESULT(0)
        }
        WM_TIMER => {
            let mut release_capture = false;
            if let Some(ctrl) = get_controller_mut(hwnd) {
                match wparam.0 {
                    TIMER_AUTO_COLLAPSE_ID => ctrl.handle_auto_collapse_timer(),
                    TIMER_COMPACT_TO_NOTCH_ID => ctrl.handle_compact_to_notch_timer(),
                    TIMER_HUD_EXPIRE_ID => ctrl.handle_hud_expire_timer(),
                    TIMER_NOTICE_EXPIRE_ID => ctrl.dismiss_notice(),
                    TIMER_FULLSCREEN_CHECK_ID => {
                        release_capture = ctrl.refresh_fullscreen_state();
                        if !release_capture {
                            ctrl.sync_pointer_presence();
                        }
                    }
                    TIMER_STARTUP_UPDATE_ID => {
                        let _ = unsafe { KillTimer(Some(hwnd), TIMER_STARTUP_UPDATE_ID) };
                        ctrl.trigger_update_check(true);
                    }
                    TIMER_TIMELINE_TICK_ID
                        if !ctrl.animator.is_animating()
                            && !ctrl.dragging_timeline
                            && matches!(
                                ctrl.current_effective_mode(),
                                IslandMode::Expanded | IslandMode::Slim | IslandMode::Compact
                            ) =>
                    {
                        ctrl.render_current_frame();
                    }
                    _ => {}
                }
            }
            if release_capture {
                unsafe {
                    let _ = ReleaseCapture();
                }
            }
            LRESULT(0)
        }
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_SETCURSOR => {
            if let Some(ctrl) = get_controller_mut(hwnd) {
                let cursor = if ctrl.dragging_timeline
                    || ctrl.dragging_volume
                    || ctrl.hovered_action.is_some()
                {
                    ctrl.cursor_hand
                } else {
                    ctrl.cursor_arrow
                };
                unsafe {
                    SetCursor(Some(cursor));
                }
                return LRESULT(1);
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_MOUSEWHEEL => {
            if let Some(ctrl) = get_controller_mut(hwnd)
                && ctrl.handle_mouse_wheel(wparam, lparam)
            {
                return LRESULT(0);
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_MOUSEMOVE => {
            if let Some(ctrl) = get_controller_mut(hwnd) {
                ctrl.handle_mouse_move(lparam);
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            if let Some(ctrl) = get_controller_mut(hwnd) {
                ctrl.handle_mouse_leave();
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let capture = get_controller_mut(hwnd)
                .map_or(CaptureChange::None, |ctrl| ctrl.handle_left_button_down(lparam));
            if capture == CaptureChange::Acquire {
                unsafe {
                    let _ = SetCapture(hwnd);
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let (capture, rearm_track) = get_controller_mut(hwnd).map_or(
                (CaptureChange::None, false),
                |ctrl| ctrl.handle_left_button_up(lparam),
            );
            if capture == CaptureChange::Release {
                unsafe {
                    let _ = ReleaseCapture();
                }
                if rearm_track
                    && let Some(ctrl) = get_controller_mut(hwnd)
                {
                    ctrl.ensure_mouse_tracked();
                }
            }
            LRESULT(0)
        }
        WM_CAPTURECHANGED => {
            if let Some(ctrl) = get_controller_mut(hwnd) {
                ctrl.handle_capture_changed();
            }
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            if let Some(ctrl) = get_controller_mut(hwnd) {
                ctrl.handle_right_click();
            }
            LRESULT(0)
        }
        WM_DPICHANGED | WM_DISPLAYCHANGE => {
            if let Some(ctrl) = get_controller_mut(hwnd) {
                ctrl.refresh_monitors();
                ctrl.refresh_fullscreen_state();
                ctrl.apply_target_and_render();
            }
            LRESULT(0)
        }
        WM_APP_TRAY => {
            let event = (lparam.0 & 0xFFFF) as u32;
            if let Some(ctrl) = get_controller_mut(hwnd) {
                match event {
                    WM_LBUTTONUP | WM_LBUTTONDBLCLK => ctrl.toggle_visibility_from_tray(),
                    WM_RBUTTONUP => {
                        ctrl.refresh_monitors();
                        let expanded = matches!(
                            ctrl.requested_mode,
                            IslandMode::Expanded
                                | IslandMode::Settings
                                | IslandMode::AudioDevices
                                | IslandMode::Notice
                        );
                        if let Some(tray) = &ctrl.tray {
                            let update_ver =
                                ctrl.available_update.as_ref().map(|r| r.version_tag.as_str());
                            tray.show_context_menu(
                                ctrl.config,
                                &ctrl.media_state,
                                &ctrl.monitors,
                                &ctrl.audio_devices,
                                expanded,
                                update_ver,
                            );
                        }
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let cmd_id = wparam.0 & 0xFFFF;
            if let Some(ctrl) = get_controller_mut(hwnd) {
                match cmd_id {
                    IDM_TOGGLE_VISIBLE => ctrl.toggle_visibility_from_tray(),
                    IDM_OPEN_SETTINGS => {
                        ctrl.dismiss_notice_state_quiet();
                        ctrl.cancel_compact_to_notch();
                        ctrl.cancel_auto_collapse();
                        ctrl.settings_return_mode = if ctrl.requested_mode == IslandMode::Expanded {
                            IslandMode::Expanded
                        } else {
                            ctrl.collapsed_rest_mode()
                        };
                        ctrl.config.visible = true;
                        ctrl.requested_mode = IslandMode::Settings;
                        ctrl.sync_fullscreen_timer();
                        ctrl.apply_target_and_render();
                        ctrl.update_tray_tooltip();
                        save_config(&ctrl.config);
                    }
                    IDM_TOGGLE_EXPANDED => {
                        ctrl.dismiss_notice_state_quiet();
                        ctrl.config.visible = true;
                        ctrl.requested_mode = match ctrl.requested_mode {
                            IslandMode::Expanded
                            | IslandMode::Settings
                            | IslandMode::AudioDevices
                            | IslandMode::Notice => {
                                ctrl.cancel_auto_collapse();
                                if ctrl.config.notch_collapse_after_4s
                                    || !ctrl.media_state.has_session
                                {
                                    IslandMode::Slim
                                } else {
                                    ctrl.collapsed_rest_mode()
                                }
                            }
                            _ => {
                                ctrl.cancel_compact_to_notch();
                                if !ctrl.mouse_tracked {
                                    ctrl.schedule_auto_collapse(AUTO_COLLAPSE_DELAY_MS);
                                }
                                IslandMode::Expanded
                            }
                        };
                        ctrl.sync_fullscreen_timer();
                        ctrl.apply_target_and_render();
                        ctrl.update_tray_tooltip();
                        save_config(&ctrl.config);
                    }
                    IDM_TOGGLE_NOTCH_COLLAPSE => {
                        ctrl.execute_ui_action(UiAction::ToggleNotchCollapse)
                    }
                    IDM_TOGGLE_AUTO_HIDE => ctrl.execute_ui_action(UiAction::ToggleAutoHideIdle),
                    IDM_TOGGLE_FULLSCREEN => {
                        ctrl.execute_ui_action(UiAction::ToggleAutoHideFullscreen)
                    }
                    IDM_TOGGLE_VISUALIZER => ctrl.execute_ui_action(UiAction::ToggleVisualizer),
                    IDM_TOGGLE_HUD_VOLUME => ctrl.execute_ui_action(UiAction::ToggleHudVolume),
                    IDM_TOGGLE_HUD_WHEEL => ctrl.execute_ui_action(UiAction::ToggleHudWheel),
                    IDM_TOGGLE_HUD_POWER => ctrl.execute_ui_action(UiAction::ToggleHudPower),
                    IDM_TOGGLE_AUTOSTART => ctrl.execute_ui_action(UiAction::ToggleAutostart),
                    IDM_CHECK_UPDATE => ctrl.handle_check_update_command(),
                    IDM_EXIT => ctrl.shutdown_and_destroy(),
                    id if (IDM_MONITOR_BASE..=IDM_MONITOR_MAX).contains(&id) => {
                        let idx = id - IDM_MONITOR_BASE;
                        if idx < ctrl.monitors.len() {
                            ctrl.config.monitor_index = idx;
                            ctrl.reposition_for_selected_monitor();
                            ctrl.refresh_fullscreen_state();
                            ctrl.apply_target_and_render();
                            save_config(&ctrl.config);
                        }
                    }
                    id if (IDM_AUDIO_DEVICE_BASE..=IDM_AUDIO_DEVICE_MAX).contains(&id) => {
                        ctrl.select_audio_device_at(id - IDM_AUDIO_DEVICE_BASE);
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            let ptr = unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) } as *mut WindowController;
            if !ptr.is_null() {
                unsafe {
                    drop(Box::from_raw(ptr));
                }
            }
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, PM_REMOVE, PeekMessageW, MSG, SPI_GETMOUSEWHEELROUTING, SM_CXSCREEN,
        SystemParametersInfoW, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };

    struct ComStaGuard(bool);
    impl Drop for ComStaGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }

    fn make_wheel_wparam(mods: usize, delta: i16) -> WPARAM {
        let hi = (delta as u16 as usize) << 16;
        WPARAM(hi | (mods & 0xFFFF))
    }

    fn make_lparam_xy(x: i32, y: i32) -> LPARAM {
        let lo = (x as i16 as u16) as usize;
        let hi = ((y as i16 as u16) as usize) << 16;
        LPARAM((lo | hi) as isize)
    }

    #[test]
    fn wasapi_audio_monitor_connects_and_shuts_down_cleanly() {
        let _com = ComStaGuard(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok());
        let hmodule = unsafe { GetModuleHandleW(None) }.expect("GetModuleHandleW");
        let cls = to_wide_null("STATIC");
        let title = to_wide_null("HudWasapiSmoke");
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                PCWSTR(cls.as_ptr()),
                PCWSTR(title.as_ptr()),
                WS_POPUP,
                0,
                0,
                16,
                16,
                None,
                None,
                Some(HINSTANCE::from(hmodule)),
                None,
            )
        }
        .expect("CreateWindowExW");

        let monitor = AudioMonitor::start(hwnd);
        let start = Instant::now();
        let mut received_audio = None;
        let mut received_power = false;
        let mut received_devices = false;
        while start.elapsed() < Duration::from_millis(1200) {
            while let Some(ev) = monitor.try_recv() {
                match ev {
                    HudWorkerEvent::PowerSync { initial, .. } => {
                        assert!(initial);
                        received_power = true;
                    }
                    HudWorkerEvent::DevicesChanged { .. } => {
                        received_devices = true;
                    }
                    other => {
                        received_audio = Some(other);
                    }
                }
            }
            if received_audio.is_some() && received_power {
                break;
            }
            std::thread::sleep(Duration::from_millis(15));
        }

        assert!(received_power, "AudioMonitor phai gui PowerSync khoi tao");
        let ev = received_audio.expect("AudioMonitor phai gui trang thai am thanh khoi tao");
        match ev {
            HudWorkerEvent::VolumeSync {
                level,
                show_hud,
                ..
            } => {
                assert!((0.0..=1.0).contains(&level));
                assert!(!show_hud, "Khoi dong khong duoc tu bat HUD gay phien");
                assert!(received_devices, "AudioMonitor phai gui DevicesChanged khi co endpoint");
            }
            HudWorkerEvent::AudioUnavailable => {}
            HudWorkerEvent::DevicesChanged { .. } | HudWorkerEvent::PowerSync { .. } => unreachable!(),
        }

        drop(monitor);
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
    }

    #[test]
    fn mouse_wheel_screen_coords_delta_settings_and_drag_integration() {
        std::thread::spawn(|| {
            let _com =
                ComStaGuard(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok());

            let mut wheel_routing: u32 = 0;
            let _ = unsafe {
                SystemParametersInfoW(
                    SPI_GETMOUSEWHEELROUTING,
                    0,
                    Some((&raw mut wheel_routing).cast()),
                    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
                )
            };
            println!(
                "Windows SPI_GETMOUSEWHEELROUTING={}, SM_CXSCREEN={}",
                wheel_routing,
                unsafe { GetSystemMetrics(SM_CXSCREEN) }
            );

            let hmodule = unsafe { GetModuleHandleW(None) }.expect("GetModuleHandleW");
            let hinstance: HINSTANCE = hmodule.into();
            let class_name = to_wide_null("WinDynamicIslandWheelTestClass");
            let window_title = to_wide_null("WheelTest");
            let cursor_arrow = unsafe { LoadCursorW(None, IDC_ARROW) }.expect("IDC_ARROW");
            let cursor_hand = unsafe { LoadCursorW(None, IDC_HAND) }.expect("IDC_HAND");

            let wc = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(window_proc),
                hInstance: hinstance,
                hCursor: cursor_arrow,
                lpszClassName: PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            assert_ne!(unsafe { RegisterClassExW(&wc) }, 0);

            let (surf_w, surf_h) = compute_surface_size_px(1.0);
            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP | WS_EX_NOACTIVATE,
                    PCWSTR(class_name.as_ptr()),
                    PCWSTR(window_title.as_ptr()),
                    WS_POPUP,
                    40,
                    0,
                    surf_w,
                    surf_h,
                    None,
                    None,
                    Some(hinstance),
                    None,
                )
            }
            .expect("CreateWindowExW");

            let renderer = IslandRenderer::new(hwnd, 1.0).expect("IslandRenderer::new");
            let config = AppConfig {
                hud_volume_enabled: true,
                hud_wheel_enabled: true,
                hud_power_enabled: true,
                ..Default::default()
            };
            let monitors = enumerate_monitors();

            let controller = Box::new(WindowController {
                hwnd,
                config,
                monitors,
                requested_mode: IslandMode::Slim,
                settings_return_mode: IslandMode::Expanded,
                audio_devices_return_mode: IslandMode::Expanded,
                media_state: MediaDisplayState::default(),
                fullscreen_active: false,
                mouse_tracked: false,
                dragging_timeline: false,
                timeline_drag_ratio: None,
                dragging_volume: false,
                volume_drag_ratio: None,
                drag_render_dirty: false,
                volume_hold_until: None,
                wheel_accum: WheelVolumeAccumulator::default(),
                pointer_down_pos: None,
                pressed_action: None,
                seek_hold_until: None,
                hovered_action: None,
                dpi_scale: 1.0,
                animator: IslandAnimator::new(IslandMode::Slim),
                renderer,
                vsync: None,
                media_worker: None,
                audio_monitor: None,
                audio_devices: Vec::new(),
                audio_device_scroll: 0,
                current_audio_device_name: None,
                volume_snapshot: VolumeSnapshot {
                    level: 0.50,
                    muted: false,
                    available: true,
                },
                power_tracker: PowerStateTracker::default(),
                power_notify_handle: None,
                active_hud: None,
                fg_hook: None,
                tray: None,
                last_region: None,
                last_window_rect: None,
                hitbox_scratch: Vec::with_capacity(12),
                visualizer_phase: 0.0,
                cursor_arrow,
                cursor_hand,
                update_rx: None,
                available_update: None,
                downloading_release: None,
                active_notice: None,
                update_in_progress: false,
            });
            unsafe {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(controller) as isize);
            }

            let pill_to_screen = |x_dip: f32, y_dip: f32| -> POINT {
                let mut pt = POINT {
                    x: x_dip.round() as i32,
                    y: y_dip.round() as i32,
                };
                assert!(unsafe { ClientToScreen(hwnd, &mut pt) }.as_bool());
                pt
            };

            // 1. Cuộn lên +120 tại tâm dải Slim -> tăng lên 53% (+3%) và bung HUD Compact.
            let slim_pill = get_controller_mut(hwnd).unwrap().animator.current_pill();
            let in_pill = pill_to_screen(slim_pill.left + slim_pill.width * 0.5, slim_pill.top + 8.0);
            unsafe {
                window_proc(
                    hwnd,
                    WM_MOUSEWHEEL,
                    make_wheel_wparam(0, 120),
                    make_lparam_xy(in_pill.x, in_pill.y),
                );
            }
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                assert!((ctrl.volume_snapshot.level - 0.53).abs() < 1e-4);
                assert!(matches!(ctrl.active_hud, Some(HudContent::Volume { .. })));
                assert_eq!(ctrl.current_effective_mode(), IslandMode::Compact);
            }

            // 2. Cuộn ngoài viên thuốc -> không đổi âm lượng.
            let out_pill = pill_to_screen(slim_pill.left - 80.0, slim_pill.top + 8.0);
            unsafe {
                window_proc(
                    hwnd,
                    WM_MOUSEWHEEL,
                    make_wheel_wparam(0, 120),
                    make_lparam_xy(out_pill.x, out_pill.y),
                );
            }
            assert!((get_controller_mut(hwnd).unwrap().volume_snapshot.level - 0.53).abs() < 1e-4);

            // 3. Ctrl + Wheel và Shift + Wheel không bị chặn làm chỉnh âm lượng.
            unsafe {
                window_proc(
                    hwnd,
                    WM_MOUSEWHEEL,
                    make_wheel_wparam(MK_CONTROL_MASK, 120),
                    make_lparam_xy(in_pill.x, in_pill.y),
                );
                window_proc(
                    hwnd,
                    WM_MOUSEWHEEL,
                    make_wheel_wparam(MK_SHIFT_MASK, -120),
                    make_lparam_xy(in_pill.x, in_pill.y),
                );
            }
            assert!((get_controller_mut(hwnd).unwrap().volume_snapshot.level - 0.53).abs() < 1e-4);

            // 4. Touchpad cuộn mịn +15 x 3 -> 54%; +30 rồi WM_MOUSELEAVE xóa tích lũy.
            unsafe {
                for _ in 0..3 {
                    window_proc(
                        hwnd,
                        WM_MOUSEWHEEL,
                        make_wheel_wparam(0, 15),
                        make_lparam_xy(in_pill.x, in_pill.y),
                    );
                }
            }
            assert!((get_controller_mut(hwnd).unwrap().volume_snapshot.level - 0.54).abs() < 1e-4);

            unsafe {
                window_proc(
                    hwnd,
                    WM_MOUSEWHEEL,
                    make_wheel_wparam(0, 30),
                    make_lparam_xy(in_pill.x, in_pill.y),
                );
                window_proc(hwnd, WM_MOUSELEAVE, WPARAM(0), LPARAM(0));
                window_proc(
                    hwnd,
                    WM_MOUSEWHEEL,
                    make_wheel_wparam(0, 15),
                    make_lparam_xy(in_pill.x, in_pill.y),
                );
            }
            assert!((get_controller_mut(hwnd).unwrap().volume_snapshot.level - 0.54).abs() < 1e-4);

            // 5. Kéo thanh SeekVolume trên HUD bằng SetCapture/ReleaseCapture.
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                ctrl.animator = IslandAnimator::new(IslandMode::Compact);
            }
            let compact_pill = get_controller_mut(hwnd).unwrap().animator.current_pill();
            let mut hud_hbs = Vec::with_capacity(2);
            fill_volume_hud_hitboxes(compact_pill, &mut hud_hbs);
            let slider = hud_hbs
                .iter()
                .find(|hb| hb.action == UiAction::SeekVolume)
                .unwrap()
                .rect;
            let start_x = (slider.left + slider.width() * 0.25).round() as i32;
            let end_x = (slider.left + slider.width() * 0.80).round() as i32;
            let mid_y = (slider.top + slider.height() * 0.5).round() as i32;
            unsafe {
                window_proc(hwnd, WM_LBUTTONDOWN, WPARAM(1), make_lparam_xy(start_x, mid_y));
                assert!(get_controller_mut(hwnd).unwrap().dragging_volume);
                window_proc(hwnd, WM_MOUSEMOVE, WPARAM(1), make_lparam_xy(end_x, mid_y));
                window_proc(hwnd, WM_LBUTTONUP, WPARAM(0), make_lparam_xy(end_x, mid_y));
            }
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                assert!(!ctrl.dragging_volume);
                assert!(ctrl.volume_snapshot.level > 0.72 && ctrl.volume_snapshot.level < 0.88);
            }

            // 6. Tắt setting Cuộn chỉnh loa -> cuộn chuột không đổi âm lượng nữa (không chạm config.ini thật).
            let prev_lvl = get_controller_mut(hwnd).unwrap().volume_snapshot.level;
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                ctrl.config.hud_wheel_enabled = false;
                ctrl.wheel_accum.reset();
                assert!(!ctrl.config.hud_wheel_enabled);
            }
            let cp = pill_to_screen(compact_pill.left + compact_pill.width * 0.5, compact_pill.top + 12.0);
            unsafe {
                window_proc(
                    hwnd,
                    WM_MOUSEWHEEL,
                    make_wheel_wparam(0, 120),
                    make_lparam_xy(cp.x, cp.y),
                );
            }
            assert!((get_controller_mut(hwnd).unwrap().volume_snapshot.level - prev_lvl).abs() < 1e-4);

            // 7. Khi đang ở Settings hoặc Expanded, bật hud_wheel_enabled lại nhưng không cho HUD nhảy đè.
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                ctrl.config.hud_wheel_enabled = true;
                ctrl.clear_hud_state_quiet();
                ctrl.requested_mode = IslandMode::Settings;
                ctrl.apply_target_and_render();
                assert_eq!(ctrl.current_effective_mode(), IslandMode::Settings);
                ctrl.show_hud(HudContent::Volume {
                    level: 0.30,
                    muted: false,
                    show_device_name: false,
                });
                assert_eq!(ctrl.current_effective_mode(), IslandMode::Settings);
                assert!(ctrl.active_hud.is_none());
            }

            // 8. Trong Settings: bấm trượt từ một thẻ sang khoảng trống không làm đóng Settings nhầm,
            //    và bấm Thiết bị phát -> Quay lại phải trở về đúng Settings.
            let settings_pill = {
                let ctrl = get_controller_mut(hwnd).unwrap();
                ctrl.animator = IslandAnimator::new(IslandMode::Settings);
                ctrl.animator.current_pill()
            };
            let mut settings_hbs = Vec::with_capacity(14);
            fill_interactive_hitboxes(settings_pill, IslandMode::Settings, &mut settings_hbs);
            let tile_rect = settings_hbs
                .iter()
                .find(|hb| hb.action == UiAction::ToggleHudWheel)
                .unwrap()
                .rect;
            let gap_x = (tile_rect.right + 3.0).round() as i32;
            let tile_mid_x = (tile_rect.left + tile_rect.width() * 0.5).round() as i32;
            let tile_mid_y = (tile_rect.top + tile_rect.height() * 0.5).round() as i32;
            unsafe {
                window_proc(hwnd, WM_LBUTTONDOWN, WPARAM(1), make_lparam_xy(tile_mid_x, tile_mid_y));
                window_proc(hwnd, WM_LBUTTONUP, WPARAM(0), make_lparam_xy(gap_x, tile_mid_y));
                window_proc(hwnd, WM_LBUTTONDOWN, WPARAM(1), make_lparam_xy(gap_x, tile_mid_y));
                window_proc(hwnd, WM_LBUTTONUP, WPARAM(0), make_lparam_xy(gap_x, tile_mid_y));
            }
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                assert_eq!(ctrl.current_effective_mode(), IslandMode::Settings);
                assert!(ctrl.config.hud_wheel_enabled);
                ctrl.execute_ui_action(UiAction::OpenAudioDevices);
                assert_eq!(ctrl.current_effective_mode(), IslandMode::AudioDevices);
                ctrl.execute_ui_action(UiAction::BackToMedia);
                assert_eq!(ctrl.current_effective_mode(), IslandMode::Settings);
            }

            // 9. Khi đang Slim và di chuột vào vùng đệm ngoài viên thuốc thật -> không bung nhầm lên Compact.
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                ctrl.requested_mode = IslandMode::Slim;
                ctrl.animator = IslandAnimator::new(IslandMode::Slim);
                ctrl.mouse_tracked = false;
            }
            let outside_slim_x = (slim_pill.left - 30.0).round() as i32;
            let outside_slim_y = (slim_pill.top + 8.0).round() as i32;
            unsafe {
                window_proc(hwnd, WM_MOUSEMOVE, WPARAM(0), make_lparam_xy(outside_slim_x, outside_slim_y));
            }
            assert_eq!(get_controller_mut(hwnd).unwrap().requested_mode, IslandMode::Slim);

            // 10. Khi đang giữ mouse_tracked=true ở Slim + HUD hết hạn -> giữ Compact chứ không tụt hẫng dưới con trỏ.
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                ctrl.mouse_tracked = true;
                ctrl.show_hud(HudContent::Volume {
                    level: 0.45,
                    muted: false,
                    show_device_name: false,
                });
                ctrl.mouse_tracked = true;
                // Giả lập chuột thực sự ở ngoài (cửa sổ test đang ẩn nên is_cursor_actually_over_pill=false):
                // test nhánh clear_hud khi mouse_tracked=false phải thu về Slim.
                ctrl.clear_hud();
                assert_eq!(ctrl.requested_mode, IslandMode::Slim);

                let need_release = ctrl.reset_pointer_and_drag_state();
                assert!(!need_release);
                assert!(!ctrl.mouse_tracked);
                assert!(ctrl.pressed_action.is_none());
            }

            // 11. Mở AudioDevices từ Expanded, cuộn danh sách > 5 thiết bị, chọn thiết bị tại chỗ và click khoảng trống không tự đóng.
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                ctrl.audio_devices = (0..7)
                    .map(|idx| AudioOutputDevice {
                        id: format!("dev-{idx}"),
                        name: format!("Speaker Device {idx}"),
                        is_default: idx == 0,
                    })
                    .collect();
                ctrl.current_audio_device_name = Some(String::from("Speaker Device 0"));
                ctrl.requested_mode = IslandMode::Expanded;
                ctrl.animator = IslandAnimator::new(IslandMode::Expanded);
                ctrl.execute_ui_action(UiAction::OpenAudioDevices);
                assert_eq!(ctrl.current_effective_mode(), IslandMode::AudioDevices);
                assert!((ctrl.animator.height.target - 293.0).abs() < 0.1);
                let mut t = Instant::now();
                for _ in 0..200 {
                    t += Duration::from_millis(8);
                    if !ctrl.animator.step(t) {
                        break;
                    }
                }
            }
            let dev_pill = get_controller_mut(hwnd).unwrap().animator.current_pill();
            let dev_pt = pill_to_screen(dev_pill.left + dev_pill.width * 0.5, dev_pill.top + 120.0);
            unsafe {
                window_proc(
                    hwnd,
                    WM_MOUSEWHEEL,
                    make_wheel_wparam(0, -120),
                    make_lparam_xy(dev_pt.x, dev_pt.y),
                );
            }
            assert_eq!(get_controller_mut(hwnd).unwrap().audio_device_scroll, 1);

            let mut dev_hbs = Vec::with_capacity(6);
            fill_audio_devices_hitboxes(dev_pill, MAX_AUDIO_DEVICES_VISIBLE, &mut dev_hbs);
            let row2 = dev_hbs
                .iter()
                .find(|hb| hb.action == UiAction::SelectAudioDevice(1))
                .unwrap()
                .rect;
            let row2_x = (row2.left + row2.width() * 0.5).round() as i32;
            let row2_y = (row2.top + row2.height() * 0.5).round() as i32;
            unsafe {
                window_proc(hwnd, WM_LBUTTONDOWN, WPARAM(1), make_lparam_xy(row2_x, row2_y));
                window_proc(hwnd, WM_LBUTTONUP, WPARAM(0), make_lparam_xy(row2_x, row2_y));
            }
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                assert_eq!(ctrl.current_effective_mode(), IslandMode::AudioDevices);
                assert!(ctrl.audio_devices[2].is_default);
                assert_eq!(
                    ctrl.current_audio_device_name.as_deref(),
                    Some("Speaker Device 2")
                );
            }
            let gap_dev_y = (row2.bottom + 2.0).round() as i32;
            unsafe {
                window_proc(hwnd, WM_LBUTTONDOWN, WPARAM(1), make_lparam_xy(row2_x, gap_dev_y));
                window_proc(hwnd, WM_LBUTTONUP, WPARAM(0), make_lparam_xy(row2_x, gap_dev_y));
            }
            assert_eq!(
                get_controller_mut(hwnd).unwrap().current_effective_mode(),
                IslandMode::AudioDevices
            );

            // 12. Thẻ thông báo Liquid Glass (Notice) thay MessageBoxW: checking -> up-to-date -> dismiss, update available -> để sau, lỗi -> đóng.
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                ctrl.requested_mode = IslandMode::Compact;
                ctrl.animator = IslandAnimator::new(IslandMode::Compact);
                let (up_tx, up_rx) = std::sync::mpsc::channel();
                up_tx
                    .send(UpdaterEvent::CheckUpToDate { silent: false })
                    .unwrap();
                ctrl.update_rx = Some(up_rx);
                ctrl.update_in_progress = true;
                ctrl.handle_update_event();
                assert_eq!(ctrl.current_effective_mode(), IslandMode::Notice);
                assert_eq!(
                    ctrl.active_notice.as_ref().map(|n| n.kind),
                    Some(NoticeKind::UpToDate)
                );
                assert!(ctrl.active_notice.as_ref().is_some_and(|n| n.auto_expire));
                ctrl.animator = IslandAnimator::new(IslandMode::Notice);
            }
            let notice_pill = get_controller_mut(hwnd).unwrap().animator.current_pill();
            let mut notice_hbs = Vec::with_capacity(2);
            fill_notice_hitboxes(notice_pill, false, &mut notice_hbs);
            let ok_btn = notice_hbs[0].rect;
            let ok_x = (ok_btn.left + ok_btn.width() * 0.5).round() as i32;
            let ok_y = (ok_btn.top + ok_btn.height() * 0.5).round() as i32;
            let blank_y = (notice_pill.top + 48.0).round() as i32;
            unsafe {
                window_proc(hwnd, WM_LBUTTONDOWN, WPARAM(1), make_lparam_xy(ok_x, blank_y));
                window_proc(hwnd, WM_LBUTTONUP, WPARAM(0), make_lparam_xy(ok_x, blank_y));
            }
            assert_eq!(
                get_controller_mut(hwnd).unwrap().current_effective_mode(),
                IslandMode::Notice
            );
            unsafe {
                window_proc(hwnd, WM_LBUTTONDOWN, WPARAM(1), make_lparam_xy(ok_x, ok_y));
                window_proc(hwnd, WM_LBUTTONUP, WPARAM(0), make_lparam_xy(ok_x, ok_y));
            }
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                assert!(ctrl.active_notice.is_none());
                assert_ne!(ctrl.current_effective_mode(), IslandMode::Notice);

                let (av_tx, av_rx) = std::sync::mpsc::channel();
                av_tx
                    .send(UpdaterEvent::CheckAvailable {
                        release: UpdateReleaseInfo {
                            version_tag: String::from("v0.2.0"),
                            exe_url: String::from("https://github.com/kha0305/dynamic-island-win/releases/download/v0.2.0/win-dynamic-island.exe"),
                            sha256_url: None,
                        },
                        silent: true,
                    })
                    .unwrap();
                ctrl.update_rx = Some(av_rx);
                ctrl.update_in_progress = true;
                ctrl.handle_update_event();
                assert_eq!(ctrl.current_effective_mode(), IslandMode::Notice);
                assert_eq!(
                    ctrl.active_notice.as_ref().map(|n| n.kind),
                    Some(NoticeKind::UpdateAvailable)
                );
                ctrl.execute_ui_action(UiAction::NoticeSecondary);
                assert!(ctrl.active_notice.is_none());
            }

            unsafe {
                let _ = DestroyWindow(hwnd);
                let mut msg = MSG::default();
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {}
                let _ = UnregisterClassW(PCWSTR(class_name.as_ptr()), Some(hinstance));
            }
        })
        .join()
        .expect("Win32 wheel integration test thread panicked");
    }
}
