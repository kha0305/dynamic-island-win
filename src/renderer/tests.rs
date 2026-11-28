use super::*;
use crate::state::IslandAnimator;
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_CPU_READ, D2D1_BITMAP_OPTIONS_TARGET,
    D2D1_MAP_OPTIONS_READ,
};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP,
    WS_EX_TOOLWINDOW, WS_POPUP,
};

#[test]
fn low_power_adapter_prefers_integrated_gpu() {
    let factory6: IDXGIFactory6 =
        unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)) }
            .expect("CreateDXGIFactory2 must succeed");
    let (_dev, name) =
        create_low_power_d3d11_device(&factory6).expect("Low-power D3D11 device must succeed");
    println!("Selected DXGI minimum-power adapter: {name}");
    assert!(!name.is_empty());
    assert!(
        !name.to_ascii_uppercase().contains("NVIDIA"),
        "Adapter duoc chon khong duoc la GPU roi NVIDIA: {name}"
    );
}

#[test]
fn winrt_glass_backdrop_initializes_and_renders() {
    struct ComGuard(bool);
    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }

    let _com = ComGuard(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok());
    let hinstance = unsafe { GetModuleHandleW(None) }.expect("GetModuleHandleW");
    let class_name = to_wide_null("STATIC");
    let title = to_wide_null("GlassTestWindow");
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP | WS_EX_NOACTIVATE,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_POPUP,
            0,
            0,
            444,
            200,
            None,
            None,
            Some(hinstance.into()),
            None,
        )
    }
    .expect("CreateWindowExW");

    let mut renderer = IslandRenderer::new(hwnd, 1.0).expect("IslandRenderer::new");
    println!(
        "WinRT Glass backdrop active: {} | Blur effect active: {} | GPU: {}",
        renderer.has_glass_backdrop(),
        renderer.has_blur_effect(),
        renderer.adapter_name()
    );
    assert!(
        renderer.has_glass_backdrop(),
        "HostBackdropBrush phai hoat dong tren Windows 11"
    );
    assert!(
        renderer.has_blur_effect(),
        "GaussianBlur CompositionEffectBrush phai hoat dong tren Windows 11"
    );

    let media = MediaDisplayState::default();
    let sample_devices = vec![
        AudioOutputDevice {
            id: String::from("dev-1"),
            name: String::from("Speakers (Realtek Audio)"),
            is_default: true,
        },
        AudioOutputDevice {
            id: String::from("dev-2"),
            name: String::from("Headphones (USB DAC)"),
            is_default: false,
        },
    ];
    let vol_snap = VolumeSnapshot {
        level: 0.64,
        muted: false,
        available: true,
    };
    for mode in [
        IslandMode::Slim,
        IslandMode::Compact,
        IslandMode::Expanded,
        IslandMode::Settings,
        IslandMode::AudioDevices,
        IslandMode::Notice,
        IslandMode::Hidden,
    ] {
        let pill = IslandAnimator::new(mode).current_pill();
        let sample_notice = if mode == IslandMode::Notice {
            Some(IslandNoticeView {
                kind: NoticeKind::UpToDate,
                title: "Đã là bản mới nhất",
                message: "Bạn đang sử dụng phiên bản mới nhất (v0.1.3).",
                primary_label: "Đã hiểu",
                secondary_label: None,
            })
        } else {
            None
        };
        let input = RenderFrameInput {
            pill,
            mode,
            pill_opacity: if mode == IslandMode::Hidden { 0.0 } else { 1.0 },
            content_alpha: if mode == IslandMode::Hidden { 0.0 } else { 1.0 },
            config: AppConfig::default(),
            media: &media,
            active_hud: None,
            volume_scrub_ratio: None,
            hovered_action: None,
            timeline_scrub_ratio: None,
            monitor_label: "M1",
            visualizer_phase: 0.4,
            audio_devices: &sample_devices,
            audio_device_scroll: 0,
            volume_snapshot: vol_snap,
            current_audio_device_name: Some("Speakers (Realtek Audio)"),
            notice: sample_notice,
            available_update_tag: Some("v0.2.2"),
            anti: None,
            anti_hooks_installed: false,
            anti_spinner_phase: 0.0,
            shelf: ShelfRenderInput::default(),
        };
        renderer.render(&input).expect("renderer.render");
    }
    for (kind, sec) in [
        (NoticeKind::Checking, None),
        (NoticeKind::UpdateAvailable, Some("Để sau")),
        (NoticeKind::Downloading, None),
        (NoticeKind::Error, Some("Thử lại")),
    ] {
        let notice_input = RenderFrameInput {
            pill: IslandAnimator::new(IslandMode::Notice).current_pill(),
            mode: IslandMode::Notice,
            pill_opacity: 1.0,
            content_alpha: 1.0,
            config: AppConfig::default(),
            media: &media,
            active_hud: None,
            volume_scrub_ratio: None,
            hovered_action: Some(UiAction::NoticePrimary),
            timeline_scrub_ratio: None,
            monitor_label: "M1",
            visualizer_phase: 0.0,
            audio_devices: &[],
            audio_device_scroll: 0,
            volume_snapshot: vol_snap,
            current_audio_device_name: None,
            notice: Some(IslandNoticeView {
                kind,
                title: "Kiểm tra thông báo",
                message: "Nội dung thông báo mẫu nhiều dòng hiển thị đẹp trên thẻ kính khói.",
                primary_label: "Cập nhật ngay",
                secondary_label: sec,
            }),
            available_update_tag: None,
            anti: None,
            anti_hooks_installed: false,
            anti_spinner_phase: 0.0,
            shelf: ShelfRenderInput::default(),
        };
        renderer
            .render(&notice_input)
            .expect("render IslandNoticeView");
    }
    let sample_art = DecodedArtwork {
        width: 32,
        height: 20,
        bgra: [30, 50, 210, 255].repeat(32 * 20),
    };
    renderer.update_artwork(ArtworkUpdate::Set(sample_art));
    assert!(renderer.artwork_aura_bitmap.is_some());
    let mut active_media = media.clone();
    active_media.has_session = true;
    active_media.is_playing = true;
    active_media.timeline = crate::state::TimelineState {
        available: true,
        start_ticks: 0,
        end_ticks: 100_000_000,
        base_position_ticks: 45_000_000,
        last_updated_filetime_ticks: 0,
    };
    let slim_active_input = RenderFrameInput {
        pill: IslandAnimator::new(IslandMode::Slim).current_pill(),
        mode: IslandMode::Slim,
        pill_opacity: 1.0,
        content_alpha: 1.0,
        config: AppConfig::default(),
        media: &active_media,
        active_hud: None,
        volume_scrub_ratio: None,
        hovered_action: None,
        timeline_scrub_ratio: None,
        monitor_label: "M1",
        visualizer_phase: 0.4,
        audio_devices: &sample_devices,
        audio_device_scroll: 0,
        volume_snapshot: vol_snap,
        current_audio_device_name: Some("Speakers (Realtek Audio)"),
        notice: None,
        available_update_tag: None,
        anti: None,
        anti_hooks_installed: false,
        anti_spinner_phase: 0.0,
        shelf: ShelfRenderInput::default(),
    };
    renderer
        .render(&slim_active_input)
        .expect("render active Slim Notch");
    let compact_active_input = RenderFrameInput {
        pill: IslandAnimator::new(IslandMode::Compact).current_pill(),
        mode: IslandMode::Compact,
        pill_opacity: 1.0,
        content_alpha: 1.0,
        config: AppConfig::default(),
        media: &active_media,
        active_hud: None,
        volume_scrub_ratio: None,
        hovered_action: None,
        timeline_scrub_ratio: None,
        monitor_label: "M1",
        visualizer_phase: 0.4,
        audio_devices: &sample_devices,
        audio_device_scroll: 0,
        volume_snapshot: vol_snap,
        current_audio_device_name: Some("Speakers (Realtek Audio)"),
        notice: None,
        available_update_tag: None,
        anti: None,
        anti_hooks_installed: false,
        anti_spinner_phase: 0.0,
        shelf: ShelfRenderInput::default(),
    };
    renderer
        .render(&compact_active_input)
        .expect("render active Compact pill");
    let mut paused_media = active_media.clone();
    paused_media.is_playing = false;
    let compact_paused_input = RenderFrameInput {
        media: &paused_media,
        ..compact_active_input
    };
    renderer
        .render(&compact_paused_input)
        .expect("render paused Compact pill");
    let expanded_input = RenderFrameInput {
        pill: IslandAnimator::new(IslandMode::Expanded).current_pill(),
        mode: IslandMode::Expanded,
        pill_opacity: 1.0,
        content_alpha: 1.0,
        config: AppConfig::default(),
        media: &active_media,
        active_hud: None,
        volume_scrub_ratio: None,
        hovered_action: None,
        timeline_scrub_ratio: None,
        monitor_label: "M1",
        visualizer_phase: 0.4,
        audio_devices: &sample_devices,
        audio_device_scroll: 0,
        volume_snapshot: vol_snap,
        current_audio_device_name: Some("Speakers (Realtek Audio)"),
        notice: None,
        available_update_tag: None,
        anti: None,
        anti_hooks_installed: false,
        anti_spinner_phase: 0.0,
        shelf: ShelfRenderInput::default(),
    };
    renderer
        .render(&expanded_input)
        .expect("render with artwork aura");
    let mut dev_anim = IslandAnimator::new(IslandMode::Slim);
    dev_anim.set_target_mode_with_audio_count(IslandMode::AudioDevices, sample_devices.len());
    let mut t = std::time::Instant::now();
    for _ in 0..300 {
        t += std::time::Duration::from_millis(8);
        if !dev_anim.step(t) {
            break;
        }
    }
    let audio_dev_input = RenderFrameInput {
        pill: dev_anim.current_pill(),
        mode: IslandMode::AudioDevices,
        pill_opacity: 1.0,
        content_alpha: 1.0,
        config: AppConfig::default(),
        media: &active_media,
        active_hud: None,
        volume_scrub_ratio: None,
        hovered_action: None,
        timeline_scrub_ratio: None,
        monitor_label: "M1",
        visualizer_phase: 0.0,
        audio_devices: &sample_devices,
        audio_device_scroll: 0,
        volume_snapshot: vol_snap,
        current_audio_device_name: Some("Speakers (Realtek Audio)"),
        notice: None,
        available_update_tag: None,
        anti: None,
        anti_hooks_installed: false,
        anti_spinner_phase: 0.0,
        shelf: ShelfRenderInput::default(),
    };
    renderer
        .render(&audio_dev_input)
        .expect("render compact dynamic AudioDevices");
    for hud in [
        HudContent::Volume {
            level: 0.64,
            muted: false,
            show_device_name: false,
        },
        HudContent::Volume {
            level: 0.64,
            muted: false,
            show_device_name: true,
        },
        HudContent::Volume {
            level: 0.64,
            muted: true,
            show_device_name: false,
        },
        HudContent::Power {
            on_ac: true,
            percent: Some(84),
            low_battery: false,
        },
        HudContent::Power {
            on_ac: false,
            percent: Some(12),
            low_battery: true,
        },
    ] {
        let hud_input = RenderFrameInput {
            pill: IslandAnimator::new(IslandMode::Compact).current_pill(),
            mode: IslandMode::Compact,
            pill_opacity: 1.0,
            content_alpha: 1.0,
            config: AppConfig::default(),
            media: &active_media,
            active_hud: Some(hud),
            volume_scrub_ratio: None,
            hovered_action: Some(UiAction::SeekVolume),
            timeline_scrub_ratio: None,
            monitor_label: "M1",
            visualizer_phase: 0.0,
            audio_devices: &sample_devices,
            audio_device_scroll: 0,
            volume_snapshot: vol_snap,
            current_audio_device_name: Some("Headphones (USB DAC)"),
            notice: None,
            available_update_tag: None,
            anti: None,
            anti_hooks_installed: false,
            anti_spinner_phase: 0.0,
            shelf: ShelfRenderInput::default(),
        };
        renderer.render(&hud_input).expect("render HUD Compact");
    }
    renderer.update_artwork(ArtworkUpdate::Clear);
    assert!(renderer.artwork_aura_bitmap.is_none());

    renderer.hide_backdrop();
    drop(renderer);
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
}

#[test]
fn artwork_aura_and_accent_match_video_colors() {
    // Nửa trái đỏ ấm, nửa phải xanh lam -> lưới màu và điểm nhấn phải giữ đúng tương quan màu.
    let w = 16_usize;
    let h = 10_usize;
    let mut bgra = Vec::with_capacity(w * h * 4);
    for _y in 0..h {
        for x in 0..w {
            if x < w / 2 {
                bgra.extend_from_slice(&[28, 45, 220, 255]); // Đỏ ấm (BGR)
            } else {
                bgra.extend_from_slice(&[215, 130, 30, 255]); // Xanh dương (BGR)
            }
        }
    }
    let art = DecodedArtwork {
        width: w as u32,
        height: h as u32,
        bgra,
    };
    let (aura, aw, ah) = build_ambient_aura_bgra(&art);
    assert_eq!((aw, ah), (16, 10));
    // Cột trái ưu tiên đỏ (R > B), cột phải ưu tiên xanh (B > R).
    let left_idx = (5 * 16 + 1) * 4;
    let right_idx = (5 * 16 + 14) * 4;
    assert!(aura[left_idx + 2] > aura[left_idx]);
    assert!(aura[right_idx] > aura[right_idx + 2]);

    let accent = boost_accent_for_readability(extract_vibrant_accent_rgb(&art.bgra));
    assert!(accent.iter().copied().fold(0.0_f32, f32::max) >= 0.70);

    let white_art = DecodedArtwork {
        width: 8,
        height: 8,
        bgra: [255, 255, 255, 255].repeat(64),
    };
    let (white_aura, _, _) = build_ambient_aura_bgra(&white_art);
    assert!(
        white_aura[2] <= 168 && white_aura[1] <= 168 && white_aura[0] <= 168,
        "Aura từ ảnh trắng phải được hãm sáng để không tẩy nhạt chữ: {:?}",
        &white_aura[0..4]
    );
}

#[test]
fn smoked_glass_preserves_translucency_and_contrast_over_white() {
    fn srgb_to_rel_lum(r: f32, g: f32, b: f32) -> f32 {
        let lin = |c: f32| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
    }

    struct ComGuard(bool);
    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }

    let _com = ComGuard(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok());
    let hinstance = unsafe { GetModuleHandleW(None) }.expect("GetModuleHandleW");
    let class_name = to_wide_null("STATIC");
    let title = to_wide_null("ContrastTestWindow");
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP | WS_EX_NOACTIVATE,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_POPUP,
            0,
            0,
            444,
            260,
            None,
            None,
            Some(hinstance.into()),
            None,
        )
    }
    .expect("CreateWindowExW");

    let mut renderer = IslandRenderer::new(hwnd, 1.0).expect("IslandRenderer::new");
    let size_px = compute_surface_size_px(1.0);
    let bmp_size = D2D_SIZE_U {
        width: size_px.0 as u32,
        height: size_px.1 as u32,
    };
    let target_props = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
        ..Default::default()
    };
    let gpu_bmp = unsafe {
        renderer
            .d2d_context
            .CreateBitmap(bmp_size, None, 0, &target_props)
    }
    .expect("Create offscreen target bitmap");
    let target_img: ID2D1Image = gpu_bmp.cast().expect("ID2D1Image");
    let pill = IslandAnimator::new(IslandMode::Settings).current_pill();
    let media = MediaDisplayState::default();
    let input = RenderFrameInput {
        pill,
        mode: IslandMode::Settings,
        pill_opacity: 1.0,
        content_alpha: 1.0,
        config: AppConfig::default(),
        media: &media,
        active_hud: None,
        volume_scrub_ratio: None,
        hovered_action: None,
        timeline_scrub_ratio: None,
        monitor_label: "1 - Chính",
        visualizer_phase: 0.0,
        audio_devices: &[],
        audio_device_scroll: 0,
        volume_snapshot: VolumeSnapshot::default(),
        current_audio_device_name: None,
        notice: None,
        available_update_tag: None,
        anti: None,
        anti_hooks_installed: false,
        anti_spinner_phase: 0.0,
        shelf: ShelfRenderInput::default(),
    };
    let clear = D2D1_COLOR_F {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
    unsafe {
        let off_ctx = renderer.d2d_context.clone();
        let canvas = DrawCanvas {
            ctx: &off_ctx,
            scale: 1.0,
            offset_x_px: 0.0,
            offset_y_px: 0.0,
        };
        off_ctx.SetTarget(&target_img);
        off_ctx.BeginDraw();
        off_ctx.SetDpi(96.0, 96.0);
        off_ctx.SetTransform(&canvas.base_transform());
        off_ctx.Clear(Some(&clear));
        renderer.draw_pill_shell(canvas, pill, 1.0, true, false);
        renderer.draw_clipped_contents(canvas, &input, 1.0);
        off_ctx.EndDraw(None, None).expect("EndDraw");
    }

    let cpu_props = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        ..Default::default()
    };
    let cpu_bmp = unsafe {
        renderer
            .d2d_context
            .CreateBitmap(bmp_size, None, 0, &cpu_props)
    }
    .expect("Create CPU_READ bitmap");
    let src_bmp: ID2D1Bitmap = gpu_bmp.cast().expect("ID2D1Bitmap");
    unsafe {
        cpu_bmp
            .CopyFromBitmap(None, &src_bmp, None)
            .expect("CopyFromBitmap");
    }
    let mapped = unsafe { cpu_bmp.Map(D2D1_MAP_OPTIONS_READ) }.expect("Map");
    let pitch = mapped.pitch as usize;
    let total_bytes = pitch * bmp_size.height as usize;
    let buf = unsafe { std::slice::from_raw_parts(mapped.bits, total_bytes) };

    let sample_over_white = |x_f: f32, y_f: f32| -> (f32, f32, f32, f32) {
        let x = (x_f.round() as usize).min(bmp_size.width as usize - 1);
        let y = (y_f.round() as usize).min(bmp_size.height as usize - 1);
        let off = y * pitch + x * 4;
        let b_p = buf[off] as f32 / 255.0;
        let g_p = buf[off + 1] as f32 / 255.0;
        let r_p = buf[off + 2] as f32 / 255.0;
        let a = buf[off + 3] as f32 / 255.0;
        // Hòa trộn PREMULTIPLIED lên nền trắng thuần (1.0, 1.0, 1.0)
        (r_p + (1.0 - a), g_p + (1.0 - a), b_p + (1.0 - a), a)
    };

    let glass = sample_over_white(pill.left + pill.width - 18.0, pill.top + 45.0);
    let card = sample_over_white(pill.left + pill.width - 20.0, pill.top + 14.0);
    let exit = sample_over_white(pill.left + pill.width - 24.0, pill.top + pill.height - 16.0);
    let mut check_hbs = Vec::new();
    fill_interactive_hitboxes(pill, IslandMode::Settings, &mut check_hbs);
    let check_rect = check_hbs
        .iter()
        .find(|hb| hb.action == UiAction::CheckUpdate)
        .expect("CheckUpdate hitbox")
        .rect;
    let ver_text = format!("v{APP_VERSION}");
    let ver_w = unsafe {
        renderer.measure_text_width(
            &ver_text,
            TextFormatSlot::SmallRight,
            62.0,
            check_rect.height(),
        )
    }
    .expect("measure version text width");
    let live_dot_cx = IslandRenderer::check_update_live_dot_cx(check_rect.right, ver_w);
    let live_dot_cy = check_rect.top + check_rect.height() * 0.5;
    let live_dot = sample_over_white(live_dot_cx, live_dot_cy);

    unsafe {
        let _ = cpu_bmp.Unmap();
    }
    renderer.hide_backdrop();
    drop(renderer);
    unsafe {
        let _ = DestroyWindow(hwnd);
    }

    let glass_contrast = (1.0 + 0.05) / (srgb_to_rel_lum(glass.0, glass.1, glass.2) + 0.05);
    println!(
        "Over pure #FFFFFF -> glass={glass:?} (contrast={glass_contrast:.2}:1), card={card:?}, exit={exit:?}, live_dot={live_dot:?} (ver_w={ver_w:.1})"
    );

    assert!(
        glass.3 >= 0.55 && glass.3 <= 0.80,
        "Kính phải giữ độ xuyên sáng vừa phải (0.55..0.80), hiện tại: {:.3}",
        glass.3
    );
    assert!(
        glass_contrast >= 4.5,
        "Độ tương phản chữ trắng trên kính đặt lên nền #FFFFFF phải >= 4.5:1, hiện tại: {glass_contrast:.2}"
    );
    assert!(
        card.0 <= 0.32 && card.1 <= 0.34 && card.2 <= 0.38,
        "Thẻ cài đặt trên nền trắng phải đầm tối, không tẩy xám sáng: {card:?}"
    );
    assert!(
        exit.0 > exit.1 * 1.45 && exit.0 <= 0.62,
        "Nút Thoát ứng dụng trên nền trắng phải giữ đỏ trầm, không bạc hồng: {exit:?}"
    );
    assert!(
        live_dot.1 > live_dot.0 * 1.35 && live_dot.1 > live_dot.2 * 1.20,
        "Chấm live cạnh phiên bản phải hiển thị xanh lục sắc nét: {live_dot:?}"
    );
}

#[test]
fn file_shelf_and_anti_spinner_render_cleanly_offscreen() {
    struct ComGuard(bool);
    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }

    let _com = ComGuard(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok());
    let hinstance = unsafe { GetModuleHandleW(None) }.expect("GetModuleHandleW");
    let class_name = to_wide_null("STATIC");
    let title = to_wide_null("ShelfAntiRenderTestWindow");
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP | WS_EX_NOACTIVATE,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_POPUP,
            0,
            0,
            1040,
            440,
            None,
            None,
            Some(hinstance.into()),
            None,
        )
    }
    .expect("CreateWindowExW");

    let mut renderer = IslandRenderer::new(hwnd, 1.0).expect("IslandRenderer::new");
    let media = MediaDisplayState::default();
    let anti = crate::anti::AntiProgressSnapshot {
        state: crate::anti::AntiAgentState::Coding,
        source: crate::anti::AntiSource::AgyCli,
        detail: String::from("Đang ghép File Shelf"),
        conversation_id: String::new(),
        had_error: false,
        updated_unix_ms: 0,
    };
    let sample_items = [
        crate::shelf::ShelfItem {
            path: std::path::PathBuf::from(r"C:\Temp\bao_cao_tai_chinh.pdf"),
            name: String::from("bao_cao_tai_chinh.pdf"),
            size_label: String::from("1.4 MB"),
            ext_badge: String::from("PDF"),
            selected: false,
        },
        crate::shelf::ShelfItem {
            path: std::path::PathBuf::from(r"C:\Temp\anh_chup_man_hinh.png"),
            name: String::from("anh_chup_man_hinh.png"),
            size_label: String::from("320 KB"),
            ext_badge: String::from("PNG"),
            selected: true,
        },
    ];

    for (open, items, hover) in [
        (false, &[][..], None),
        (true, &sample_items[..1], Some(UiAction::ShelfToggleSelect(0))),
        (true, &sample_items[..], Some(UiAction::ShelfDeleteAll)),
    ] {
        let input = RenderFrameInput {
            pill: IslandAnimator::new(IslandMode::Compact).current_pill(),
            mode: IslandMode::Compact,
            pill_opacity: 1.0,
            content_alpha: 1.0,
            config: AppConfig::default(),
            media: &media,
            active_hud: None,
            volume_scrub_ratio: None,
            hovered_action: hover,
            timeline_scrub_ratio: None,
            monitor_label: "1 - Chính",
            visualizer_phase: 0.2,
            audio_devices: &[],
            audio_device_scroll: 0,
            volume_snapshot: VolumeSnapshot::default(),
            current_audio_device_name: None,
            notice: None,
            available_update_tag: Some("v0.3.0"),
            anti: Some(&anti),
            anti_hooks_installed: true,
            anti_spinner_phase: 1.75,
            shelf: ShelfRenderInput {
                visible: true,
                open,
                drag_hover: !open,
                items,
                scroll: 0,
            },
        };
        renderer.render(&input).expect("render Anti + Shelf");
    }

    renderer.hide_backdrop();
    drop(renderer);
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
}
