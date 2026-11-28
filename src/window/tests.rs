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
                HudWorkerEvent::DevicesChanged { .. } | HudWorkerEvent::DeviceChanged(_) => {
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
        HudWorkerEvent::DevicesChanged { .. }
        | HudWorkerEvent::DeviceChanged(_)
        | HudWorkerEvent::PowerSync { .. } => unreachable!(),
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
            hovered_header: false,
            dpi_scale: 1.0,
            animator: IslandAnimator::new(IslandMode::Slim),
            renderer,
            vsync: None,
            media_worker: None,
            audio_monitor: None,
            audio_devices: Vec::new(),
            audio_device_scroll: 0,
            current_audio_device_name: None,
            current_audio_device_kind: AudioOutputKind::Speaker,
            hud_device_name: None,
            hud_device_container_key: None,
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
            wm_taskbar_created: 0,
            tray_retry_attempts: 0,
            last_region: None,
            last_shelf_region: None,
            last_left_anti_region: None,
            last_bridge_region: None,
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
            anti_watcher: None,
            anti_slots: Vec::new(),
            dismissed_anti_keys: Vec::new(),
            anti_expanded_key: None,
            anti_hooks_installed: false,
            anti_spinner_phase: 0.0,
            shelf: ShelfState::default(),
            shelf_drop_rx: std::sync::mpsc::channel().1,
            drop_target_registered: false,
            shelf_auto_opened_by_drag: false,
            shelf_drag_candidate: None,
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
                    kind: AudioOutputKind::Speaker,
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

        // 13. Khi đã tắt Gọn mép 4s (và tắt Tự ẩn khi rảnh), tuyệt đối không tự co về Slim dù không có nhạc.
        {
            let ctrl = get_controller_mut(hwnd).unwrap();
            ctrl.config.notch_collapse_after_4s = false;
            ctrl.config.auto_hide_when_idle = false;
            ctrl.media_state = MediaDisplayState::default();
            ctrl.requested_mode = IslandMode::Slim;
            ctrl.execute_ui_action(UiAction::OpenSettings);
            assert_eq!(ctrl.current_effective_mode(), IslandMode::Settings);
            ctrl.sync_mode_after_collapse_policy_change();
            ctrl.execute_ui_action(UiAction::BackToMedia);
            assert_eq!(ctrl.requested_mode, IslandMode::Compact);

            ctrl.mouse_tracked = true;
            ctrl.handle_mouse_leave();
            assert_eq!(ctrl.requested_mode, IslandMode::Compact);
            ctrl.handle_compact_to_notch_timer();
            assert_eq!(ctrl.requested_mode, IslandMode::Compact);

            ctrl.requested_mode = IslandMode::Expanded;
            ctrl.handle_auto_collapse_timer();
            assert_eq!(ctrl.requested_mode, IslandMode::Compact);
        }

        // 14. File Shelf: mở khay cạnh phải, tick chọn mới hiện Delete All, và xóa khay giữ nguyên tệp gốc trên đĩa.
        {
            let tmp_dir = std::env::temp_dir().join(format!("wdi_win_shelf_{}", std::process::id()));
            let _ = std::fs::create_dir_all(&tmp_dir);
            let f1 = tmp_dir.join("note.txt");
            let f2 = tmp_dir.join("spec.md");
            let _ = std::fs::write(&f1, b"abc");
            let _ = std::fs::write(&f2, b"xyz");

            let (drop_tx, drop_rx) = std::sync::mpsc::channel();
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                ctrl.shelf_drop_rx = drop_rx;
                ctrl.config.visible = true;
                ctrl.requested_mode = IslandMode::Compact;
                ctrl.animator = IslandAnimator::new(IslandMode::Compact);
            }
            drop_tx.send(vec![f1.clone(), f2.clone()]).unwrap();
            unsafe {
                window_proc(hwnd, WM_APP_SHELF_DROP, WPARAM(0), LPARAM(0));
            }
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                assert_eq!(ctrl.shelf.items.len(), 2);
                assert!(ctrl.shelf.open);
                assert_eq!(ctrl.shelf.selected_count(), 0);
            }

            let pill = get_controller_mut(hwnd).unwrap().animator.current_pill();
            let s_rect = shelf_pill_rect(pill, IslandMode::Compact, true, 2).unwrap();
            let mut shelf_hbs = Vec::new();
            fill_shelf_hitboxes(s_rect, true, 2, 0, 0, &mut shelf_hbs);
            assert!(
                !shelf_hbs.iter().any(|hb| hb.action == UiAction::ShelfDeleteAll),
                "Khong duoc co hitbox Delete All khi chua tick muc nao"
            );

            // Click tickbox đầu tiên (WM_LBUTTONDOWN/UP nhận tọa độ client).
            let tick0 = shelf_hbs
                .iter()
                .find(|hb| hb.action == UiAction::ShelfToggleSelect(0))
                .unwrap()
                .rect;
            let tick_x = (tick0.left + tick0.width() * 0.5).round() as i32;
            let tick_y = (tick0.top + tick0.height() * 0.5).round() as i32;
            unsafe {
                window_proc(hwnd, WM_LBUTTONDOWN, WPARAM(1), make_lparam_xy(tick_x, tick_y));
                window_proc(hwnd, WM_LBUTTONUP, WPARAM(0), make_lparam_xy(tick_x, tick_y));
            }
            assert_eq!(get_controller_mut(hwnd).unwrap().shelf.selected_count(), 1);

            fill_shelf_hitboxes(s_rect, true, 2, 0, 1, &mut shelf_hbs);
            let del_btn = shelf_hbs
                .iter()
                .find(|hb| hb.action == UiAction::ShelfDeleteAll)
                .unwrap()
                .rect;
            let del_x = (del_btn.left + del_btn.width() * 0.5).round() as i32;
            let del_y = (del_btn.top + del_btn.height() * 0.5).round() as i32;
            unsafe {
                window_proc(hwnd, WM_LBUTTONDOWN, WPARAM(1), make_lparam_xy(del_x, del_y));
                window_proc(hwnd, WM_LBUTTONUP, WPARAM(0), make_lparam_xy(del_x, del_y));
            }
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                assert_eq!(ctrl.shelf.items.len(), 1);
                assert_eq!(ctrl.shelf.selected_count(), 0);
            }
            assert!(f1.exists());
            assert!(f2.exists());

            // 15. Kiểm tra WM_NCHITTEST + vùng cầu nối khi kéo (WM_APP_SHELF_HOVER) + trễ DragLeave và tách kéo ra khỏi &mut borrow.
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                ctrl.shelf.open = false;
                ctrl.shelf_auto_opened_by_drag = false;
                ctrl.shelf.drag_hover = false;
                ctrl.requested_mode = IslandMode::Compact;
                ctrl.animator = IslandAnimator::new(IslandMode::Compact);
            }
            let compact_pill = get_controller_mut(hwnd).unwrap().animator.current_pill();
            let in_pt = pill_to_screen(compact_pill.left + 40.0, compact_pill.top + 14.0);
            let gap_pt = pill_to_screen(compact_pill.left + compact_pill.width + 5.0, compact_pill.top + 14.0);

            unsafe {
                window_proc(hwnd, WM_APP_SHELF_HOVER, WPARAM(1), LPARAM(0));
            }
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                assert!(ctrl.shelf.open, "Keo tep toi phai tu mo Khay tep");
                assert!(ctrl.shelf_auto_opened_by_drag);
            }
            let nc_bridge = unsafe {
                window_proc(hwnd, WM_NCHITTEST, WPARAM(0), make_lparam_xy(gap_pt.x, gap_pt.y))
            };
            assert_eq!(
                nc_bridge.0, HTCLIENT as isize,
                "Trong luc keo tep, khe 10 DIP giua dao va khay phai la HTCLIENT de khong dut DragLeave"
            );

            unsafe {
                window_proc(hwnd, WM_APP_SHELF_HOVER, WPARAM(0), LPARAM(0));
            }
            assert!(
                get_controller_mut(hwnd).unwrap().shelf.open,
                "Khay phai giu mo trong khoang tre chong rung DragLeave"
            );
            unsafe {
                window_proc(hwnd, WM_TIMER, WPARAM(TIMER_SHELF_DRAG_LEAVE_ID), LPARAM(0));
            }
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                assert!(!ctrl.shelf.open, "Het tre DragLeave ma chua tha phai tu thu gon lai");
                assert!(!ctrl.shelf_auto_opened_by_drag);
            }

            let nc_in = unsafe {
                window_proc(hwnd, WM_NCHITTEST, WPARAM(0), make_lparam_xy(in_pt.x, in_pt.y))
            };
            let nc_gap = unsafe {
                window_proc(hwnd, WM_NCHITTEST, WPARAM(0), make_lparam_xy(gap_pt.x, gap_pt.y))
            };
            assert_eq!(nc_in.0, HTCLIENT as isize);
            assert_eq!(nc_gap.0, HTTRANSPARENT as isize);

            // Mở khay với 1 tệp và xác nhận handle_mouse_move trả về đường dẫn kéo ra ngoài borrow.
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                ctrl.shelf.open = true;
            }
            let s_open = shelf_pill_rect(compact_pill, IslandMode::Compact, true, 1).unwrap();
            let mut open_hbs = Vec::new();
            fill_shelf_hitboxes(s_open, true, 1, 0, 0, &mut open_hbs);
            let drag_row = open_hbs
                .iter()
                .find(|hb| matches!(hb.action, UiAction::ShelfDragItem(0)))
                .unwrap()
                .rect;
            let dx = (drag_row.left + drag_row.width() * 0.5).round() as i32;
            let dy = (drag_row.top + drag_row.height() * 0.5).round() as i32;
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                assert_eq!(
                    ctrl.handle_left_button_down(make_lparam_xy(dx, dy)),
                    CaptureChange::Acquire
                );
                let dragged = ctrl.handle_mouse_move(make_lparam_xy(dx + 24, dy));
                assert_eq!(dragged, Some(vec![f1.clone()]));
                assert!(ctrl.shelf_drag_candidate.is_none());
            }

            // 16. Bấm thanh tiêu đề phía trên của Khay tệp và Cài đặt để thu gọn/quay lại nhanh (vẫn giữ nút gốc).
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                ctrl.shelf.open = true;
                ctrl.requested_mode = IslandMode::Compact;
                ctrl.animator = IslandAnimator::new(IslandMode::Compact);
            }
            let hdr_x = (s_open.left + 44.0).round() as i32;
            let hdr_y = (s_open.top + 16.0).round() as i32;
            unsafe {
                window_proc(hwnd, WM_LBUTTONDOWN, WPARAM(1), make_lparam_xy(hdr_x, hdr_y));
                window_proc(hwnd, WM_LBUTTONUP, WPARAM(0), make_lparam_xy(hdr_x, hdr_y));
            }
            assert!(
                !get_controller_mut(hwnd).unwrap().shelf.open,
                "Bam thanh tieu de Khay tep phai thu gon nhanh"
            );

            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                ctrl.requested_mode = IslandMode::Compact;
                ctrl.execute_ui_action(UiAction::OpenSettings);
                ctrl.animator = IslandAnimator::new(IslandMode::Settings);
            }
            let st_pill = get_controller_mut(hwnd).unwrap().animator.current_pill();
            let st_hdr_x = (st_pill.left + 90.0).round() as i32;
            let st_hdr_y = (st_pill.top + 18.0).round() as i32;
            unsafe {
                window_proc(hwnd, WM_LBUTTONDOWN, WPARAM(1), make_lparam_xy(st_hdr_x, st_hdr_y));
                window_proc(hwnd, WM_LBUTTONUP, WPARAM(0), make_lparam_xy(st_hdr_x, st_hdr_y));
            }
            assert_eq!(
                get_controller_mut(hwnd).unwrap().current_effective_mode(),
                IslandMode::Compact,
                "Bam thanh tieu de Cai dat phai quay lai nhanh"
            );

            // 17. Chặn mở khay khi đang ẩn; tự thoát Settings nhận thả tệp; bật/tắt Gọn mép vẫn kéo thả mượt; và hẹn thu gọn sau drag-out.
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                ctrl.config.visible = false;
                ctrl.shelf.open = false;
                ctrl.shelf_auto_opened_by_drag = false;
                ctrl.shelf.drag_hover = false;
            }
            unsafe {
                window_proc(hwnd, WM_APP_SHELF_HOVER, WPARAM(1), LPARAM(0));
            }
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                assert!(!ctrl.shelf.open, "Khong duoc tu mo Khay tep khi dao dang an");
                ctrl.config.visible = true;
                ctrl.execute_ui_action(UiAction::OpenSettings);
                ctrl.animator = IslandAnimator::new(IslandMode::Settings);
                assert_eq!(ctrl.current_effective_mode(), IslandMode::Settings);
            }
            let f3 = tmp_dir.join("extra_in_settings.txt");
            let _ = std::fs::write(&f3, b"123");
            unsafe {
                window_proc(hwnd, WM_APP_SHELF_HOVER, WPARAM(1), LPARAM(0));
            }
            {
                let ctrl = get_controller_mut(hwnd).unwrap();
                assert_eq!(
                    ctrl.current_effective_mode(),
                    IslandMode::Compact,
                    "Keo tep qua dao khi dang mo Settings phai tu quay ve Compact de tha ngay"
                );
                assert!(ctrl.shelf.open);
                let before_count = ctrl.shelf.items.len();
                ctrl.handle_shelf_drop_paths(vec![f3.clone()]);
                assert_eq!(ctrl.shelf.items.len(), before_count + 1);
                assert!(ctrl.shelf.open);

                // Bật Gọn mép -> Slim: mép trên khay mở khớp dock (2.0 DIP), timer không co đảo khi đang kéo, tắt Gọn mép vẫn kéo thả sạch.
                ctrl.shelf.open = false;
                ctrl.execute_ui_action(UiAction::ToggleNotchCollapse);
                let slim_pill = IslandAnimator::new(IslandMode::Slim).current_pill();
                let slim_dock = shelf_pill_rect(slim_pill, IslandMode::Slim, false, 1).unwrap();
                let slim_open = shelf_pill_rect(slim_pill, IslandMode::Slim, true, 1).unwrap();
                assert!((slim_dock.top - slim_open.top).abs() < 0.01);

                ctrl.requested_mode = IslandMode::Compact;
                ctrl.mouse_tracked = false;
                ctrl.shelf.drag_hover = true;
                ctrl.handle_compact_to_notch_timer();
                assert_eq!(
                    ctrl.requested_mode,
                    IslandMode::Compact,
                    "Khong duoc thu ve Slim trong luc dang keo tep"
                );
                ctrl.shelf.drag_hover = false;

                ctrl.execute_ui_action(UiAction::ToggleNotchCollapse);
                assert!(!ctrl.config.notch_collapse_after_4s);
                ctrl.requested_mode = IslandMode::Compact;
                ctrl.animator = IslandAnimator::new(IslandMode::Compact);

                // Kéo ra ngoài khi đang Expanded -> kết thúc bên ngoài phải lên lịch TIMER_AUTO_COLLAPSE_ID.
                ctrl.config.notch_collapse_after_4s = true;
                ctrl.config.auto_hide_when_idle = false;
                ctrl.media_state = MediaDisplayState {
                    has_session: true,
                    ..MediaDisplayState::default()
                };
                ctrl.requested_mode = IslandMode::Expanded;
                ctrl.animator = IslandAnimator::new(IslandMode::Expanded);
                ctrl.mouse_tracked = true;
                ctrl.hovered_action = Some(UiAction::MediaPlayPause);
                let _ = unsafe { KillTimer(Some(hwnd), TIMER_AUTO_COLLAPSE_ID) };
                ctrl.finish_shelf_drag_out_with_presence(false);
                assert!(!ctrl.mouse_tracked);
                assert!(ctrl.hovered_action.is_none());
                assert!(
                    unsafe { KillTimer(Some(hwnd), TIMER_AUTO_COLLAPSE_ID) }.is_ok(),
                    "Ket thuc keo ra ngoai tu Expanded phai hen gio thu gon"
                );
            }
            let _ = std::fs::remove_file(&f3);

            let _ = std::fs::remove_file(&f1);
            let _ = std::fs::remove_file(&f2);
            let _ = std::fs::remove_dir(&tmp_dir);
        }

        // 18. Bấm icon thiết bị âm thanh trên Compact mở nhanh AudioDevices; HUD thiết bị Bluetooth & bật/tắt; AntiExpanded & DismissAnti.
        {
            let ctrl = get_controller_mut(hwnd).unwrap();
            ctrl.config.visible = true;
            ctrl.config.hud_device_enabled = true;
            ctrl.active_hud = None;
            ctrl.anti_slots.clear();
            ctrl.dismissed_anti_keys.clear();
            ctrl.anti_expanded_key = None;
            ctrl.media_state = MediaDisplayState {
                has_session: true,
                is_playing: true,
                ..MediaDisplayState::default()
            };
            ctrl.requested_mode = IslandMode::Compact;
            ctrl.animator = IslandAnimator::new(IslandMode::Compact);
        }
        let compact_pill = get_controller_mut(hwnd).unwrap().animator.current_pill();
        let mut c_boxes = Vec::new();
        fill_compact_media_hitboxes(compact_pill, &mut c_boxes);
        let audio_icon_rect = c_boxes[0].rect;
        let ax = (audio_icon_rect.left + audio_icon_rect.width() * 0.5).round() as i32;
        let ay = (audio_icon_rect.top + audio_icon_rect.height() * 0.5).round() as i32;
        unsafe {
            window_proc(hwnd, WM_LBUTTONDOWN, WPARAM(1), make_lparam_xy(ax, ay));
            window_proc(hwnd, WM_LBUTTONUP, WPARAM(0), make_lparam_xy(ax, ay));
        }
        {
            let ctrl = get_controller_mut(hwnd).unwrap();
            assert_eq!(ctrl.current_effective_mode(), IslandMode::AudioDevices);
            ctrl.execute_ui_action(UiAction::BackToMedia);
            assert_eq!(ctrl.current_effective_mode(), IslandMode::Compact);

            ctrl.hud_device_name = Some(String::from("Diamond Stereo"));
            ctrl.hud_device_container_key = Some(String::from("{BT1}"));
            ctrl.show_hud_with_duration(
                HudContent::Device {
                    category: DeviceHudCategory::Bluetooth,
                    connected: true,
                    battery_percent: Some(85),
                },
                DEVICE_HUD_EXPIRE_DELAY_MS,
            );
            assert!(matches!(
                ctrl.active_hud,
                Some(HudContent::Device {
                    category: DeviceHudCategory::Bluetooth,
                    connected: true,
                    battery_percent: Some(85)
                })
            ));
            ctrl.execute_ui_action(UiAction::ToggleHudDevice);
            assert!(!ctrl.config.hud_device_enabled);
            assert!(ctrl.active_hud.is_none());
            ctrl.config.hud_device_enabled = true;

            ctrl.anti_slots = vec![AntiProgressSnapshot {
                source: crate::anti::AntiSource::AntiIde,
                conversation_id: String::from("conv-ide"),
                state: AntiAgentState::WaitingApproval,
                detail: String::from("Chờ bạn xác nhận"),
                recent_steps: vec![String::from("Đang chờ duyệt: Chờ bạn xác nhận")],
                had_error: false,
                updated_unix_ms: now_unix_ms(),
            }];
            ctrl.requested_mode = IslandMode::Compact;
            ctrl.animator = IslandAnimator::new(IslandMode::Compact);
        }
        let cp_anti = get_controller_mut(hwnd).unwrap().animator.current_pill();
        let anti_x = (cp_anti.left + 80.0).round() as i32;
        let anti_y = (cp_anti.top + 16.0).round() as i32;
        unsafe {
            window_proc(hwnd, WM_LBUTTONDOWN, WPARAM(1), make_lparam_xy(anti_x, anti_y));
            window_proc(hwnd, WM_LBUTTONUP, WPARAM(0), make_lparam_xy(anti_x, anti_y));
        }
        {
            let ctrl = get_controller_mut(hwnd).unwrap();
            assert_eq!(ctrl.current_effective_mode(), IslandMode::AntiExpanded);
            ctrl.execute_ui_action(UiAction::DismissAnti);
            assert!(ctrl.anti_display_snapshot().is_none());
            assert_ne!(ctrl.current_effective_mode(), IslandMode::AntiExpanded);
        }

        // 19. Mở 2 Anti đồng thời: chính ở giữa, phụ bên trái; bấm viên trái mở AntiExpanded đúng phiên phụ & xuyên chuột khoảng trống.
        {
            let ctrl = get_controller_mut(hwnd).unwrap();
            ctrl.dismissed_anti_keys.clear();
            ctrl.anti_expanded_key = None;
            let now = now_unix_ms();
            ctrl.anti_slots = vec![
                AntiProgressSnapshot {
                    source: crate::anti::AntiSource::AgyCli,
                    conversation_id: String::from("conv-cli"),
                    state: AntiAgentState::Building,
                    detail: String::from("cargo build"),
                    recent_steps: vec![String::from("Đang build: cargo build")],
                    had_error: false,
                    updated_unix_ms: now,
                },
                AntiProgressSnapshot {
                    source: crate::anti::AntiSource::AntiIde,
                    conversation_id: String::from("conv-ide-2"),
                    state: AntiAgentState::Coding,
                    detail: String::from("Sửa renderer"),
                    recent_steps: vec![String::from("Đang code: Sửa renderer")],
                    had_error: false,
                    updated_unix_ms: now.saturating_sub(200),
                },
            ];
            ctrl.requested_mode = IslandMode::Slim;
            ctrl.animator = IslandAnimator::new(IslandMode::Compact);
            ctrl.apply_target_and_render();
            assert_eq!(
                ctrl.anti_display_snapshot().map(|s| s.source),
                Some(crate::anti::AntiSource::AgyCli)
            );
            assert_eq!(
                ctrl.anti_secondary_snapshot().map(|s| s.source),
                Some(crate::anti::AntiSource::AntiIde)
            );
        }
        let (left_pt, gap_pt) = {
            let ctrl = get_controller_mut(hwnd).unwrap();
            let pill = ctrl.animator.current_pill();
            let left_rect = ctrl.active_left_anti_rect(pill).expect("left anti rect");
            assert!(left_rect.left + left_rect.width <= pill.left - 8.0);
            let l_pt = pill_to_screen(left_rect.left + 40.0, left_rect.top + 16.0);
            let g_pt = pill_to_screen(pill.left - 5.0, left_rect.top + 16.0);
            (l_pt, g_pt)
        };
        assert_eq!(
            unsafe { window_proc(hwnd, WM_NCHITTEST, WPARAM(0), make_lparam_xy(left_pt.x, left_pt.y)) }.0,
            HTCLIENT as isize
        );
        assert_eq!(
            unsafe { window_proc(hwnd, WM_NCHITTEST, WPARAM(0), make_lparam_xy(gap_pt.x, gap_pt.y)) }.0,
            HTTRANSPARENT as isize
        );
        let left_click_local = {
            let ctrl = get_controller_mut(hwnd).unwrap();
            let lr = ctrl.active_left_anti_rect(ctrl.animator.current_pill()).unwrap();
            ((lr.left + 30.0).round() as i32, (lr.top + 16.0).round() as i32)
        };
        unsafe {
            window_proc(
                hwnd,
                WM_LBUTTONDOWN,
                WPARAM(1),
                make_lparam_xy(left_click_local.0, left_click_local.1),
            );
            window_proc(
                hwnd,
                WM_LBUTTONUP,
                WPARAM(0),
                make_lparam_xy(left_click_local.0, left_click_local.1),
            );
        }
        {
            let ctrl = get_controller_mut(hwnd).unwrap();
            assert_eq!(ctrl.current_effective_mode(), IslandMode::AntiExpanded);
            assert_eq!(
                ctrl.anti_display_snapshot().map(|s| s.source),
                Some(crate::anti::AntiSource::AntiIde)
            );
            assert_eq!(
                ctrl.anti_secondary_snapshot().map(|s| s.source),
                Some(crate::anti::AntiSource::AgyCli)
            );
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
