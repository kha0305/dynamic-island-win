use std::mem::size_of;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateRectRgn, CreateRoundRectRgn, DeleteObject, HGDIOBJ, SetWindowRgn};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA,
    GetWindowLongPtrW, HCURSOR, IDC_ARROW, IDC_HAND, IDYES, IsWindow, KillTimer, LoadCursorW,
    MA_NOACTIVATE, MB_ICONERROR, MB_ICONINFORMATION, MB_TOPMOST, MB_YESNO, MessageBoxW,
    PostQuitMessage, RegisterClassExW, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOZORDER,
    SetCursor, SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow, UnregisterClassW, WM_APP,
    WM_CAPTURECHANGED, WM_COMMAND, WM_DESTROY, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_LBUTTONDBLCLK,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_NCCREATE, WM_RBUTTONUP,
    WM_SETCURSOR, WM_TIMER, WNDCLASSEXW, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::PCWSTR;

use crate::config::{load_config, save_config, set_autostart_enabled};
use crate::media::{MediaCommand, MediaWorker, WM_APP_MEDIA_UPDATE};
use crate::renderer::{IslandRenderer, RenderFrameInput};
use crate::state::{
    AppConfig, IslandAnimator, IslandMode, MediaDisplayState, PixelRect, UiAction, UiHitbox,
    UiRectDip, compute_pill_region_px, compute_surface_size_px, compute_window_origin_px,
    current_windows_filetime_ticks, effective_mode, fill_interactive_hitboxes,
    should_collapse_expanded_on_blank_click, timeline_ratio_for_x, timeline_seek_hit_bounds,
};
use crate::tray::{
    IDM_CHECK_UPDATE, IDM_EXIT, IDM_MONITOR_BASE, IDM_MONITOR_MAX, IDM_OPEN_SETTINGS,
    IDM_TOGGLE_AUTO_HIDE, IDM_TOGGLE_AUTOSTART, IDM_TOGGLE_EXPANDED, IDM_TOGGLE_FULLSCREEN,
    IDM_TOGGLE_NOTCH_COLLAPSE, IDM_TOGGLE_VISIBLE, IDM_TOGGLE_VISUALIZER, TrayIcon, WM_APP_TRAY,
    to_wide_null,
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
const AUTO_COLLAPSE_DELAY_MS: u32 = 5_000;
const COMPACT_TO_NOTCH_DELAY_MS: u32 = 4_000;
const FULLSCREEN_CHECK_INTERVAL_MS: u32 = 800;
const TIMELINE_TICK_INTERVAL_MS: u32 = 1000;
const STARTUP_UPDATE_DELAY_MS: u32 = 10_000;

const TIMELINE_SEEK_HOLD_MS: u64 = 850;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CaptureChange {
    None,
    Acquire,
    Release,
}

enum DeferredUpdateModal {
    Info(String),
    Error(String),
    ConfirmInstall(UpdateReleaseInfo),
}

struct WindowController {
    hwnd: HWND,
    config: AppConfig,
    monitors: Vec<DisplayMonitor>,
    requested_mode: IslandMode,
    media_state: MediaDisplayState,
    fullscreen_active: bool,
    mouse_tracked: bool,
    dragging_timeline: bool,
    timeline_drag_ratio: Option<f32>,
    pointer_down_pos: Option<(f32, f32)>,
    seek_hold_until: Option<Instant>,
    hovered_action: Option<UiAction>,
    dpi_scale: f32,
    animator: IslandAnimator,
    renderer: IslandRenderer,
    vsync: Option<VSyncTicker>,
    media_worker: Option<MediaWorker>,
    fg_hook: Option<ForegroundHookGuard>,
    tray: Option<TrayIcon>,
    last_region: Option<(PixelRect, i32)>,
    hitbox_scratch: Vec<UiHitbox>,
    visualizer_phase: f32,
    cursor_arrow: HCURSOR,
    cursor_hand: HCURSOR,
    update_rx: Option<Receiver<UpdaterEvent>>,
    available_update: Option<UpdateReleaseInfo>,
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
        vsync.set_monitor_rect(active_mon.monitor_rect);
        let media_worker = MediaWorker::start(hwnd);
        let fg_hook = ForegroundHookGuard::install(hwnd);

        let tooltip = format!("Win Dynamic Island (GPU: {})", renderer.adapter_name());
        let tray = TrayIcon::new(hwnd, &tooltip).ok();

        let initial_mode = effective_mode(IslandMode::Slim, config, false, false);
        let controller = Box::new(WindowController {
            hwnd,
            config,
            monitors,
            requested_mode: IslandMode::Slim,
            media_state: MediaDisplayState::default(),
            fullscreen_active: false,
            mouse_tracked: false,
            dragging_timeline: false,
            timeline_drag_ratio: None,
            pointer_down_pos: None,
            seek_hold_until: None,
            hovered_action: None,
            dpi_scale: active_mon.dpi_scale,
            animator: IslandAnimator::new(initial_mode),
            renderer,
            vsync: Some(vsync),
            media_worker: Some(media_worker),
            fg_hook: Some(fg_hook),
            tray,
            last_region: None,
            hitbox_scratch: Vec::with_capacity(8),
            visualizer_phase: 0.0,
            cursor_arrow,
            cursor_hand,
            update_rx: None,
            available_update: None,
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
        if let Some(vsync) = &self.vsync {
            vsync.set_monitor_rect(mon.monitor_rect);
        }
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

    fn refresh_fullscreen_state(&mut self) {
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
                self.animator = IslandAnimator::new(IslandMode::Hidden);
                unsafe {
                    let _ = ShowWindow(self.hwnd, SW_HIDE);
                }
                self.apply_empty_region();
                self.update_vsync_activity();
                self.update_tray_tooltip();
            } else {
                self.apply_target_and_render();
                self.update_tray_tooltip();
            }
        }
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
        let mode = self.current_effective_mode();
        self.animator.set_target_mode(mode);

        if mode != IslandMode::Hidden {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            }
        }

        self.render_current_frame();
        if self.animator.should_hide_window() {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
            self.apply_empty_region();
        }
        self.update_vsync_activity();
        self.sync_timeline_timer();
    }

    fn sync_timeline_timer(&self) {
        let mode = self.current_effective_mode();
        let need_timeline = self.media_state.has_session
            && self.media_state.is_playing
            && self.media_state.timeline.available
            && (mode == IslandMode::Expanded
                || (mode == IslandMode::Slim && !self.config.visualizer_enabled));
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
        let still_animating = self.animator.step(Instant::now());
        if self.config.visualizer_enabled && self.media_state.is_playing {
            self.visualizer_phase = (self.visualizer_phase + 0.28) % std::f32::consts::TAU;
        }
        self.render_current_frame();

        if self.animator.should_hide_window() {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
            self.apply_empty_region();
        }

        if !still_animating {
            self.update_vsync_activity();
        }
    }

    fn update_vsync_activity(&self) {
        let ticker_mode = if self.animator.is_animating() {
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

        let frame = RenderFrameInput {
            pill,
            mode: display_mode,
            pill_opacity: self.animator.opacity.value,
            content_alpha: self.animator.content_alpha.value,
            config: self.config,
            media: &self.media_state,
            hovered_action: self.hovered_action,
            timeline_scrub_ratio: if self.dragging_timeline {
                self.timeline_drag_ratio
            } else {
                None
            },
            monitor_label: &mon_label,
            visualizer_phase: self.visualizer_phase,
        };
        let _ = self.renderer.render(&frame);

        if self.animator.opacity.value > 0.02 {
            let next_rgn = compute_pill_region_px(pill, self.dpi_scale);
            if self.last_region != Some(next_rgn) {
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

    fn start_update_download(&mut self, release: UpdateReleaseInfo) {
        if self.update_in_progress {
            return;
        }
        self.update_in_progress = true;
        self.update_rx = Some(download_and_stage_update_async(self.hwnd, release));
        self.update_tray_tooltip();
    }

    fn handle_check_update_command(&mut self) -> Option<DeferredUpdateModal> {
        if self.update_in_progress {
            return None;
        }
        if let Some(release) = self.available_update.clone() {
            return Some(DeferredUpdateModal::ConfirmInstall(release));
        }
        self.trigger_update_check(false);
        None
    }

    fn handle_update_event(&mut self) -> Option<DeferredUpdateModal> {
        let rx = self.update_rx.take()?;
        let Ok(event) = rx.try_recv() else {
            self.update_rx = Some(rx);
            return None;
        };
        self.update_in_progress = false;
        self.update_tray_tooltip();

        match event {
            UpdaterEvent::CheckUpToDate { silent } => {
                if silent {
                    None
                } else {
                    Some(DeferredUpdateModal::Info(format!(
                        "Bạn đang sử dụng phiên bản mới nhất (v{APP_VERSION})."
                    )))
                }
            }
            UpdaterEvent::CheckAvailable { release, silent } => {
                self.available_update = Some(release.clone());
                self.update_tray_tooltip();
                if silent {
                    None
                } else {
                    Some(DeferredUpdateModal::ConfirmInstall(release))
                }
            }
            UpdaterEvent::CheckError { message, silent } => {
                if silent {
                    None
                } else {
                    Some(DeferredUpdateModal::Error(message))
                }
            }
            UpdaterEvent::DownloadReadyToRestart => {
                if let Err(err) = spawn_updated_process() {
                    return Some(DeferredUpdateModal::Error(err));
                }
                self.shutdown_and_destroy();
                None
            }
            UpdaterEvent::DownloadError { message } => Some(DeferredUpdateModal::Error(message)),
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
        fill_interactive_hitboxes(pill, mode, &mut self.hitbox_scratch);
        self.hitbox_scratch
            .iter()
            .find(|hb| hb.rect.contains(x_dip, y_dip))
            .map(|hb| hb.action)
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
        self.ensure_mouse_tracked();
        self.cancel_auto_collapse();
        self.cancel_compact_to_notch();
        let (x_dip, y_dip) = self.client_dip_from_lparam(lparam);
        self.pointer_down_pos = Some((x_dip, y_dip));

        if self.current_effective_mode() == IslandMode::Expanded
            && let Some(track) = self.current_timeline_track_rect()
            && timeline_seek_hit_bounds(track).contains(x_dip, y_dip)
        {
            let ratio = timeline_ratio_for_x(track, x_dip);
            self.dragging_timeline = true;
            self.timeline_drag_ratio = Some(ratio);
            self.hovered_action = Some(UiAction::SeekTimeline);
            unsafe {
                SetCursor(Some(self.cursor_hand));
            }
            self.render_current_frame();
            return CaptureChange::Acquire;
        }

        CaptureChange::None
    }

    fn handle_mouse_move(&mut self, lparam: LPARAM) {
        self.cancel_auto_collapse();
        self.cancel_compact_to_notch();
        let (x_dip, y_dip) = self.client_dip_from_lparam(lparam);

        if self.dragging_timeline {
            if let Some(track) = self.current_timeline_track_rect() {
                let ratio = timeline_ratio_for_x(track, x_dip);
                if self.timeline_drag_ratio != Some(ratio) {
                    self.timeline_drag_ratio = Some(ratio);
                    self.hovered_action = Some(UiAction::SeekTimeline);
                    unsafe {
                        SetCursor(Some(self.cursor_hand));
                    }
                    self.render_current_frame();
                }
            }
            return;
        }

        self.ensure_mouse_tracked();
        if self.requested_mode == IslandMode::Slim {
            self.requested_mode = IslandMode::Compact;
            self.apply_target_and_render();
        }

        let next_hover = self.hit_test_action(x_dip, y_dip);
        if next_hover != self.hovered_action {
            self.hovered_action = next_hover;
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
        if self.dragging_timeline {
            return;
        }
        self.mouse_tracked = false;
        let hover_changed = self.hovered_action.take().is_some();
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
        } else if hover_changed && !self.animator.is_animating() {
            self.render_current_frame();
        }
    }

    fn handle_capture_changed(&mut self) {
        if !self.dragging_timeline {
            return;
        }
        self.dragging_timeline = false;
        self.timeline_drag_ratio = None;
        self.pointer_down_pos = None;
        self.mouse_tracked = false;
        self.hovered_action = None;
        if self.requested_mode == IslandMode::Expanded {
            self.schedule_auto_collapse(AUTO_COLLAPSE_DELAY_MS);
        }
        self.render_current_frame();
    }

    fn handle_left_button_up(&mut self, lparam: LPARAM) -> (CaptureChange, bool) {
        let (x_dip, y_dip) = self.client_dip_from_lparam(lparam);
        let down_pos = self.pointer_down_pos.take();

        if self.dragging_timeline {
            self.dragging_timeline = false;
            let fallback_ratio = self.timeline_drag_ratio.take().unwrap_or(0.0);
            let ratio = self
                .current_timeline_track_rect()
                .map_or(fallback_ratio, |track| timeline_ratio_for_x(track, x_dip));
            self.commit_timeline_seek_ratio(ratio);

            let pill = self.animator.current_pill();
            let inside_pill = x_dip >= pill.left
                && x_dip <= pill.left + pill.width
                && y_dip >= pill.top
                && y_dip <= pill.top + pill.height;
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
            self.render_current_frame();
            return (CaptureChange::Release, inside_pill);
        }

        if let Some(action) = self.hit_test_action(x_dip, y_dip) {
            if action == UiAction::SeekTimeline {
                self.handle_timeline_seek(x_dip);
                return (CaptureChange::None, false);
            }
            self.execute_ui_action(action);
            return (CaptureChange::None, false);
        }

        if self.requested_mode == IslandMode::Expanded {
            let pill = self.animator.current_pill();
            if !should_collapse_expanded_on_blank_click(
                pill,
                down_pos.unwrap_or((x_dip, y_dip)),
                (x_dip, y_dip),
            ) {
                return (CaptureChange::None, false);
            }
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
            IslandMode::Settings => {
                self.cancel_compact_to_notch();
                IslandMode::Expanded
            }
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

    fn handle_right_click(&mut self) {
        self.requested_mode = if self.requested_mode == IslandMode::Settings {
            IslandMode::Expanded
        } else {
            IslandMode::Settings
        };
        self.hovered_action = None;
        self.apply_target_and_render();
    }

    fn execute_ui_action(&mut self, action: UiAction) {
        match action {
            UiAction::OpenSettings => {
                self.cancel_compact_to_notch();
                self.requested_mode = IslandMode::Settings;
            }
            UiAction::BackToMedia => {
                self.requested_mode = IslandMode::Expanded;
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
                self.cancel_compact_to_notch();
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
            UiAction::SeekTimeline => return,
        }
        self.hovered_action = None;
        self.apply_target_and_render();
    }

    fn shutdown_and_destroy(&mut self) {
        self.cancel_auto_collapse();
        self.cancel_compact_to_notch();
        let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_FULLSCREEN_CHECK_ID) };
        let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_TIMELINE_TICK_ID) };
        let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_STARTUP_UPDATE_ID) };
        self.fg_hook.take();
        self.media_worker.take();
        self.vsync.take();
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
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
        WM_APP_FOREGROUND_CHANGED => {
            if let Some(ctrl) = get_controller_mut(hwnd) {
                ctrl.refresh_fullscreen_state();
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
            let modal = get_controller_mut(hwnd).and_then(|ctrl| ctrl.handle_update_event());
            show_deferred_update_modal(hwnd, modal);
            LRESULT(0)
        }
        WM_TIMER => {
            if let Some(ctrl) = get_controller_mut(hwnd) {
                match wparam.0 {
                    TIMER_AUTO_COLLAPSE_ID => ctrl.handle_auto_collapse_timer(),
                    TIMER_COMPACT_TO_NOTCH_ID => ctrl.handle_compact_to_notch_timer(),
                    TIMER_FULLSCREEN_CHECK_ID => ctrl.refresh_fullscreen_state(),
                    TIMER_STARTUP_UPDATE_ID => {
                        let _ = unsafe { KillTimer(Some(hwnd), TIMER_STARTUP_UPDATE_ID) };
                        ctrl.trigger_update_check(true);
                    }
                    TIMER_TIMELINE_TICK_ID
                        if !ctrl.animator.is_animating()
                            && !ctrl.dragging_timeline
                            && matches!(
                                ctrl.current_effective_mode(),
                                IslandMode::Expanded | IslandMode::Slim
                            ) =>
                    {
                        ctrl.render_current_frame();
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_SETCURSOR => {
            if let Some(ctrl) = get_controller_mut(hwnd) {
                let cursor = if ctrl.dragging_timeline || ctrl.hovered_action.is_some() {
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
                            IslandMode::Expanded | IslandMode::Settings
                        );
                        if let Some(tray) = &ctrl.tray {
                            let update_ver =
                                ctrl.available_update.as_ref().map(|r| r.version_tag.as_str());
                            tray.show_context_menu(
                                ctrl.config,
                                &ctrl.media_state,
                                &ctrl.monitors,
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
                        ctrl.cancel_compact_to_notch();
                        ctrl.config.visible = true;
                        ctrl.requested_mode = IslandMode::Settings;
                        ctrl.sync_fullscreen_timer();
                        ctrl.apply_target_and_render();
                        ctrl.update_tray_tooltip();
                        save_config(&ctrl.config);
                    }
                    IDM_TOGGLE_EXPANDED => {
                        ctrl.config.visible = true;
                        ctrl.requested_mode = match ctrl.requested_mode {
                            IslandMode::Expanded | IslandMode::Settings => {
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
                    IDM_TOGGLE_AUTOSTART => ctrl.execute_ui_action(UiAction::ToggleAutostart),
                    IDM_CHECK_UPDATE => {
                        let modal = ctrl.handle_check_update_command();
                        let _ = ctrl;
                        show_deferred_update_modal(hwnd, modal);
                    }
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

fn show_deferred_update_modal(hwnd: HWND, modal: Option<DeferredUpdateModal>) {
    let Some(modal) = modal else {
        return;
    };
    let caption = to_wide_null("Win Dynamic Island - Cập nhật");
    match modal {
        DeferredUpdateModal::Info(msg) => {
            let body = to_wide_null(&msg);
            unsafe {
                let _ = MessageBoxW(
                    None,
                    PCWSTR(body.as_ptr()),
                    PCWSTR(caption.as_ptr()),
                    MB_ICONINFORMATION | MB_TOPMOST,
                );
            }
        }
        DeferredUpdateModal::Error(msg) => {
            let body = to_wide_null(&msg);
            unsafe {
                let _ = MessageBoxW(
                    None,
                    PCWSTR(body.as_ptr()),
                    PCWSTR(caption.as_ptr()),
                    MB_ICONERROR | MB_TOPMOST,
                );
            }
        }
        DeferredUpdateModal::ConfirmInstall(release) => {
            let prompt = format!(
                "Đã có phiên bản mới {} (hiện tại v{APP_VERSION}).\n\nBạn có muốn tải về và cập nhật ngay không?",
                release.version_tag
            );
            let body = to_wide_null(&prompt);
            let choice = unsafe {
                MessageBoxW(
                    None,
                    PCWSTR(body.as_ptr()),
                    PCWSTR(caption.as_ptr()),
                    MB_YESNO | MB_ICONINFORMATION | MB_TOPMOST,
                )
            };
            if choice == IDYES
                && let Some(ctrl) = get_controller_mut(hwnd)
            {
                ctrl.start_update_download(release);
            }
        }
    }
}
