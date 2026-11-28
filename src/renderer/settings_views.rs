use super::*;

impl IslandRenderer {
    pub(super) unsafe fn draw_settings(&mut self, canvas: DrawCanvas<'_>, input: &RenderFrameInput<'_>) {
        let p = virtual_pill_anchored(input.pill, 424.0, 436.0, false);
        let text_primary = self.brushes.text_primary.clone();
        let text_secondary = self.brushes.text_secondary.clone();
        let sys_accent = self.brushes.sys_accent.clone();
        let power_ok = self.brushes.power_ok.clone();
        let version_label = format!("v{APP_VERSION}");

        unsafe {
            self.draw_text(
                canvas,
                "Bảng điều khiển nhanh",
                TextFormatSlot::Title,
                &text_primary,
                UiRectDip {
                    left: p.left + 18.0,
                    top: p.top + 10.0,
                    right: p.left + p.width - 98.0,
                    bottom: p.top + 34.0,
                },
            );
        }

        let sec_tops = settings_section_tops(p);
        for (sec_top, title) in sec_tops
            .into_iter()
            .zip(["Hiển thị", "HUD & Âm thanh", "Hệ thống"])
        {
            unsafe {
                self.draw_text(
                    canvas,
                    title,
                    TextFormatSlot::CompactSub,
                    &text_secondary,
                    UiRectDip {
                        left: p.left + 18.0,
                        top: sec_top,
                        right: p.left + p.width - 18.0,
                        bottom: sec_top + 15.0,
                    },
                );
            }
        }

        fill_interactive_hitboxes(p, IslandMode::Settings, &mut self.hitbox_scratch);
        let count = self.hitbox_scratch.len();
        for idx in 0..count {
            let hb = self.hitbox_scratch[idx];
            let hovered = input.hovered_action == Some(hb.action);
            let is_danger = hb.action == UiAction::ExitApp;
            let has_pending_update =
                hb.action == UiAction::CheckUpdate && input.available_update_tag.is_some();
            let fill = if is_danger {
                if hovered {
                    &self.brushes.danger_hover
                } else {
                    &self.brushes.danger_bg
                }
            } else if hovered {
                &self.brushes.surface_hover
            } else if has_pending_update {
                &self.brushes.sys_accent_soft
            } else {
                &self.brushes.surface_card
            };

            let card = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: hb.rect.left,
                    top: hb.rect.top,
                    right: hb.rect.right,
                    bottom: hb.rect.bottom,
                },
                radiusX: 10.0,
                radiusY: 10.0,
            };
            unsafe {
                canvas.ctx.FillRoundedRectangle(&card, fill);
                if is_danger {
                    canvas.ctx.DrawRoundedRectangle(
                        &card,
                        &self.brushes.danger_border,
                        1.0,
                        None,
                    );
                } else if has_pending_update {
                    canvas
                        .ctx
                        .DrawRoundedRectangle(&card, &self.brushes.sys_accent, 0.95, None);
                } else {
                    canvas.ctx.DrawRoundedRectangle(
                        &card,
                        &self.brushes.card_border,
                        0.85,
                        None,
                    );
                }
            }

            match hb.action {
                UiAction::BackToMedia => unsafe {
                    self.draw_text(
                        canvas,
                        "Quay lại",
                        TextFormatSlot::ButtonCenter,
                        &text_primary,
                        hb.rect,
                    );
                },
                UiAction::ToggleNotchCollapse => unsafe {
                    self.draw_toggle_tile(
                        canvas,
                        hb.rect,
                        "Gọn mép 4s",
                        input.config.notch_collapse_after_4s,
                        &text_primary,
                        &sys_accent,
                    );
                },
                UiAction::ToggleHudVolume => unsafe {
                    self.draw_toggle_tile(
                        canvas,
                        hb.rect,
                        "HUD Âm lượng",
                        input.config.hud_volume_enabled,
                        &text_primary,
                        &sys_accent,
                    );
                },
                UiAction::ToggleHudWheel => unsafe {
                    self.draw_toggle_tile(
                        canvas,
                        hb.rect,
                        "Cuộn chỉnh loa",
                        input.config.hud_wheel_enabled,
                        &text_primary,
                        &sys_accent,
                    );
                },
                UiAction::ToggleHudPower => unsafe {
                    self.draw_toggle_tile(
                        canvas,
                        hb.rect,
                        "HUD Pin / Sạc",
                        input.config.hud_power_enabled,
                        &text_primary,
                        &sys_accent,
                    );
                },
                UiAction::ToggleHudDevice => unsafe {
                    self.draw_toggle_tile(
                        canvas,
                        hb.rect,
                        "HUD Thiết bị",
                        input.config.hud_device_enabled,
                        &text_primary,
                        &sys_accent,
                    );
                },
                UiAction::OpenAudioDevices => unsafe {
                    self.draw_text(
                        canvas,
                        "Thiết bị phát",
                        TextFormatSlot::Button,
                        &text_primary,
                        UiRectDip {
                            left: hb.rect.left + 10.0,
                            top: hb.rect.top,
                            right: hb.rect.right - 46.0,
                            bottom: hb.rect.bottom,
                        },
                    );
                    self.draw_text(
                        canvas,
                        "Chọn",
                        TextFormatSlot::SmallRight,
                        &sys_accent,
                        UiRectDip {
                            left: hb.rect.right - 48.0,
                            top: hb.rect.top,
                            right: hb.rect.right - 10.0,
                            bottom: hb.rect.bottom,
                        },
                    );
                },
                UiAction::ToggleAutoHideIdle => unsafe {
                    self.draw_toggle_tile(
                        canvas,
                        hb.rect,
                        "Tự ẩn khi rảnh",
                        input.config.auto_hide_when_idle,
                        &text_primary,
                        &sys_accent,
                    );
                },
                UiAction::ToggleAutoHideFullscreen => unsafe {
                    self.draw_toggle_tile(
                        canvas,
                        hb.rect,
                        "Ẩn toàn màn hình",
                        input.config.auto_hide_fullscreen,
                        &text_primary,
                        &sys_accent,
                    );
                },
                UiAction::ToggleVisualizer => unsafe {
                    self.draw_toggle_tile(
                        canvas,
                        hb.rect,
                        "Sóng nhạc động",
                        input.config.visualizer_enabled,
                        &text_primary,
                        &sys_accent,
                    );
                },
                UiAction::ToggleAutostart => unsafe {
                    self.draw_toggle_tile(
                        canvas,
                        hb.rect,
                        "Mở cùng Windows",
                        input.config.autostart,
                        &text_primary,
                        &sys_accent,
                    );
                },
                UiAction::CheckUpdate => {
                    let (label, right_tag, right_brush, dot_brush) =
                        match input.available_update_tag {
                            Some(tag) => ("Có bản mới", tag, &sys_accent, &sys_accent),
                            None => (
                                "Kiểm tra cập nhật",
                                version_label.as_str(),
                                &text_secondary,
                                &power_ok,
                            ),
                        };
                    let tag_right = hb.rect.right - 10.0;
                    let tag_box_w = 62.0;
                    let tag_w = unsafe {
                        self.measure_text_width(
                            right_tag,
                            TextFormatSlot::SmallRight,
                            tag_box_w,
                            hb.rect.height(),
                        )
                    }
                    .unwrap_or(36.0);
                    let dot_cx = Self::check_update_live_dot_cx(hb.rect.right, tag_w);
                    let dot_cy = hb.rect.top + hb.rect.height() * 0.5;
                    let halo_r = 5.0;
                    let core_r = 2.6;
                    let label_right = (dot_cx - halo_r - 4.0).max(hb.rect.left + 90.0);
                    let halo = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: dot_cx - halo_r,
                            top: dot_cy - halo_r,
                            right: dot_cx + halo_r,
                            bottom: dot_cy + halo_r,
                        },
                        radiusX: halo_r,
                        radiusY: halo_r,
                    };
                    let core = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: dot_cx - core_r,
                            top: dot_cy - core_r,
                            right: dot_cx + core_r,
                            bottom: dot_cy + core_r,
                        },
                        radiusX: core_r,
                        radiusY: core_r,
                    };
                    unsafe {
                        self.draw_text(
                            canvas,
                            label,
                            TextFormatSlot::Button,
                            &text_primary,
                            UiRectDip {
                                left: hb.rect.left + 10.0,
                                top: hb.rect.top,
                                right: label_right,
                                bottom: hb.rect.bottom,
                            },
                        );
                        dot_brush.SetOpacity(0.25);
                        canvas.ctx.FillRoundedRectangle(&halo, dot_brush);
                        dot_brush.SetOpacity(1.0);
                        canvas.ctx.FillRoundedRectangle(&core, dot_brush);
                        self.draw_text(
                            canvas,
                            right_tag,
                            TextFormatSlot::SmallRight,
                            right_brush,
                            UiRectDip {
                                left: tag_right - tag_box_w,
                                top: hb.rect.top,
                                right: tag_right,
                                bottom: hb.rect.bottom,
                            },
                        );
                    }
                }
                UiAction::CycleMonitor => {
                    let label = format!("Màn hình: {}", input.monitor_label);
                    unsafe {
                        self.draw_text(
                            canvas,
                            &label,
                            TextFormatSlot::Button,
                            &text_primary,
                            UiRectDip {
                                left: hb.rect.left + 12.0,
                                top: hb.rect.top,
                                right: hb.rect.right - 72.0,
                                bottom: hb.rect.bottom,
                            },
                        );
                        self.draw_text(
                            canvas,
                            "Đổi",
                            TextFormatSlot::SmallRight,
                            &sys_accent,
                            UiRectDip {
                                left: hb.rect.right - 64.0,
                                top: hb.rect.top,
                                right: hb.rect.right - 10.0,
                                bottom: hb.rect.bottom,
                            },
                        );
                    }
                }
                UiAction::ToggleAntiMonitor => unsafe {
                    self.draw_toggle_tile(
                        canvas,
                        hb.rect,
                        "Theo dõi Anti",
                        input.config.anti_monitor_enabled,
                        &text_primary,
                        &sys_accent,
                    );
                },
                UiAction::ToggleAntiHooks => {
                    let (status_text, status_brush) = if input.anti_hooks_installed {
                        ("Đã cài", &power_ok)
                    } else {
                        ("Cài đặt", &sys_accent)
                    };
                    unsafe {
                        self.draw_text(
                            canvas,
                            "Hook Anti",
                            TextFormatSlot::Button,
                            &text_primary,
                            UiRectDip {
                                left: hb.rect.left + 10.0,
                                top: hb.rect.top,
                                right: hb.rect.right - 64.0,
                                bottom: hb.rect.bottom,
                            },
                        );
                        self.draw_text(
                            canvas,
                            status_text,
                            TextFormatSlot::SmallRight,
                            status_brush,
                            UiRectDip {
                                left: hb.rect.right - 64.0,
                                top: hb.rect.top,
                                right: hb.rect.right - 10.0,
                                bottom: hb.rect.bottom,
                            },
                        );
                    }
                }
                UiAction::HideIsland => unsafe {
                    self.draw_text(
                        canvas,
                        "Ẩn tạm",
                        TextFormatSlot::ButtonCenter,
                        &text_primary,
                        hb.rect,
                    );
                },
                UiAction::ExitApp => unsafe {
                    self.draw_text(
                        canvas,
                        "Thoát ứng dụng",
                        TextFormatSlot::ButtonCenter,
                        &text_primary,
                        hb.rect,
                    );
                },
                _ => {}
            }
        }
    }

    pub(super) unsafe fn draw_notice(&mut self, canvas: DrawCanvas<'_>, input: &RenderFrameInput<'_>) {
        let Some(notice) = input.notice else {
            return;
        };
        let p = virtual_pill_anchored(input.pill, 416.0, 136.0, false);
        let text_primary = self.brushes.text_primary.clone();
        let text_secondary = self.brushes.text_secondary.clone();
        let accent_brush = match notice.kind {
            NoticeKind::UpToDate => self.brushes.power_ok.clone(),
            NoticeKind::Error => self.brushes.danger_border.clone(),
            NoticeKind::Checking | NoticeKind::UpdateAvailable | NoticeKind::Downloading => {
                self.brushes.accent.clone()
            }
        };
        let badge_fill = if notice.kind == NoticeKind::Error {
            self.brushes.danger_bg.clone()
        } else {
            self.brushes.surface_card.clone()
        };

        let cx = p.left + 35.0;
        let cy = p.top + 42.0;
        let badge_r = 17.5;
        let badge = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - badge_r,
                top: cy - badge_r,
                right: cx + badge_r,
                bottom: cy + badge_r,
            },
            radiusX: badge_r,
            radiusY: badge_r,
        };
        unsafe {
            canvas.ctx.FillRoundedRectangle(&badge, &badge_fill);
            canvas
                .ctx
                .DrawRoundedRectangle(&badge, &accent_brush, 1.25, None);
            self.draw_notice_glyph(canvas, cx, cy, notice.kind, &accent_brush);
            self.draw_text(
                canvas,
                notice.title,
                TextFormatSlot::Title,
                &text_primary,
                UiRectDip {
                    left: p.left + 64.0,
                    top: p.top + 14.0,
                    right: p.left + p.width - 18.0,
                    bottom: p.top + 36.0,
                },
            );
            self.draw_text(
                canvas,
                notice.message,
                TextFormatSlot::BodyLeft,
                &text_secondary,
                UiRectDip {
                    left: p.left + 64.0,
                    top: p.top + 38.0,
                    right: p.left + p.width - 18.0,
                    bottom: p.top + 88.0,
                },
            );
        }

        fill_notice_hitboxes(p, notice.secondary_label.is_some(), &mut self.hitbox_scratch);
        let count = self.hitbox_scratch.len();
        for idx in 0..count {
            let hb = self.hitbox_scratch[idx];
            let hovered = input.hovered_action == Some(hb.action);
            let card = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: hb.rect.left,
                    top: hb.rect.top,
                    right: hb.rect.right,
                    bottom: hb.rect.bottom,
                },
                radiusX: 11.0,
                radiusY: 11.0,
            };
            match hb.action {
                UiAction::NoticePrimary => {
                    let is_err = notice.kind == NoticeKind::Error;
                    let fill = if is_err {
                        if hovered {
                            &self.brushes.danger_hover
                        } else {
                            &self.brushes.danger_bg
                        }
                    } else if hovered {
                        &self.brushes.surface_hover
                    } else {
                        &self.brushes.accent_soft
                    };
                    let border = if is_err {
                        &self.brushes.danger_border
                    } else {
                        &self.brushes.accent
                    };
                    unsafe {
                        canvas.ctx.FillRoundedRectangle(&card, fill);
                        canvas.ctx.DrawRoundedRectangle(&card, border, 1.0, None);
                        self.draw_text(
                            canvas,
                            notice.primary_label,
                            TextFormatSlot::ButtonCenter,
                            &text_primary,
                            hb.rect,
                        );
                    }
                }
                UiAction::NoticeSecondary => {
                    let label = notice.secondary_label.unwrap_or("Đóng");
                    let fill = if hovered {
                        &self.brushes.surface_hover
                    } else {
                        &self.brushes.surface_card
                    };
                    unsafe {
                        canvas.ctx.FillRoundedRectangle(&card, fill);
                        canvas.ctx.DrawRoundedRectangle(
                            &card,
                            &self.brushes.card_border,
                            0.85,
                            None,
                        );
                        self.draw_text(
                            canvas,
                            label,
                            TextFormatSlot::ButtonCenter,
                            &text_secondary,
                            hb.rect,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    pub(super) unsafe fn draw_notice_glyph(
        &self,
        canvas: DrawCanvas<'_>,
        cx: f32,
        cy: f32,
        kind: NoticeKind,
        brush: &ID2D1SolidColorBrush,
    ) {
        let stroke = Some(&self.icons.round_stroke);
        unsafe {
            match kind {
                NoticeKind::UpToDate => {
                    canvas.ctx.DrawLine(
                        Vector2 { X: cx - 5.5, Y: cy + 0.4 },
                        Vector2 { X: cx - 1.5, Y: cy + 4.4 },
                        brush,
                        2.1,
                        stroke,
                    );
                    canvas.ctx.DrawLine(
                        Vector2 { X: cx - 1.5, Y: cy + 4.4 },
                        Vector2 { X: cx + 6.0, Y: cy - 4.4 },
                        brush,
                        2.1,
                        stroke,
                    );
                }
                NoticeKind::UpdateAvailable | NoticeKind::Downloading => {
                    canvas.ctx.DrawLine(
                        Vector2 { X: cx, Y: cy - 6.2 },
                        Vector2 { X: cx, Y: cy + 3.4 },
                        brush,
                        2.0,
                        stroke,
                    );
                    canvas.ctx.DrawLine(
                        Vector2 { X: cx - 4.4, Y: cy - 0.6 },
                        Vector2 { X: cx, Y: cy + 3.8 },
                        brush,
                        2.0,
                        stroke,
                    );
                    canvas.ctx.DrawLine(
                        Vector2 { X: cx + 4.4, Y: cy - 0.6 },
                        Vector2 { X: cx, Y: cy + 3.8 },
                        brush,
                        2.0,
                        stroke,
                    );
                    canvas.ctx.DrawLine(
                        Vector2 { X: cx - 5.5, Y: cy + 6.8 },
                        Vector2 { X: cx + 5.5, Y: cy + 6.8 },
                        brush,
                        1.8,
                        stroke,
                    );
                }
                NoticeKind::Checking => {
                    let dot = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: cx - 1.3,
                            top: cy - 6.2,
                            right: cx + 1.3,
                            bottom: cy - 3.6,
                        },
                        radiusX: 1.3,
                        radiusY: 1.3,
                    };
                    canvas.ctx.FillRoundedRectangle(&dot, brush);
                    canvas.ctx.DrawLine(
                        Vector2 { X: cx, Y: cy - 1.2 },
                        Vector2 { X: cx, Y: cy + 6.0 },
                        brush,
                        2.1,
                        stroke,
                    );
                }
                NoticeKind::Error => {
                    canvas.ctx.DrawLine(
                        Vector2 { X: cx, Y: cy - 6.0 },
                        Vector2 { X: cx, Y: cy + 1.8 },
                        brush,
                        2.2,
                        stroke,
                    );
                    let dot = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: cx - 1.4,
                            top: cy + 4.4,
                            right: cx + 1.4,
                            bottom: cy + 7.2,
                        },
                        radiusX: 1.4,
                        radiusY: 1.4,
                    };
                    canvas.ctx.FillRoundedRectangle(&dot, brush);
                }
            }
        }
    }

    pub(super) unsafe fn draw_toggle_tile(
        &mut self,
        canvas: DrawCanvas<'_>,
        rect: UiRectDip,
        label: &str,
        enabled: bool,
        text_brush: &ID2D1SolidColorBrush,
        track_on_brush: &ID2D1SolidColorBrush,
    ) {
        let text_rect = UiRectDip {
            left: rect.left + 10.0,
            top: rect.top,
            right: rect.right - 44.0,
            bottom: rect.bottom,
        };
        unsafe {
            self.draw_text(canvas, label, TextFormatSlot::Button, text_brush, text_rect);
        }

        let sw_w = 30.0;
        let sw_h = 16.0;
        let sw_x = rect.right - sw_w - 10.0;
        let sw_y = rect.top + (rect.height() - sw_h) * 0.5;
        let track = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: sw_x,
                top: sw_y,
                right: sw_x + sw_w,
                bottom: sw_y + sw_h,
            },
            radiusX: sw_h * 0.5,
            radiusY: sw_h * 0.5,
        };
        let track_brush = if enabled {
            track_on_brush
        } else {
            &self.brushes.switch_off
        };
        unsafe {
            canvas.ctx.FillRoundedRectangle(&track, track_brush);
        }

        let knob_size = 12.0;
        let knob_x = if enabled {
            sw_x + sw_w - knob_size - 2.0
        } else {
            sw_x + 2.0
        };
        let knob_y = sw_y + 2.0;
        let knob = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: knob_x,
                top: knob_y,
                right: knob_x + knob_size,
                bottom: knob_y + knob_size,
            },
            radiusX: knob_size * 0.5,
            radiusY: knob_size * 0.5,
        };
        unsafe {
            canvas.ctx.FillRoundedRectangle(&knob, &self.brushes.knob);
        }
    }

    pub(super) fn check_update_live_dot_cx(card_right: f32, tag_w: f32) -> f32 {
        (card_right - 10.0) - tag_w.clamp(24.0, 56.0) - 9.0
    }
}
