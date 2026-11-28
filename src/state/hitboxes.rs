use super::{IslandMode, MAX_AUDIO_DEVICES_VISIBLE, PillRectDip, UiAction, UiHitbox, UiRectDip};

/// Bố cục nút bấm dùng chung giữa lớp vẽ (`renderer`) và xử lý chuột (`window`).
pub fn fill_interactive_hitboxes(pill: PillRectDip, mode: IslandMode, out: &mut Vec<UiHitbox>) {
    out.clear();
    match mode {
        IslandMode::Expanded => fill_expanded_hitboxes(pill, out),
        IslandMode::Settings => fill_settings_hitboxes(pill, out),
        IslandMode::AudioDevices => fill_audio_devices_hitboxes(pill, 0, out),
        IslandMode::Notice => fill_notice_hitboxes(pill, false, out),
        _ => {}
    }
}

pub fn fill_notice_hitboxes(pill: PillRectDip, has_secondary: bool, out: &mut Vec<UiHitbox>) {
    out.clear();
    let pad_x = 16.0;
    let right_edge = pill.left + pill.width - pad_x;
    let btn_bottom = pill.top + pill.height - 14.0;
    let btn_top = btn_bottom - 32.0;
    let primary_w = if has_secondary { 126.0 } else { 114.0 };
    let primary_left = right_edge - primary_w;

    if has_secondary {
        let gap = 8.0;
        let sec_w = 98.0;
        let sec_right = primary_left - gap;
        out.push(UiHitbox {
            rect: UiRectDip {
                left: sec_right - sec_w,
                top: btn_top,
                right: sec_right,
                bottom: btn_bottom,
            },
            action: UiAction::NoticeSecondary,
        });
    }

    out.push(UiHitbox {
        rect: UiRectDip {
            left: primary_left,
            top: btn_top,
            right: right_edge,
            bottom: btn_bottom,
        },
        action: UiAction::NoticePrimary,
    });
}

pub fn fill_audio_devices_hitboxes(
    pill: PillRectDip,
    visible_count: usize,
    out: &mut Vec<UiHitbox>,
) {
    out.clear();
    let pad_x = 16.0;
    let inner_left = pill.left + pad_x;
    let inner_right = pill.left + pill.width - pad_x;

    let back_btn = UiRectDip {
        left: inner_right - 74.0,
        top: pill.top + 12.0,
        right: inner_right,
        bottom: pill.top + 36.0,
    };
    out.push(UiHitbox {
        rect: back_btn,
        action: UiAction::BackToMedia,
    });

    let count = visible_count.min(MAX_AUDIO_DEVICES_VISIBLE);
    let list_top = pill.top + 56.0;
    let row_h = 37.0;
    let gap = 6.0;
    for idx in 0..count {
        let top = list_top + idx as f32 * (row_h + gap);
        out.push(UiHitbox {
            rect: UiRectDip {
                left: inner_left,
                top,
                right: inner_right,
                bottom: top + row_h,
            },
            action: UiAction::SelectAudioDevice(idx),
        });
    }
}

pub fn fill_volume_hud_hitboxes(pill: PillRectDip, out: &mut Vec<UiHitbox>) {
    out.clear();
    let mute_btn = UiRectDip {
        left: pill.left + 6.0,
        top: pill.top + 3.0,
        right: pill.left + 36.0,
        bottom: pill.top + pill.height - 3.0,
    };
    let slider = UiRectDip {
        left: pill.left + 44.0,
        top: pill.top + 2.0,
        right: (pill.left + pill.width - 50.0).max(pill.left + 84.0),
        bottom: pill.top + pill.height - 2.0,
    };
    out.extend_from_slice(&[
        UiHitbox {
            rect: mute_btn,
            action: UiAction::ToggleVolumeMute,
        },
        UiHitbox {
            rect: slider,
            action: UiAction::SeekVolume,
        },
    ]);
}

pub fn volume_seek_hit_bounds(track: UiRectDip) -> UiRectDip {
    UiRectDip {
        left: track.left - 8.0,
        top: track.top,
        right: track.right + 8.0,
        bottom: track.bottom,
    }
}

pub fn pill_contains_dip(pill: PillRectDip, x_dip: f32, y_dip: f32, pad: f32) -> bool {
    x_dip >= pill.left - pad
        && x_dip <= pill.left + pill.width + pad
        && y_dip >= pill.top - pad
        && y_dip <= pill.top + pill.height + pad
}

fn fill_expanded_hitboxes(pill: PillRectDip, out: &mut Vec<UiHitbox>) {
    let timeline_track = UiRectDip {
        left: pill.left + 60.0,
        top: pill.top + 81.0,
        right: pill.left + pill.width - 60.0,
        bottom: pill.top + 110.0,
    };

    let controls_bottom = pill.top + pill.height - 10.0;
    let center_x = pill.left + pill.width * 0.5;

    let prev_btn = UiRectDip {
        left: center_x - 60.0,
        top: controls_bottom - 36.0,
        right: center_x - 26.0,
        bottom: controls_bottom - 2.0,
    };
    let play_btn = UiRectDip {
        left: center_x - 19.0,
        top: controls_bottom - 38.0,
        right: center_x + 19.0,
        bottom: controls_bottom,
    };
    let next_btn = UiRectDip {
        left: center_x + 26.0,
        top: controls_bottom - 36.0,
        right: center_x + 60.0,
        bottom: controls_bottom - 2.0,
    };
    let audio_btn = UiRectDip {
        left: pill.left + 16.0,
        top: controls_bottom - 34.0,
        right: center_x - 68.0,
        bottom: controls_bottom - 2.0,
    };
    let settings_btn = UiRectDip {
        left: pill.left + pill.width - 110.0,
        top: controls_bottom - 34.0,
        right: pill.left + pill.width - 16.0,
        bottom: controls_bottom - 2.0,
    };

    out.extend_from_slice(&[
        UiHitbox {
            rect: timeline_track,
            action: UiAction::SeekTimeline,
        },
        UiHitbox {
            rect: prev_btn,
            action: UiAction::MediaPrev,
        },
        UiHitbox {
            rect: play_btn,
            action: UiAction::MediaPlayPause,
        },
        UiHitbox {
            rect: next_btn,
            action: UiAction::MediaNext,
        },
        UiHitbox {
            rect: audio_btn,
            action: UiAction::OpenAudioDevices,
        },
        UiHitbox {
            rect: settings_btn,
            action: UiAction::OpenSettings,
        },
    ]);
}

/// Mở rộng biên nhận nhấn chuột hai đầu thanh timeline để dễ bắt điểm 0% và 100%.
pub fn timeline_seek_hit_bounds(track: UiRectDip) -> UiRectDip {
    UiRectDip {
        left: track.left - 14.0,
        top: track.top,
        right: track.right + 14.0,
        bottom: track.bottom,
    }
}

pub fn timeline_ratio_for_x(track: UiRectDip, x_dip: f32) -> f32 {
    let width = track.width().max(1.0);
    ((x_dip - track.left) / width).clamp(0.0, 1.0)
}

fn is_compact_click(down_pos: (f32, f32), up_pos: (f32, f32)) -> bool {
    let dx = up_pos.0 - down_pos.0;
    let dy = up_pos.1 - down_pos.1;
    (dx * dx + dy * dy).sqrt() <= 8.0
}

/// Chỉ cho phép thu gọn khi nhấn gọn ở vùng tiêu đề phía trên, tránh trượt tay vùng timeline/nút bấm.
pub fn should_collapse_expanded_on_blank_click(
    pill: PillRectDip,
    down_pos: (f32, f32),
    up_pos: (f32, f32),
) -> bool {
    if !is_compact_click(down_pos, up_pos) {
        return false;
    }
    let header_limit_y = pill.top + 80.0;
    down_pos.1 < header_limit_y && up_pos.1 < header_limit_y
}

/// Cho phép bấm vào thanh tiêu đề phía trên của bảng Cài đặt / Thiết bị phát để quay lại nhanh.
pub fn should_close_subview_on_header_click(
    pill: PillRectDip,
    down_pos: (f32, f32),
    up_pos: (f32, f32),
) -> bool {
    if !is_compact_click(down_pos, up_pos) {
        return false;
    }
    let header_limit_y = pill.top + 38.0;
    pill_contains_dip(pill, down_pos.0, down_pos.1, 0.0)
        && pill_contains_dip(pill, up_pos.0, up_pos.1, 0.0)
        && down_pos.1 < header_limit_y
        && up_pos.1 < header_limit_y
}

/// Cho phép bấm vào thanh tiêu đề phía trên của Khay tệp để thu gọn nhanh.
pub fn should_collapse_shelf_on_header_click(
    rect: PillRectDip,
    down_pos: (f32, f32),
    up_pos: (f32, f32),
) -> bool {
    if !is_compact_click(down_pos, up_pos) {
        return false;
    }
    let header_limit_y = rect.top + 34.0;
    shelf_contains_dip(rect, down_pos.0, down_pos.1, 0.0)
        && shelf_contains_dip(rect, up_pos.0, up_pos.1, 0.0)
        && down_pos.1 < header_limit_y
        && up_pos.1 < header_limit_y
}

pub fn settings_section_tops(pill: PillRectDip) -> [f32; 3] {
    [pill.top + 39.0, pill.top + 132.0, pill.top + 225.0]
}

fn fill_settings_hitboxes(pill: PillRectDip, out: &mut Vec<UiHitbox>) {
    let pad_x = 16.0;
    let gap = 6.0;
    let inner_left = pill.left + pad_x;
    let inner_right = pill.left + pill.width - pad_x;
    let col_w = ((inner_right - inner_left - gap) * 0.5).max(80.0);

    let back_btn = UiRectDip {
        left: inner_right - 74.0,
        top: pill.top + 10.0,
        right: inner_right,
        bottom: pill.top + 34.0,
    };

    let row_h = 32.0;
    let row1_top = pill.top + 55.0;
    let row2_top = row1_top + row_h + gap;
    let row3_top = pill.top + 148.0;
    let row4_top = row3_top + row_h + gap;
    let row5_top = pill.top + 241.0;
    let row6_top = row5_top + row_h + gap;
    let row7_top = row6_top + row_h + gap;
    let bottom_h = 32.0;
    let footer_top = row7_top + row_h + 7.0;

    out.extend_from_slice(&[
        UiHitbox {
            rect: back_btn,
            action: UiAction::BackToMedia,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left,
                top: row1_top,
                right: inner_left + col_w,
                bottom: row1_top + row_h,
            },
            action: UiAction::ToggleAutoHideIdle,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left + col_w + gap,
                top: row1_top,
                right: inner_right,
                bottom: row1_top + row_h,
            },
            action: UiAction::ToggleAutoHideFullscreen,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left,
                top: row2_top,
                right: inner_left + col_w,
                bottom: row2_top + row_h,
            },
            action: UiAction::ToggleNotchCollapse,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left + col_w + gap,
                top: row2_top,
                right: inner_right,
                bottom: row2_top + row_h,
            },
            action: UiAction::ToggleVisualizer,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left,
                top: row3_top,
                right: inner_left + col_w,
                bottom: row3_top + row_h,
            },
            action: UiAction::ToggleHudVolume,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left + col_w + gap,
                top: row3_top,
                right: inner_right,
                bottom: row3_top + row_h,
            },
            action: UiAction::ToggleHudWheel,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left,
                top: row4_top,
                right: inner_left + col_w,
                bottom: row4_top + row_h,
            },
            action: UiAction::ToggleHudPower,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left + col_w + gap,
                top: row4_top,
                right: inner_right,
                bottom: row4_top + row_h,
            },
            action: UiAction::OpenAudioDevices,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left,
                top: row5_top,
                right: inner_left + col_w,
                bottom: row5_top + row_h,
            },
            action: UiAction::ToggleAutostart,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left + col_w + gap,
                top: row5_top,
                right: inner_right,
                bottom: row5_top + row_h,
            },
            action: UiAction::CheckUpdate,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left,
                top: row6_top,
                right: inner_right,
                bottom: row6_top + row_h,
            },
            action: UiAction::CycleMonitor,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left,
                top: row7_top,
                right: inner_left + col_w,
                bottom: row7_top + row_h,
            },
            action: UiAction::ToggleAntiMonitor,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left + col_w + gap,
                top: row7_top,
                right: inner_right,
                bottom: row7_top + row_h,
            },
            action: UiAction::ToggleAntiHooks,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left,
                top: footer_top,
                right: inner_left + col_w,
                bottom: footer_top + bottom_h,
            },
            action: UiAction::HideIsland,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left + col_w + gap,
                top: footer_top,
                right: inner_right,
                bottom: footer_top + bottom_h,
            },
            action: UiAction::ExitApp,
        },
    ]);
}

pub const MAX_SHELF_VISIBLE_ROWS: usize = 5;

pub fn shelf_pill_rect(
    pill: PillRectDip,
    mode: IslandMode,
    shelf_open: bool,
    item_count: usize,
) -> Option<PillRectDip> {
    if !matches!(
        mode,
        IslandMode::Slim | IslandMode::Compact | IslandMode::Expanded
    ) {
        return None;
    }
    let gap = 10.0;
    let raw_left = pill.left + pill.width + gap;
    if !shelf_open {
        let dock_w = if item_count > 0 { 96.0 } else { 88.0 };
        let dock_h = if mode == IslandMode::Slim { 24.0 } else { 28.0 };
        let top = if mode == IslandMode::Slim {
            2.0
        } else {
            pill.top + (pill.height.min(40.0) - dock_h) * 0.5
        };
        let left = raw_left.min(super::MAX_SURFACE_WIDTH_DIP - dock_w - 6.0);
        return Some(PillRectDip {
            left,
            top,
            width: dock_w,
            height: dock_h,
            corner_radius: dock_h * 0.5,
        });
    }

    let panel_w = 264.0;
    let left = raw_left.min(super::MAX_SURFACE_WIDTH_DIP - panel_w - 6.0);
    let top = pill.top.max(6.0);
    let visible = item_count.min(MAX_SHELF_VISIBLE_ROWS);
    let body_h = if visible == 0 {
        70.0
    } else {
        visible as f32 * 32.0 + visible.saturating_sub(1) as f32 * 4.0
    };
    let select_bar_h = if visible > 0 { 26.0 } else { 0.0 };
    let footer_h = if visible > 0 { 36.0 } else { 0.0 };
    let panel_h = 34.0 + select_bar_h + body_h + footer_h + 10.0;
    Some(PillRectDip {
        left,
        top,
        width: panel_w,
        height: panel_h,
        corner_radius: 20.0,
    })
}

pub fn shelf_contains_dip(rect: PillRectDip, x_dip: f32, y_dip: f32, margin: f32) -> bool {
    x_dip >= rect.left - margin
        && x_dip <= rect.left + rect.width + margin
        && y_dip >= rect.top - margin
        && y_dip <= rect.top + rect.height + margin
}

pub fn fill_shelf_hitboxes(
    rect: PillRectDip,
    open: bool,
    items_len: usize,
    scroll: usize,
    selected_count: usize,
    out: &mut Vec<UiHitbox>,
) {
    out.clear();
    if !open {
        out.push(UiHitbox {
            rect: UiRectDip {
                left: rect.left,
                top: rect.top,
                right: rect.left + rect.width,
                bottom: rect.top + rect.height,
            },
            action: UiAction::ToggleShelf,
        });
        return;
    }

    let pad_x = 12.0;
    let inner_left = rect.left + pad_x;
    let inner_right = rect.left + rect.width - pad_x;

    out.push(UiHitbox {
        rect: UiRectDip {
            left: inner_right - 66.0,
            top: rect.top + 8.0,
            right: inner_right,
            bottom: rect.top + 30.0,
        },
        action: UiAction::ShelfClose,
    });

    if items_len == 0 {
        return;
    }

    let select_bar_top = rect.top + 34.0;
    out.push(UiHitbox {
        rect: UiRectDip {
            left: inner_left,
            top: select_bar_top,
            right: inner_left + 126.0,
            bottom: select_bar_top + 22.0,
        },
        action: UiAction::ShelfToggleSelectAll,
    });

    let list_top = select_bar_top + 26.0;
    let row_h = 32.0;
    let gap = 4.0;
    let visible = items_len.saturating_sub(scroll).min(MAX_SHELF_VISIBLE_ROWS);
    for vis_idx in 0..visible {
        let actual_idx = scroll + vis_idx;
        let top = list_top + vis_idx as f32 * (row_h + gap);
        let bottom = top + row_h;
        out.push(UiHitbox {
            rect: UiRectDip {
                left: inner_left,
                top,
                right: inner_left + 26.0,
                bottom,
            },
            action: UiAction::ShelfToggleSelect(actual_idx),
        });
        out.push(UiHitbox {
            rect: UiRectDip {
                left: inner_left + 28.0,
                top,
                right: inner_right,
                bottom,
            },
            action: UiAction::ShelfDragItem(actual_idx),
        });
    }

    // Chỉ tạo hitbox nút Delete All khi có ít nhất 1 tickbox đang được chọn.
    if selected_count > 0 {
        let footer_top = list_top + visible as f32 * row_h + visible.saturating_sub(1) as f32 * gap + 6.0;
        out.push(UiHitbox {
            rect: UiRectDip {
                left: inner_left,
                top: footer_top,
                right: inner_right,
                bottom: footer_top + 30.0,
            },
            action: UiAction::ShelfDeleteAll,
        });
    }
}
