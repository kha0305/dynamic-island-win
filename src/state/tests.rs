use std::time::Instant;

use super::animator::SpringScalar;
use super::*;

#[test]
fn spring_converges_and_stays_stable_on_large_dt_spike() {
    let mut spring = SpringScalar::new(156.0);
    spring.set_target(424.0);
    // Giả lập frame bị trễ nặng 1.0 giây vẫn không được nổ giá trị.
    let _ = spring.step(1.0, 330.0, 28.5);
    assert!(spring.value.is_finite());
    assert!(spring.value > 156.0 && spring.value < 600.0);

    for _ in 0..240 {
        if !spring.step(1.0 / 120.0, 330.0, 28.5) {
            break;
        }
    }
    assert!(spring.is_settled());
    assert_eq!(spring.value, 424.0);
    assert_eq!(spring.velocity, 0.0);
}

#[test]
fn spring_preserves_velocity_when_interrupted() {
    let mut spring = SpringScalar::new(156.0);
    spring.set_target(412.0);
    for _ in 0..8 {
        spring.step(1.0 / 120.0, 330.0, 28.5);
    }
    let mid_val = spring.value;
    let mid_vel = spring.velocity;
    assert!(mid_vel > 0.0);

    spring.set_target(156.0);
    assert_eq!(spring.value, mid_val);
    assert_eq!(spring.velocity, mid_vel);
}

#[test]
fn animator_stops_after_reaching_target() {
    let mut anim = IslandAnimator::new(IslandMode::Slim);
    assert!(!anim.is_animating());

    anim.set_target_mode(IslandMode::Expanded);
    assert!(anim.is_animating());

    let mut now = Instant::now();
    let mut ticks = 0;
    while ticks < 300 {
        now += std::time::Duration::from_millis(8);
        ticks += 1;
        if !anim.step(now) {
            break;
        }
    }
    assert!(!anim.is_animating());
    let pill = anim.current_pill();
    assert!((pill.width - 424.0).abs() < 0.5);
    assert!((pill.height - 160.0).abs() < 0.5);
}

#[test]
fn settings_hitboxes_stay_inside_pill_and_do_not_overlap() {
    let mut anim = IslandAnimator::new(IslandMode::Settings);
    let pill = anim.current_pill();
    let mut hitboxes = Vec::with_capacity(17);
    fill_interactive_hitboxes(pill, IslandMode::Settings, &mut hitboxes);
    assert_eq!(hitboxes.len(), 17);
    assert!(hitboxes.iter().any(|hb| hb.action == UiAction::CheckUpdate));
    assert!(hitboxes.iter().any(|hb| hb.action == UiAction::OpenAudioDevices));
    assert!(hitboxes.iter().any(|hb| hb.action == UiAction::ToggleHudDevice));
    assert!(hitboxes.iter().any(|hb| hb.action == UiAction::ToggleAntiMonitor));
    assert!(hitboxes.iter().any(|hb| hb.action == UiAction::ToggleAntiHooks));

    for hb in &hitboxes {
        assert!(hb.rect.left >= pill.left);
        assert!(hb.rect.right <= pill.left + pill.width);
        assert!(hb.rect.top >= pill.top);
        assert!(hb.rect.bottom <= pill.top + pill.height);
        assert!(hb.rect.width() > 30.0);
        assert!(hb.rect.height() >= 24.0);
    }

    for i in 0..hitboxes.len() {
        for j in (i + 1)..hitboxes.len() {
            let a = hitboxes[i].rect;
            let b = hitboxes[j].rect;
            let overlap_x = a.left < b.right && a.right > b.left;
            let overlap_y = a.top < b.bottom && a.bottom > b.top;
            assert!(
                !(overlap_x && overlap_y),
                "Hitbox {i} va {j} bi chong cheo: {:?} vs {:?}",
                a,
                b
            );
        }
    }
    anim.set_target_mode(IslandMode::Slim);
}

#[test]
fn expanded_hitboxes_stay_inside_pill_and_do_not_overlap() {
    let anim = IslandAnimator::new(IslandMode::Expanded);
    let pill = anim.current_pill();
    let mut hitboxes = Vec::with_capacity(6);
    fill_interactive_hitboxes(pill, IslandMode::Expanded, &mut hitboxes);
    assert_eq!(hitboxes.len(), 6);
    let audio_rect = hitboxes
        .iter()
        .find(|hb| hb.action == UiAction::OpenAudioDevices)
        .expect("OpenAudioDevices")
        .rect;
    let prev_rect = hitboxes
        .iter()
        .find(|hb| hb.action == UiAction::MediaPrev)
        .expect("MediaPrev")
        .rect;
    let next_rect = hitboxes
        .iter()
        .find(|hb| hb.action == UiAction::MediaNext)
        .expect("MediaNext")
        .rect;
    let settings_rect = hitboxes
        .iter()
        .find(|hb| hb.action == UiAction::OpenSettings)
        .expect("OpenSettings")
        .rect;
    assert!(audio_rect.right <= prev_rect.left);
    assert!(next_rect.right <= settings_rect.left);
    assert!((settings_rect.width() - 94.0).abs() < 0.1);
    assert!(audio_rect.width() >= 120.0);
    assert!(settings_rect.top >= pill.top + 110.0);

    for hb in &hitboxes {
        assert!(hb.rect.left >= pill.left);
        assert!(hb.rect.right <= pill.left + pill.width);
        assert!(hb.rect.top >= pill.top);
        assert!(hb.rect.bottom <= pill.top + pill.height);
        if hb.action == UiAction::SeekTimeline {
            assert!(hb.rect.height() >= 26.0);
        }
    }

    for i in 0..hitboxes.len() {
        for j in (i + 1)..hitboxes.len() {
            let a = hitboxes[i].rect;
            let b = hitboxes[j].rect;
            let overlap_x = a.left < b.right && a.right > b.left;
            let overlap_y = a.top < b.bottom && a.bottom > b.top;
            assert!(!(overlap_x && overlap_y));
        }
    }

    let tl = hitboxes
        .iter()
        .find(|hb| hb.action == UiAction::SeekTimeline)
        .expect("SeekTimeline")
        .rect;
    let grab_bounds = timeline_seek_hit_bounds(tl);
    assert!(grab_bounds.contains(tl.left - 5.0, tl.top + 10.0));
    assert_eq!(timeline_ratio_for_x(tl, tl.left - 10.0), 0.0);
    assert_eq!(timeline_ratio_for_x(tl, tl.right + 10.0), 1.0);
    assert!(should_collapse_expanded_on_blank_click(
        pill,
        (pill.left + 120.0, pill.top + 30.0),
        (pill.left + 121.0, pill.top + 31.0),
    ));
    assert!(!should_collapse_expanded_on_blank_click(
        pill,
        (pill.left + 120.0, pill.top + 30.0),
        (pill.left + 160.0, pill.top + 32.0),
    ));
    assert!(!should_collapse_expanded_on_blank_click(
        pill,
        (pill.left + 32.0, pill.top + 96.0),
        (pill.left + 32.0, pill.top + 96.0),
    ));
}

#[test]
fn timeline_progress_and_formatting_work_accurately() {
    let tl = TimelineState {
        available: true,
        start_ticks: 0,
        end_ticks: 250 * 10_000_000,
        base_position_ticks: 60 * 10_000_000,
        last_updated_filetime_ticks: 1_000_000_000,
    };
    let now = 1_000_000_000 + 24 * 10_000_000;
    assert_eq!(tl.current_position_ticks(now, true), 84 * 10_000_000);
    assert_eq!(tl.current_position_ticks(now, false), 60 * 10_000_000);
    assert_eq!(format_duration_ticks(84 * 10_000_000), "01:24");
    assert_eq!(format_duration_ticks(3665 * 10_000_000), "1:01:05");
    assert_eq!(tl.seek_ticks_for_ratio(0.5), Some(125 * 10_000_000));
}

#[test]
fn center_window_on_primary_and_negative_secondary_monitors() {
    let (sw, _sh) = compute_surface_size_px(1.0);
    let (x, y) = compute_window_origin_px(0, 1920, 0, sw);
    assert_eq!(x, (1920 - sw) / 2);
    assert_eq!(y, 0);

    let (sw150, _) = compute_surface_size_px(1.5);
    let (x2, y2) = compute_window_origin_px(-1920, 0, 32, sw150);
    assert_eq!(x2, -1920 + (1920 - sw150) / 2);
    assert_eq!(y2, 32);
}

#[test]
fn effective_mode_respects_fullscreen_visibility_and_auto_hide() {
    let cfg = AppConfig::default();
    assert_eq!(
        effective_mode(IslandMode::Slim, cfg, false, false, false),
        IslandMode::Slim
    );
    assert_eq!(
        effective_mode(IslandMode::Expanded, cfg, true, true, true),
        IslandMode::Hidden
    );

    let mut auto_idle = cfg;
    auto_idle.auto_hide_when_idle = true;
    assert_eq!(
        effective_mode(IslandMode::Slim, auto_idle, false, false, false),
        IslandMode::Hidden
    );
    // Khi có HUD dù đang Tự ẩn khi rảnh, viên thuốc vẫn bung gọn lên Compact.
    assert_eq!(
        effective_mode(IslandMode::Slim, auto_idle, false, false, true),
        IslandMode::Compact
    );
    // Nhưng không nhảy đè nếu đang ở Expanded, Settings hoặc AudioDevices.
    assert_eq!(
        effective_mode(IslandMode::Expanded, auto_idle, false, true, true),
        IslandMode::Expanded
    );
    assert_eq!(
        effective_mode(IslandMode::Settings, auto_idle, false, false, true),
        IslandMode::Settings
    );
    assert_eq!(
        effective_mode(IslandMode::AudioDevices, auto_idle, false, false, true),
        IslandMode::AudioDevices
    );
    assert_eq!(
        effective_mode(IslandMode::Notice, auto_idle, false, false, true),
        IslandMode::Notice
    );
}

#[test]
fn notice_hitboxes_stay_inside_pill_and_do_not_overlap() {
    let pill = IslandAnimator::new(IslandMode::Notice).current_pill();
    let mut hitboxes = Vec::with_capacity(2);
    fill_notice_hitboxes(pill, false, &mut hitboxes);
    assert_eq!(hitboxes.len(), 1);
    assert_eq!(hitboxes[0].action, UiAction::NoticePrimary);

    fill_notice_hitboxes(pill, true, &mut hitboxes);
    assert_eq!(hitboxes.len(), 2);
    assert_eq!(hitboxes[0].action, UiAction::NoticeSecondary);
    assert_eq!(hitboxes[1].action, UiAction::NoticePrimary);
    for hb in &hitboxes {
        assert!(hb.rect.left >= pill.left);
        assert!(hb.rect.right <= pill.left + pill.width);
        assert!(hb.rect.top >= pill.top + 86.0);
        assert!(hb.rect.bottom <= pill.top + pill.height);
    }
    assert!(hitboxes[0].rect.right < hitboxes[1].rect.left);
}

#[test]
fn audio_devices_hitboxes_stay_inside_pill_and_do_not_overlap() {
    let anim = IslandAnimator::new(IslandMode::AudioDevices);
    let pill = anim.current_pill();
    let mut hitboxes = Vec::with_capacity(6);
    fill_audio_devices_hitboxes(pill, 7, &mut hitboxes);
    assert_eq!(hitboxes.len(), 1 + MAX_AUDIO_DEVICES_VISIBLE);
    assert_eq!(hitboxes[0].action, UiAction::BackToMedia);

    for hb in &hitboxes {
        assert!(hb.rect.left >= pill.left);
        assert!(hb.rect.right <= pill.left + pill.width);
        assert!(hb.rect.top >= pill.top);
        assert!(hb.rect.bottom <= pill.top + pill.height);
    }
    for i in 0..hitboxes.len() {
        for j in (i + 1)..hitboxes.len() {
            let a = hitboxes[i].rect;
            let b = hitboxes[j].rect;
            let overlap_x = a.left < b.right && a.right > b.left;
            let overlap_y = a.top < b.bottom && a.bottom > b.top;
            assert!(!(overlap_x && overlap_y));
        }
    }
}

#[test]
fn audio_devices_height_scales_with_device_count() {
    assert_eq!(audio_devices_height_for_count(0), 107.0);
    assert_eq!(audio_devices_height_for_count(1), 107.0);
    assert_eq!(audio_devices_height_for_count(3), 193.0);
    assert_eq!(audio_devices_height_for_count(5), 279.0);
    assert_eq!(audio_devices_height_for_count(7), 293.0);
}

#[test]
fn volume_hud_hitboxes_stay_inside_compact_pill_and_do_not_overlap() {
    let pill = IslandAnimator::new(IslandMode::Compact).current_pill();
    let mut hitboxes = Vec::with_capacity(2);
    fill_volume_hud_hitboxes(pill, &mut hitboxes);
    assert_eq!(hitboxes.len(), 2);
    let mute = hitboxes[0].rect;
    let slider = hitboxes[1].rect;
    assert!(mute.left >= pill.left && mute.right <= pill.left + pill.width);
    assert!(slider.left >= mute.right && slider.right <= pill.left + pill.width);
    let grab = volume_seek_hit_bounds(slider);
    assert!(grab.contains(slider.left - 4.0, slider.top + 8.0));
    assert!(pill_contains_dip(
        pill,
        pill.left + pill.width * 0.5,
        pill.top + 12.0,
        1.0
    ));
}

#[test]
fn slim_notch_and_compact_preserve_top_edge_hit_area() {
    let slim_pill = IslandAnimator::new(IslandMode::Slim).current_pill();
    assert_eq!(slim_pill.top, 0.0);
    assert_eq!(slim_pill.width, 152.0);
    assert_eq!(slim_pill.height, 16.0);
    let (slim_rgn, _) = compute_pill_region_px(slim_pill, 1.0);
    assert_eq!(slim_rgn.top, 0);

    let compact_pill = IslandAnimator::new(IslandMode::Compact).current_pill();
    assert_eq!(compact_pill.top, 2.0);
    let (compact_rgn, _) = compute_pill_region_px(compact_pill, 1.0);
    assert_eq!(compact_rgn.top, 0);
}

#[test]
fn file_shelf_stays_right_of_island_and_gates_delete_all_on_selection() {
    let expanded = IslandAnimator::new(IslandMode::Expanded).current_pill();
    let dock = shelf_pill_rect(expanded, IslandMode::Expanded, false, 0)
        .expect("Shelf dock must exist beside Expanded island");
    assert!(dock.left >= expanded.left + expanded.width + 8.0);
    assert!(dock.left + dock.width <= MAX_SURFACE_WIDTH_DIP);

    let panel = shelf_pill_rect(expanded, IslandMode::Expanded, true, 3)
        .expect("Shelf panel must fit inside FHD surface beside Expanded island");
    assert!(panel.left >= expanded.left + expanded.width + 8.0);
    assert!(panel.left + panel.width <= MAX_SURFACE_WIDTH_DIP);
    assert!(panel.top + panel.height <= MAX_SURFACE_HEIGHT_DIP);

    let mut hitboxes = Vec::new();
    fill_shelf_hitboxes(panel, true, 3, 0, 0, &mut hitboxes);
    assert!(
        !hitboxes.iter().any(|hb| hb.action == UiAction::ShelfDeleteAll),
        "Delete All hitbox must be hidden when 0 tickboxes are selected"
    );
    assert!(hitboxes.iter().any(|hb| hb.action == UiAction::ShelfToggleSelect(0)));
    assert!(hitboxes.iter().any(|hb| hb.action == UiAction::ShelfDragItem(0)));

    fill_shelf_hitboxes(panel, true, 3, 0, 2, &mut hitboxes);
    assert!(
        hitboxes.iter().any(|hb| hb.action == UiAction::ShelfDeleteAll),
        "Delete All hitbox must appear once >= 1 tickboxes are selected"
    );

    // Không chồng lấn cặp hitbox nào trong Shelf panel.
    for (i, a) in hitboxes.iter().enumerate() {
        assert!(a.rect.left >= panel.left && a.rect.right <= panel.left + panel.width);
        assert!(a.rect.top >= panel.top && a.rect.bottom <= panel.top + panel.height);
        for b in hitboxes.iter().skip(i + 1) {
            let overlap_x = a.rect.left < b.rect.right && a.rect.right > b.rect.left;
            let overlap_y = a.rect.top < b.rect.bottom && a.rect.bottom > b.rect.top;
            assert!(!(overlap_x && overlap_y));
        }
    }
}

#[test]
fn compact_media_and_anti_expanded_hitboxes_are_valid() {
    let compact = IslandAnimator::new(IslandMode::Compact).current_pill();
    let mut c_boxes = Vec::new();
    fill_compact_media_hitboxes(compact, &mut c_boxes);
    assert_eq!(c_boxes.len(), 1);
    assert_eq!(c_boxes[0].action, UiAction::OpenAudioDevices);
    assert!(
        c_boxes[0].rect.left >= compact.left && c_boxes[0].rect.right <= compact.left + compact.width
    );

    let anti = IslandAnimator::new(IslandMode::AntiExpanded).current_pill();
    let mut a_boxes = Vec::new();
    fill_interactive_hitboxes(anti, IslandMode::AntiExpanded, &mut a_boxes);
    assert_eq!(a_boxes.len(), 3);
    for (i, a) in a_boxes.iter().enumerate() {
        assert!(a.rect.left >= anti.left && a.rect.right <= anti.left + anti.width);
        assert!(a.rect.top >= anti.top && a.rect.bottom <= anti.top + anti.height);
        for b in a_boxes.iter().skip(i + 1) {
            let overlap_x = a.rect.left < b.rect.right && a.rect.right > b.rect.left;
            let overlap_y = a.rect.top < b.rect.bottom && a.rect.bottom > b.rect.top;
            assert!(!(overlap_x && overlap_y));
        }
    }
}
