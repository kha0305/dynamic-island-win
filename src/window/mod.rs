mod input;
mod modes;
#[cfg(test)]
mod tests;
mod workers;

use std::mem::size_of;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, HGDIOBJ, HRGN, RGN_OR,
    ScreenToClient, SetWindowRgn,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Ole::{IDropTarget, RegisterDragDrop, RevokeDragDrop};
use windows::Win32::System::Power::{
    HPOWERNOTIFY, POWERBROADCAST_SETTING, RegisterPowerSettingNotification,
    UnregisterPowerSettingNotification,
};
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::Shell::{DragAcceptFiles, DragFinish, HDROP};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, ChangeWindowMessageFilterEx, CreateWindowExW,
    DEVICE_NOTIFY_WINDOW_HANDLE, DefWindowProcW, DestroyWindow, GWLP_USERDATA, GetCursorPos,
    GetSystemMetrics, GetWindowLongPtrW, HCURSOR, HTCLIENT, HTTRANSPARENT, IDC_ARROW, IDC_HAND,
    IsWindow, IsWindowVisible, KillTimer, LoadCursorW, MA_NOACTIVATE, MSGFLT_ALLOW,
    PBT_APMPOWERSTATUSCHANGE, PBT_POWERSETTINGCHANGE, PostQuitMessage, RegisterClassExW,
    RegisterWindowMessageW, SM_CXDRAG, SM_CYDRAG, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE,
    SWP_NOZORDER, SetCursor, SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    UnregisterClassW, WM_APP, WM_CAPTURECHANGED, WM_COMMAND, WM_DESTROY, WM_DISPLAYCHANGE,
    WM_DPICHANGED, WM_DROPFILES, WM_ENDSESSION, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCCREATE, WM_NCHITTEST, WM_POWERBROADCAST,
    WM_RBUTTONUP, WM_SETCURSOR, WM_TIMER, WNDCLASSEXW, WS_EX_NOACTIVATE,
    WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WindowFromPoint,
};
use windows::core::PCWSTR;

use crate::anti::{
    AntiAgentState, AntiProgressSnapshot, AntiWatcher, WM_APP_ANTI_UPDATE, are_anti_hooks_installed,
    install_anti_hooks, now_unix_ms, uninstall_anti_hooks,
};
use crate::config::{load_config, save_config, set_autostart_enabled};
use crate::hud::{
    AudioCommand, AudioMonitor, AudioOutputDevice, DeviceHudEvent, GUID_ACDC_POWER_SOURCE,
    HUD_EXPIRE_DELAY_MS, HudWorkerEvent, PowerStateTracker, VolumeSnapshot, WM_APP_HUD_EVENT,
    WheelVolumeAccumulator, apply_volume_step,
};
use crate::media::{MediaCommand, MediaWorker, WM_APP_MEDIA_UPDATE};
use crate::renderer::{IslandRenderer, RenderFrameInput, ShelfRenderInput};
use crate::shelf::{
    IslandDropTarget, ShelfState, WM_APP_SHELF_DROP, WM_APP_SHELF_HOVER, extract_hdrop_paths,
    load_shelf_state, save_shelf_state, start_shelf_drag_out,
};
use crate::state::{
    AppConfig, AudioOutputKind, DeviceHudCategory, HudContent, IslandAnimator, IslandMode,
    IslandNoticeView, MAX_AUDIO_DEVICES_VISIBLE, MAX_SHELF_VISIBLE_ROWS, MAX_SURFACE_HEIGHT_DIP,
    MAX_SURFACE_WIDTH_DIP, MediaDisplayState, NoticeKind, PillRectDip, PixelRect, UiAction,
    UiHitbox, UiRectDip, compute_pill_region_px, compute_surface_size_px,
    compute_window_origin_px, current_windows_filetime_ticks, effective_mode,
    fill_audio_devices_hitboxes, fill_compact_media_hitboxes, fill_interactive_hitboxes,
    fill_notice_hitboxes, fill_shelf_hitboxes, fill_volume_hud_hitboxes, left_anti_pill_rect,
    pill_contains_dip, shelf_contains_dip, shelf_pill_rect, should_close_subview_on_header_click,
    should_collapse_expanded_on_blank_click, should_collapse_shelf_on_header_click,
    target_dimensions_for_state, target_top_for_mode, timeline_ratio_for_x,
    timeline_seek_hit_bounds, volume_seek_hit_bounds,
};
use crate::tray::{
    IDM_AUDIO_DEVICE_BASE, IDM_AUDIO_DEVICE_MAX, IDM_CHECK_UPDATE, IDM_EXIT, IDM_MONITOR_BASE,
    IDM_MONITOR_MAX, IDM_OPEN_SETTINGS, IDM_TOGGLE_AUTO_HIDE, IDM_TOGGLE_AUTOSTART,
    IDM_TOGGLE_EXPANDED, IDM_TOGGLE_FULLSCREEN, IDM_TOGGLE_HUD_DEVICE, IDM_TOGGLE_HUD_POWER,
    IDM_TOGGLE_HUD_VOLUME, IDM_TOGGLE_HUD_WHEEL, IDM_TOGGLE_NOTCH_COLLAPSE, IDM_TOGGLE_VISIBLE,
    IDM_TOGGLE_VISUALIZER, TrayIcon, WM_APP_TRAY, load_app_icon_for_window, to_wide_null,
};
use crate::updater::{
    APP_VERSION, UpdateReleaseInfo, UpdaterEvent, WM_APP_UPDATE_EVENT, check_for_updates_async,
    download_and_stage_update_async, spawn_updated_process,
};
use crate::vsync::{TickerMode, VSyncTicker, WM_APP_VSYNC_TICK};
use crate::watcher::{
    DisplayMonitor, ForegroundHookGuard, WM_APP_FOREGROUND_CHANGED, enumerate_monitors,
    focus_anti_window, is_foreground_fullscreen_on_monitor, select_monitor_or_default,
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
const TIMER_ANTI_EXPIRE_ID: usize = 8;
const TIMER_SHELF_DRAG_LEAVE_ID: usize = 9;
const TIMER_TRAY_RETRY_ID: usize = 10;
const AUTO_COLLAPSE_DELAY_MS: u32 = 5_000;
const COMPACT_TO_NOTCH_DELAY_MS: u32 = 4_000;
const NOTICE_AUTO_DISMISS_MS: u32 = 4_000;
const ANTI_TERMINAL_DISPLAY_MS: u32 = 4_000;
const ANTI_ERROR_DISPLAY_MS: u32 = 10_000;
const DEVICE_HUD_EXPIRE_DELAY_MS: u32 = 2_600;
const SHELF_DRAG_LEAVE_DELAY_MS: u32 = 180;
const TRAY_RETRY_INTERVAL_MS: u32 = 1_500;
const MAX_TRAY_RETRY_ATTEMPTS: u8 = 20;
const FULLSCREEN_CHECK_INTERVAL_MS: u32 = 800;
const TIMELINE_TICK_INTERVAL_MS: u32 = 1000;
const STARTUP_UPDATE_DELAY_MS: u32 = 10_000;

const TIMELINE_SEEK_HOLD_MS: u64 = 850;
const VOLUME_ECHO_HOLD_MS: u64 = 150;
const MK_LBUTTON_MASK: usize = 0x0001;
const MK_RBUTTON_MASK: usize = 0x0002;
const MK_SHIFT_MASK: usize = 0x0004;
const MK_CONTROL_MASK: usize = 0x0008;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    hovered_header: bool,
    dpi_scale: f32,
    animator: IslandAnimator,
    renderer: IslandRenderer,
    vsync: Option<VSyncTicker>,
    media_worker: Option<MediaWorker>,
    audio_monitor: Option<AudioMonitor>,
    audio_devices: Vec<AudioOutputDevice>,
    audio_device_scroll: usize,
    current_audio_device_name: Option<String>,
    current_audio_device_kind: AudioOutputKind,
    hud_device_name: Option<String>,
    hud_device_container_key: Option<String>,
    volume_snapshot: VolumeSnapshot,
    power_tracker: PowerStateTracker,
    power_notify_handle: Option<HPOWERNOTIFY>,
    active_hud: Option<HudContent>,
    fg_hook: Option<ForegroundHookGuard>,
    tray: Option<TrayIcon>,
    wm_taskbar_created: u32,
    tray_retry_attempts: u8,
    last_region: Option<(PixelRect, i32)>,
    last_shelf_region: Option<(PixelRect, i32)>,
    last_left_anti_region: Option<(PixelRect, i32)>,
    last_bridge_region: Option<PixelRect>,
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
    anti_watcher: Option<AntiWatcher>,
    anti_slots: Vec<AntiProgressSnapshot>,
    dismissed_anti_keys: Vec<(String, u64)>,
    anti_expanded_key: Option<String>,
    anti_hooks_installed: bool,
    anti_spinner_phase: f32,
    shelf: ShelfState,
    shelf_drop_rx: Receiver<Vec<PathBuf>>,
    drop_target_registered: bool,
    shelf_auto_opened_by_drag: bool,
    shelf_drag_candidate: Option<(usize, f32, f32)>,
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
        let app_icon_large = load_app_icon_for_window(hinstance, true).unwrap_or_default();
        let app_icon_small = load_app_icon_for_window(hinstance, false).unwrap_or_default();
        let taskbar_msg_w = to_wide_null("TaskbarCreated");
        let wm_taskbar_created = unsafe { RegisterWindowMessageW(PCWSTR(taskbar_msg_w.as_ptr())) };

        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: hinstance,
            hIcon: app_icon_large,
            hIconSm: app_icon_small,
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
        let anti_watcher = AntiWatcher::start(hwnd);
        let fg_hook = ForegroundHookGuard::install(hwnd);
        let anti_hooks_installed = are_anti_hooks_installed();
        let mut shelf = load_shelf_state();
        shelf.prune_missing_files();
        let (shelf_drop_tx, shelf_drop_rx) = std::sync::mpsc::channel();
        let drop_target: IDropTarget = IslandDropTarget::new(hwnd, shelf_drop_tx).into();
        let drop_target_registered = unsafe { RegisterDragDrop(hwnd, &drop_target) }.is_ok();
        unsafe {
            const WM_COPYGLOBALDATA: u32 = 0x0049;
            const WM_COPYDATA_MSG: u32 = 0x004A;
            let _ = ChangeWindowMessageFilterEx(hwnd, WM_DROPFILES, MSGFLT_ALLOW, None);
            let _ = ChangeWindowMessageFilterEx(hwnd, WM_COPYGLOBALDATA, MSGFLT_ALLOW, None);
            let _ = ChangeWindowMessageFilterEx(hwnd, WM_COPYDATA_MSG, MSGFLT_ALLOW, None);
            if !drop_target_registered {
                DragAcceptFiles(hwnd, true);
            }
        }

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

        let rest_mode = if config.notch_collapse_after_4s || config.auto_hide_when_idle {
            IslandMode::Slim
        } else {
            IslandMode::Compact
        };
        let initial_mode = effective_mode(rest_mode, config, false, false, false);
        let controller = Box::new(WindowController {
            hwnd,
            config,
            monitors,
            requested_mode: rest_mode,
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
            hovered_header: false,
            dpi_scale: active_mon.dpi_scale,
            animator: IslandAnimator::new(initial_mode),
            renderer,
            vsync: Some(vsync),
            media_worker: Some(media_worker),
            audio_monitor: Some(audio_monitor),
            audio_devices: Vec::new(),
            audio_device_scroll: 0,
            current_audio_device_name: None,
            current_audio_device_kind: AudioOutputKind::Speaker,
            hud_device_name: None,
            hud_device_container_key: None,
            volume_snapshot: VolumeSnapshot::default(),
            power_tracker,
            power_notify_handle,
            active_hud: None,
            fg_hook: Some(fg_hook),
            tray,
            wm_taskbar_created,
            tray_retry_attempts: 0,
            last_region: None,
            last_shelf_region: None,
            last_left_anti_region: None,
            last_bridge_region: None,
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
            anti_watcher: Some(anti_watcher),
            anti_slots: Vec::new(),
            dismissed_anti_keys: Vec::new(),
            anti_expanded_key: None,
            anti_hooks_installed,
            anti_spinner_phase: 0.0,
            shelf,
            shelf_drop_rx,
            drop_target_registered,
            shelf_auto_opened_by_drag: false,
            shelf_drag_candidate: None,
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
            ctrl.schedule_tray_retry_if_missing();
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
    pub(super) fn release_resources(&mut self) {
        self.cancel_auto_collapse();
        self.cancel_compact_to_notch();
        self.cancel_hud_expire();
        self.cancel_notice_expire();
        self.cancel_anti_expire();
        let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_FULLSCREEN_CHECK_ID) };
        let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_TIMELINE_TICK_ID) };
        let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_STARTUP_UPDATE_ID) };
        let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_TRAY_RETRY_ID) };
        if self.drop_target_registered {
            self.drop_target_registered = false;
            unsafe {
                let _ = RevokeDragDrop(self.hwnd);
            }
        }
        save_shelf_state(&self.shelf);
        if let Some(handle) = self.power_notify_handle.take() {
            unsafe {
                let _ = UnregisterPowerSettingNotification(handle);
            }
        }
        self.fg_hook.take();
        self.anti_watcher.take();
        self.audio_monitor.take();
        self.media_worker.take();
        self.vsync.take();
    }

    pub(super) fn shutdown_and_destroy(&mut self) {
        self.release_resources();
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }

    pub(super) fn build_tray_tooltip(&self) -> String {
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
        let drop_tag = if self.drop_target_registered {
            ""
        } else {
            " - Kéo thả: Fallback"
        };
        format!(
            "Win Dynamic Island - {}{}{} ({}{})",
            status,
            update_tag,
            drop_tag,
            glass_tag,
            self.renderer.adapter_name()
        )
    }

    pub(super) fn schedule_tray_retry_if_missing(&mut self) {
        if self.tray.is_none() && self.tray_retry_attempts < MAX_TRAY_RETRY_ATTEMPTS {
            unsafe {
                let _ = SetTimer(
                    Some(self.hwnd),
                    TIMER_TRAY_RETRY_ID,
                    TRAY_RETRY_INTERVAL_MS,
                    None,
                );
            }
        }
    }

    pub(super) fn handle_tray_retry_timer(&mut self) {
        if self.tray.is_some() {
            let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_TRAY_RETRY_ID) };
            return;
        }
        let tooltip = self.build_tray_tooltip();
        if let Ok(tray) = TrayIcon::new(self.hwnd, &tooltip) {
            self.tray = Some(tray);
            self.tray_retry_attempts = 0;
            let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_TRAY_RETRY_ID) };
            return;
        }
        self.tray_retry_attempts = self.tray_retry_attempts.saturating_add(1);
        if self.tray_retry_attempts >= MAX_TRAY_RETRY_ATTEMPTS {
            let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_TRAY_RETRY_ID) };
        }
    }

    pub(super) fn handle_taskbar_created(&mut self) {
        let tooltip = self.build_tray_tooltip();
        if let Some(tray) = self.tray.as_mut() {
            if !tray.re_add(&tooltip) {
                self.tray = TrayIcon::new(self.hwnd, &tooltip).ok();
            }
        } else {
            self.tray = TrayIcon::new(self.hwnd, &tooltip).ok();
        }
        if self.tray.is_some() {
            self.tray_retry_attempts = 0;
            let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_TRAY_RETRY_ID) };
        } else {
            self.schedule_tray_retry_if_missing();
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
        WM_APP_ANTI_UPDATE => {
            if let Some(ctrl) = get_controller_mut(hwnd) {
                ctrl.handle_anti_update();
            }
            LRESULT(0)
        }
        WM_APP_SHELF_HOVER => {
            if let Some(ctrl) = get_controller_mut(hwnd) {
                ctrl.handle_shelf_drag_hover(wparam.0 != 0);
            }
            LRESULT(0)
        }
        WM_APP_SHELF_DROP => {
            if let Some(ctrl) = get_controller_mut(hwnd) {
                ctrl.handle_shelf_drop();
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
                get_controller_mut(hwnd).is_some_and(WindowController::refresh_fullscreen_state);
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
                    TIMER_ANTI_EXPIRE_ID => ctrl.handle_anti_expire_timer(),
                    TIMER_SHELF_DRAG_LEAVE_ID => ctrl.handle_shelf_drag_leave_timer(),
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
                    TIMER_TRAY_RETRY_ID => ctrl.handle_tray_retry_timer(),
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
        WM_NCHITTEST => {
            if let Some(ctrl) = get_controller_mut(hwnd) {
                return LRESULT(ctrl.handle_nchittest(lparam));
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_DROPFILES => {
            let hdrop = HDROP(wparam.0 as *mut core::ffi::c_void);
            let paths = unsafe { extract_hdrop_paths(hdrop) };
            unsafe { DragFinish(hdrop) };
            if let Some(ctrl) = get_controller_mut(hwnd) {
                ctrl.handle_shelf_drop_paths(paths);
            }
            LRESULT(0)
        }
        WM_SETCURSOR => {
            if let Some(ctrl) = get_controller_mut(hwnd) {
                let cursor = if ctrl.dragging_timeline
                    || ctrl.dragging_volume
                    || ctrl.shelf_drag_candidate.is_some()
                    || ctrl.hovered_action.is_some()
                    || ctrl.hovered_header
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
            let drag_out_paths =
                get_controller_mut(hwnd).and_then(|ctrl| ctrl.handle_mouse_move(lparam));
            if let Some(paths) = drag_out_paths {
                // Giải phóng capture và chạy vòng lặp modal DoDragDrop ngoài borrow `&mut WindowController`.
                unsafe {
                    let _ = ReleaseCapture();
                }
                start_shelf_drag_out(&paths);
                if let Some(ctrl) = get_controller_mut(hwnd) {
                    ctrl.finish_shelf_drag_out();
                }
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
                                | IslandMode::AntiExpanded
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
                        ctrl.settings_return_mode = match ctrl.requested_mode {
                            IslandMode::Expanded => IslandMode::Expanded,
                            IslandMode::AntiExpanded => IslandMode::AntiExpanded,
                            _ => ctrl.collapsed_rest_mode(),
                        };
                        ctrl.config.visible = true;
                        ctrl.anti_hooks_installed = are_anti_hooks_installed();
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
                            | IslandMode::AntiExpanded
                            | IslandMode::Settings
                            | IslandMode::AudioDevices
                            | IslandMode::Notice => {
                                ctrl.cancel_auto_collapse();
                                if ctrl.should_collapse_to_slim() {
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
                    IDM_TOGGLE_HUD_DEVICE => ctrl.execute_ui_action(UiAction::ToggleHudDevice),
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
        WM_ENDSESSION => {
            if wparam.0 != 0
                && let Some(ctrl) = get_controller_mut(hwnd)
            {
                ctrl.release_resources();
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
        _ => {
            if let Some(ctrl) = get_controller_mut(hwnd)
                && ctrl.wm_taskbar_created != 0
                && msg == ctrl.wm_taskbar_created
            {
                ctrl.handle_taskbar_created();
                return LRESULT(0);
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}
