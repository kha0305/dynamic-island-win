use super::*;

impl IslandRenderer {
    pub(super) unsafe fn draw_compact_hud(
        &mut self,
        canvas: DrawCanvas<'_>,
        input: &RenderFrameInput<'_>,
        hud: HudContent,
    ) {
        let (cw, ch, _, _) = target_dimensions_for_mode(IslandMode::Compact);
        let p = virtual_pill_anchored(input.pill, cw, ch, true);
        match hud {
            HudContent::Volume {
                level,
                muted,
                show_device_name,
            } => {
                let display_level = input.volume_scrub_ratio.unwrap_or(level).clamp(0.0, 1.0);
                let is_dragging = input.volume_scrub_ratio.is_some();
                fill_volume_hud_hitboxes(p, &mut self.hitbox_scratch);
                let mute_rect = self
                    .hitbox_scratch
                    .iter()
                    .find(|hb| hb.action == UiAction::ToggleVolumeMute)
                    .map(|hb| hb.rect)
                    .unwrap_or(UiRectDip {
                        left: p.left + 6.0,
                        top: p.top + 3.0,
                        right: p.left + 36.0,
                        bottom: p.top + p.height - 3.0,
                    });
                let slider_rect = self
                    .hitbox_scratch
                    .iter()
                    .find(|hb| hb.action == UiAction::SeekVolume)
                    .map(|hb| hb.rect)
                    .unwrap_or(UiRectDip {
                        left: p.left + 44.0,
                        top: p.top + 2.0,
                        right: p.left + p.width - 50.0,
                        bottom: p.top + p.height - 2.0,
                    });

                let mute_hovered = input.hovered_action == Some(UiAction::ToggleVolumeMute);
                if mute_hovered {
                    let pill_btn = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: mute_rect.left,
                            top: mute_rect.top,
                            right: mute_rect.right,
                            bottom: mute_rect.bottom,
                        },
                        radiusX: 12.0,
                        radiusY: 12.0,
                    };
                    unsafe {
                        canvas
                            .ctx
                            .FillRoundedRectangle(&pill_btn, &self.brushes.surface_hover);
                    }
                }

                let cx = mute_rect.left + mute_rect.width() * 0.5;
                let cy = mute_rect.top + mute_rect.height() * 0.5;
                let speaker_brush = if muted {
                    &self.brushes.danger_border
                } else {
                    &self.brushes.text_primary
                };
                unsafe {
                    self.draw_audio_output_icon(
                        canvas,
                        cx,
                        cy,
                        input.current_audio_device_kind,
                        display_level,
                        muted,
                        1.0,
                        speaker_brush,
                    );
                }

                let dev_label = if show_device_name && !is_dragging {
                    input
                        .current_audio_device_name
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                } else {
                    None
                };

                let slider_hovered = input.hovered_action == Some(UiAction::SeekVolume);
                if let Some(dev_name) = dev_label {
                    let text_primary = self.brushes.text_primary.clone();
                    unsafe {
                        self.draw_text(
                            canvas,
                            dev_name,
                            TextFormatSlot::SmallLeft,
                            &text_primary,
                            UiRectDip {
                                left: slider_rect.left,
                                top: p.top + 4.5,
                                right: slider_rect.right,
                                bottom: p.top + p.height - 14.5,
                            },
                        );
                    }
                    let bar_h = if slider_hovered { 3.8 } else { 2.8 };
                    let bar_cy = p.top + p.height - 6.4;
                    let bar_top = bar_cy - bar_h * 0.5;
                    let bar_bot = bar_cy + bar_h * 0.5;
                    let bar_bg = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: slider_rect.left,
                            top: bar_top,
                            right: slider_rect.right,
                            bottom: bar_bot,
                        },
                        radiusX: bar_h * 0.5,
                        radiusY: bar_h * 0.5,
                    };
                    unsafe {
                        canvas
                            .ctx
                            .FillRoundedRectangle(&bar_bg, &self.brushes.switch_off);
                    }
                    if display_level > 0.004 {
                        let fill_w = (slider_rect.width() * display_level).max(bar_h);
                        let bar_fg = D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: slider_rect.left,
                                top: bar_top,
                                right: (slider_rect.left + fill_w).min(slider_rect.right),
                                bottom: bar_bot,
                            },
                            radiusX: bar_h * 0.5,
                            radiusY: bar_h * 0.5,
                        };
                        let fill_brush = if muted {
                            &self.brushes.danger_border
                        } else {
                            &self.brushes.accent
                        };
                        unsafe {
                            canvas.ctx.FillRoundedRectangle(&bar_fg, fill_brush);
                        }
                    }
                    if slider_hovered {
                        let knob_r = 3.8;
                        let knob_x = (slider_rect.left + slider_rect.width() * display_level)
                            .clamp(slider_rect.left + knob_r, slider_rect.right - knob_r);
                        let halo_r = knob_r + 2.0;
                        let halo = D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: knob_x - halo_r,
                                top: bar_cy - halo_r,
                                right: knob_x + halo_r,
                                bottom: bar_cy + halo_r,
                            },
                            radiusX: halo_r,
                            radiusY: halo_r,
                        };
                        let knob = D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: knob_x - knob_r,
                                top: bar_cy - knob_r,
                                right: knob_x + knob_r,
                                bottom: bar_cy + knob_r,
                            },
                            radiusX: knob_r,
                            radiusY: knob_r,
                        };
                        unsafe {
                            canvas
                                .ctx
                                .FillRoundedRectangle(&halo, &self.brushes.accent_soft);
                            canvas.ctx.FillRoundedRectangle(&knob, &self.brushes.knob);
                        }
                    }
                } else {
                    let track_h = if is_dragging {
                        6.5
                    } else if slider_hovered {
                        5.5
                    } else {
                        4.4
                    };
                    let bg_bar = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: slider_rect.left,
                            top: cy - track_h * 0.5,
                            right: slider_rect.right,
                            bottom: cy + track_h * 0.5,
                        },
                        radiusX: track_h * 0.5,
                        radiusY: track_h * 0.5,
                    };
                    unsafe {
                        canvas
                            .ctx
                            .FillRoundedRectangle(&bg_bar, &self.brushes.switch_off);
                    }

                    if display_level > 0.004 {
                        let fill_w = (slider_rect.width() * display_level).max(track_h);
                        let fg_bar = D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: slider_rect.left,
                                top: cy - track_h * 0.5,
                                right: (slider_rect.left + fill_w).min(slider_rect.right),
                                bottom: cy + track_h * 0.5,
                            },
                            radiusX: track_h * 0.5,
                            radiusY: track_h * 0.5,
                        };
                        let fill_brush = if muted {
                            &self.brushes.danger_border
                        } else {
                            &self.brushes.accent
                        };
                        unsafe {
                            canvas.ctx.FillRoundedRectangle(&fg_bar, fill_brush);
                        }
                    }

                    if slider_hovered || is_dragging {
                        let knob_r = if is_dragging { 6.2 } else { 5.2 };
                        let knob_x = (slider_rect.left + slider_rect.width() * display_level)
                            .clamp(slider_rect.left + knob_r, slider_rect.right - knob_r);
                        let halo_r = knob_r + 3.0;
                        let halo = D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: knob_x - halo_r,
                                top: cy - halo_r,
                                right: knob_x + halo_r,
                                bottom: cy + halo_r,
                            },
                            radiusX: halo_r,
                            radiusY: halo_r,
                        };
                        let knob = D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: knob_x - knob_r,
                                top: cy - knob_r,
                                right: knob_x + knob_r,
                                bottom: cy + knob_r,
                            },
                            radiusX: knob_r,
                            radiusY: knob_r,
                        };
                        unsafe {
                            canvas
                                .ctx
                                .FillRoundedRectangle(&halo, &self.brushes.accent_soft);
                            canvas.ctx.FillRoundedRectangle(&knob, &self.brushes.knob);
                        }
                    }
                }

                let label = if muted {
                    String::from("Tắt")
                } else {
                    format!("{}%", (display_level * 100.0).round() as u32)
                };
                let pct_brush = if muted {
                    self.brushes.danger_border.clone()
                } else {
                    self.brushes.text_primary.clone()
                };
                unsafe {
                    self.draw_text(
                        canvas,
                        &label,
                        TextFormatSlot::SmallRight,
                        &pct_brush,
                        UiRectDip {
                            left: slider_rect.right + 4.0,
                            top: p.top + 4.0,
                            right: p.left + p.width - 12.0,
                            bottom: p.top + p.height - 4.0,
                        },
                    );
                }
            }
            HudContent::Power {
                on_ac,
                percent,
                low_battery,
            } => {
                let cy = p.top + p.height * 0.5;
                let fill_brush = if on_ac {
                    &self.brushes.power_ok
                } else if low_battery {
                    &self.brushes.power_warn
                } else {
                    &self.brushes.accent
                };
                let pct_ratio = percent.map_or(0.5, |v| (v as f32 / 100.0).clamp(0.08, 1.0));
                unsafe {
                    self.draw_battery_icon(canvas, p.left + 21.0, cy, pct_ratio, on_ac, fill_brush);
                }

                let status_text = if on_ac {
                    "Đang cắm sạc"
                } else if low_battery {
                    "Pin yếu"
                } else {
                    "Đang dùng pin"
                };
                let pct_label = percent.map_or_else(|| String::from("--%"), |v| format!("{v}%"));
                let text_primary = self.brushes.text_primary.clone();
                let accent_brush = fill_brush.clone();
                unsafe {
                    self.draw_text(
                        canvas,
                        status_text,
                        TextFormatSlot::Compact,
                        &text_primary,
                        UiRectDip {
                            left: p.left + 38.0,
                            top: p.top + 4.0,
                            right: p.left + p.width - 54.0,
                            bottom: p.top + p.height - 4.0,
                        },
                    );
                    self.draw_text(
                        canvas,
                        &pct_label,
                        TextFormatSlot::SmallRight,
                        &accent_brush,
                        UiRectDip {
                            left: p.left + p.width - 54.0,
                            top: p.top + 4.0,
                            right: p.left + p.width - 12.0,
                            bottom: p.top + p.height - 4.0,
                        },
                    );
                }
            }
            HudContent::Device {
                category,
                connected,
                battery_percent,
            } => {
                let cy = p.top + p.height * 0.5;
                let text_primary = self.brushes.text_primary.clone();
                let text_secondary = self.brushes.text_secondary.clone();
                let icon_brush = if !connected {
                    self.brushes.text_secondary.clone()
                } else if category == DeviceHudCategory::Bluetooth {
                    self.brushes.sys_accent.clone()
                } else {
                    self.brushes.power_ok.clone()
                };
                unsafe {
                    self.draw_device_hud_icon(canvas, p.left + 21.0, cy, category, &icon_brush);
                }

                let device_title = input
                    .hud_device_name
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Thiết bị ngoại vi");
                let status_line = match (category, connected, battery_percent) {
                    (DeviceHudCategory::Bluetooth, true, Some(pct)) => {
                        format!("Đã kết nối · Pin {pct}%")
                    }
                    (_, true, _) => String::from("Đã kết nối"),
                    (DeviceHudCategory::Bluetooth, false, _) => String::from("Đã ngắt kết nối"),
                    (_, false, _) => String::from("Đã rút thiết bị"),
                };
                let right_reserve = if connected && battery_percent.is_some() {
                    68.0
                } else {
                    18.0
                };
                let sub_brush = if connected {
                    &icon_brush
                } else {
                    &text_secondary
                };
                unsafe {
                    self.draw_text(
                        canvas,
                        device_title,
                        TextFormatSlot::Compact,
                        &text_primary,
                        UiRectDip {
                            left: p.left + 40.0,
                            top: p.top + 4.0,
                            right: p.left + p.width - right_reserve,
                            bottom: p.top + 20.0,
                        },
                    );
                    self.draw_text(
                        canvas,
                        &status_line,
                        TextFormatSlot::CompactSub,
                        sub_brush,
                        UiRectDip {
                            left: p.left + 40.0,
                            top: p.top + 19.0,
                            right: p.left + p.width - right_reserve,
                            bottom: p.top + 34.0,
                        },
                    );
                }
                if connected && let Some(pct) = battery_percent {
                    let bat_brush = if pct <= 20 {
                        self.brushes.power_warn.clone()
                    } else {
                        self.brushes.power_ok.clone()
                    };
                    let pct_text = format!("{pct}%");
                    unsafe {
                        self.draw_battery_icon(
                            canvas,
                            p.left + p.width - 23.0,
                            cy - 5.0,
                            (pct as f32 / 100.0).clamp(0.08, 1.0),
                            false,
                            &bat_brush,
                        );
                        self.draw_text(
                            canvas,
                            &pct_text,
                            TextFormatSlot::CompactSub,
                            &bat_brush,
                            UiRectDip {
                                left: p.left + p.width - 62.0,
                                top: p.top + 21.0,
                                right: p.left + p.width - 13.0,
                                bottom: p.top + 36.0,
                            },
                        );
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) unsafe fn draw_audio_output_icon(
        &self,
        canvas: DrawCanvas<'_>,
        cx: f32,
        cy: f32,
        kind: AudioOutputKind,
        level: f32,
        muted: bool,
        scale: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        match kind {
            AudioOutputKind::Speaker => unsafe {
                self.draw_speaker_icon_scaled(canvas, cx, cy, level, muted, scale, brush);
            },
            AudioOutputKind::Headphones => unsafe {
                self.draw_headphones_icon(canvas, cx, cy, scale, muted, brush);
            },
            AudioOutputKind::Bluetooth => unsafe {
                self.draw_bluetooth_icon(canvas, cx, cy, scale, muted, brush);
            },
        }
    }

    unsafe fn draw_device_hud_icon(
        &self,
        canvas: DrawCanvas<'_>,
        cx: f32,
        cy: f32,
        category: DeviceHudCategory,
        brush: &ID2D1SolidColorBrush,
    ) {
        match category {
            DeviceHudCategory::Bluetooth => unsafe {
                self.draw_bluetooth_icon(canvas, cx, cy, 1.05, false, brush);
            },
            DeviceHudCategory::Headphones => unsafe {
                self.draw_headphones_icon(canvas, cx, cy, 1.05, false, brush);
            },
            DeviceHudCategory::Usb => unsafe {
                self.draw_usb_icon(canvas, cx, cy, brush);
            },
            DeviceHudCategory::Peripheral => unsafe {
                self.draw_peripheral_icon(canvas, cx, cy, brush);
            },
        }
    }

    unsafe fn draw_bluetooth_icon(
        &self,
        canvas: DrawCanvas<'_>,
        cx: f32,
        cy: f32,
        scale: f32,
        muted: bool,
        brush: &ID2D1SolidColorBrush,
    ) {
        let s = scale.clamp(0.65, 1.3);
        let stroke = Some(&self.icons.round_stroke);
        let w = 1.5 * s;
        let pts = [
            (0.0, -6.5 * s, 0.0, 6.5 * s),
            (0.0, -6.5 * s, 4.2 * s, -3.0 * s),
            (4.2 * s, -3.0 * s, -4.0 * s, 3.0 * s),
            (0.0, 6.5 * s, 4.2 * s, 3.0 * s),
            (4.2 * s, 3.0 * s, -4.0 * s, -3.0 * s),
        ];
        unsafe {
            for (x1, y1, x2, y2) in pts {
                canvas.ctx.DrawLine(
                    Vector2 { X: cx + x1, Y: cy + y1 },
                    Vector2 { X: cx + x2, Y: cy + y2 },
                    brush,
                    w,
                    stroke,
                );
            }
            if muted {
                canvas.ctx.DrawLine(
                    Vector2 { X: cx - 5.2 * s, Y: cy + 5.2 * s },
                    Vector2 { X: cx + 5.2 * s, Y: cy - 5.2 * s },
                    brush,
                    w,
                    stroke,
                );
            }
        }
    }

    unsafe fn draw_headphones_icon(
        &self,
        canvas: DrawCanvas<'_>,
        cx: f32,
        cy: f32,
        scale: f32,
        muted: bool,
        brush: &ID2D1SolidColorBrush,
    ) {
        let s = scale.clamp(0.65, 1.3);
        let stroke = Some(&self.icons.round_stroke);
        let w = 1.55 * s;
        unsafe {
            // Vòng cung trùm đầu
            canvas.ctx.DrawLine(
                Vector2 { X: cx - 5.4 * s, Y: cy + 1.0 * s },
                Vector2 { X: cx - 4.4 * s, Y: cy - 3.6 * s },
                brush,
                w,
                stroke,
            );
            canvas.ctx.DrawLine(
                Vector2 { X: cx - 4.4 * s, Y: cy - 3.6 * s },
                Vector2 { X: cx, Y: cy - 5.4 * s },
                brush,
                w,
                stroke,
            );
            canvas.ctx.DrawLine(
                Vector2 { X: cx, Y: cy - 5.4 * s },
                Vector2 { X: cx + 4.4 * s, Y: cy - 3.6 * s },
                brush,
                w,
                stroke,
            );
            canvas.ctx.DrawLine(
                Vector2 { X: cx + 4.4 * s, Y: cy - 3.6 * s },
                Vector2 { X: cx + 5.4 * s, Y: cy + 1.0 * s },
                brush,
                w,
                stroke,
            );
            // Hai chụp tai
            let left_cup = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: cx - 6.6 * s,
                    top: cy - 0.6 * s,
                    right: cx - 3.2 * s,
                    bottom: cy + 5.4 * s,
                },
                radiusX: 1.5 * s,
                radiusY: 1.5 * s,
            };
            let right_cup = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: cx + 3.2 * s,
                    top: cy - 0.6 * s,
                    right: cx + 6.6 * s,
                    bottom: cy + 5.4 * s,
                },
                radiusX: 1.5 * s,
                radiusY: 1.5 * s,
            };
            canvas.ctx.FillRoundedRectangle(&left_cup, brush);
            canvas.ctx.FillRoundedRectangle(&right_cup, brush);
            if muted {
                canvas.ctx.DrawLine(
                    Vector2 { X: cx - 5.6 * s, Y: cy + 5.4 * s },
                    Vector2 { X: cx + 5.6 * s, Y: cy - 5.0 * s },
                    brush,
                    w,
                    stroke,
                );
            }
        }
    }

    unsafe fn draw_usb_icon(
        &self,
        canvas: DrawCanvas<'_>,
        cx: f32,
        cy: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let cap = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - 3.2,
                top: cy - 6.4,
                right: cx + 3.2,
                bottom: cy - 2.2,
            },
            radiusX: 0.9,
            radiusY: 0.9,
        };
        let body = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - 4.6,
                top: cy - 2.4,
                right: cx + 4.6,
                bottom: cy + 6.4,
            },
            radiusX: 2.0,
            radiusY: 2.0,
        };
        unsafe {
            canvas.ctx.DrawRoundedRectangle(&cap, brush, 1.35, None);
            canvas.ctx.FillRoundedRectangle(&body, brush);
        }
    }

    unsafe fn draw_peripheral_icon(
        &self,
        canvas: DrawCanvas<'_>,
        cx: f32,
        cy: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let outer = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - 6.0,
                top: cy - 5.4,
                right: cx + 6.0,
                bottom: cy + 5.4,
            },
            radiusX: 2.8,
            radiusY: 2.8,
        };
        let inner = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - 2.2,
                top: cy - 2.2,
                right: cx + 2.2,
                bottom: cy + 2.2,
            },
            radiusX: 2.2,
            radiusY: 2.2,
        };
        unsafe {
            canvas.ctx.DrawRoundedRectangle(&outer, brush, 1.45, None);
            canvas.ctx.FillRoundedRectangle(&inner, brush);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) unsafe fn draw_speaker_icon_scaled(
        &self,
        canvas: DrawCanvas<'_>,
        cx: f32,
        cy: f32,
        level: f32,
        muted: bool,
        scale: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let s = scale.clamp(0.65, 1.3);
        let box_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - 7.5 * s,
                top: cy - 2.6 * s,
                right: cx - 3.6 * s,
                bottom: cy + 2.6 * s,
            },
            radiusX: 1.0 * s,
            radiusY: 1.0 * s,
        };
        unsafe {
            canvas.ctx.FillRoundedRectangle(&box_rect, brush);
        }
        if let Ok(base) = self.icons.speaker_cone.cast::<ID2D1Geometry>() {
            let mat = Matrix3x2 {
                M11: s,
                M12: 0.0,
                M21: 0.0,
                M22: s,
                M31: canvas.offset_x_px + (cx - 2.4 * s) * canvas.scale,
                M32: canvas.offset_y_px + cy * canvas.scale,
            };
            let _ = mat;
            canvas.set_local_translation(cx - 2.4 * s, cy);
            unsafe {
                canvas.ctx.FillGeometry(&base, brush, None);
                canvas
                    .ctx
                    .DrawGeometry(&base, brush, 1.4, &self.icons.round_stroke);
            }
            canvas.reset_transform();
        }
        if muted {
            unsafe {
                canvas.ctx.DrawLine(
                    Vector2 {
                        X: cx + 2.0 * s,
                        Y: cy - 3.2 * s,
                    },
                    Vector2 {
                        X: cx + 7.2 * s,
                        Y: cy + 3.2 * s,
                    },
                    brush,
                    1.6 * s,
                    Some(&self.icons.round_stroke),
                );
                canvas.ctx.DrawLine(
                    Vector2 {
                        X: cx + 7.2 * s,
                        Y: cy - 3.2 * s,
                    },
                    Vector2 {
                        X: cx + 2.0 * s,
                        Y: cy + 3.2 * s,
                    },
                    brush,
                    1.6 * s,
                    Some(&self.icons.round_stroke),
                );
            }
        } else {
            if level > 0.02 {
                let w1 = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: cx + 2.4 * s,
                        top: cy - 2.6 * s,
                        right: cx + 4.0 * s,
                        bottom: cy + 2.6 * s,
                    },
                    radiusX: 0.8 * s,
                    radiusY: 0.8 * s,
                };
                unsafe {
                    canvas.ctx.FillRoundedRectangle(&w1, brush);
                }
            }
            if level >= 0.45 {
                let w2 = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: cx + 5.2 * s,
                        top: cy - 4.4 * s,
                        right: cx + 6.8 * s,
                        bottom: cy + 4.4 * s,
                    },
                    radiusX: 0.8 * s,
                    radiusY: 0.8 * s,
                };
                unsafe {
                    canvas.ctx.FillRoundedRectangle(&w2, brush);
                }
            }
        }
    }

    pub(super) unsafe fn draw_battery_icon(
        &self,
        canvas: DrawCanvas<'_>,
        cx: f32,
        cy: f32,
        ratio: f32,
        on_ac: bool,
        fill_brush: &ID2D1SolidColorBrush,
    ) {
        let body = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - 10.0,
                top: cy - 5.5,
                right: cx + 7.0,
                bottom: cy + 5.5,
            },
            radiusX: 2.5,
            radiusY: 2.5,
        };
        let nub = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx + 7.5,
                top: cy - 2.4,
                right: cx + 9.6,
                bottom: cy + 2.4,
            },
            radiusX: 1.0,
            radiusY: 1.0,
        };
        let inner_max_w = 13.4_f32;
        let inner_w = (inner_max_w * ratio.clamp(0.08, 1.0)).max(2.0);
        let fill = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - 8.2,
                top: cy - 3.7,
                right: cx - 8.2 + inner_w,
                bottom: cy + 3.7,
            },
            radiusX: 1.4,
            radiusY: 1.4,
        };
        unsafe {
            canvas.ctx.FillRoundedRectangle(&fill, fill_brush);
            canvas.ctx.DrawRoundedRectangle(
                &body,
                &self.brushes.text_secondary,
                1.25,
                None,
            );
            canvas
                .ctx
                .FillRoundedRectangle(&nub, &self.brushes.text_secondary);
            if on_ac {
                canvas.ctx.DrawLine(
                    Vector2 {
                        X: cx - 1.2,
                        Y: cy - 3.2,
                    },
                    Vector2 {
                        X: cx - 3.0,
                        Y: cy + 0.2,
                    },
                    &self.brushes.text_primary,
                    1.3,
                    Some(&self.icons.round_stroke),
                );
                canvas.ctx.DrawLine(
                    Vector2 {
                        X: cx - 3.0,
                        Y: cy + 0.2,
                    },
                    Vector2 {
                        X: cx + 0.6,
                        Y: cy + 0.2,
                    },
                    &self.brushes.text_primary,
                    1.3,
                    Some(&self.icons.round_stroke),
                );
                canvas.ctx.DrawLine(
                    Vector2 {
                        X: cx + 0.6,
                        Y: cy + 0.2,
                    },
                    Vector2 {
                        X: cx - 1.2,
                        Y: cy + 3.4,
                    },
                    &self.brushes.text_primary,
                    1.3,
                    Some(&self.icons.round_stroke),
                );
            }
        }
    }

    pub(super) unsafe fn draw_audio_devices(
        &mut self,
        canvas: DrawCanvas<'_>,
        input: &RenderFrameInput<'_>,
    ) {
        let target_h = audio_devices_height_for_count(input.audio_devices.len());
        let p = virtual_pill_anchored(input.pill, 424.0, target_h, false);
        let text_primary = self.brushes.text_primary.clone();
        let text_secondary = self.brushes.text_secondary.clone();
        let sys_accent = self.brushes.sys_accent.clone();

        let total = input.audio_devices.len();
        let max_scroll = total.saturating_sub(MAX_AUDIO_DEVICES_VISIBLE);
        let start = input.audio_device_scroll.min(max_scroll);
        let end = (start + MAX_AUDIO_DEVICES_VISIBLE).min(total);
        let visible_count = end.saturating_sub(start);

        let vol_status = if !input.volume_snapshot.available {
            String::from("Chưa có dịch vụ âm thanh")
        } else if input.volume_snapshot.muted {
            format!(
                "Đang tắt tiếng ({}%)",
                (input.volume_snapshot.level * 100.0).round() as u32
            )
        } else {
            format!(
                "Âm lượng thiết bị hiện tại: {}%",
                (input.volume_snapshot.level * 100.0).round() as u32
            )
        };

        unsafe {
            self.draw_text(
                canvas,
                "Thiết bị phát âm thanh",
                TextFormatSlot::Title,
                &text_primary,
                UiRectDip {
                    left: p.left + 18.0,
                    top: p.top + 10.0,
                    right: p.left + p.width - 96.0,
                    bottom: p.top + 32.0,
                },
            );
            self.draw_text(
                canvas,
                &vol_status,
                TextFormatSlot::SmallLeft,
                &text_secondary,
                UiRectDip {
                    left: p.left + 18.0,
                    top: p.top + 32.0,
                    right: p.left + p.width - 96.0,
                    bottom: p.top + 49.0,
                },
            );
        }

        fill_audio_devices_hitboxes(p, visible_count, &mut self.hitbox_scratch);
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
                radiusX: 10.0,
                radiusY: 10.0,
            };

            match hb.action {
                UiAction::BackToMedia => {
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
                            "Quay lại",
                            TextFormatSlot::ButtonCenter,
                            &text_primary,
                            hb.rect,
                        );
                    }
                }
                UiAction::SelectAudioDevice(rel_idx) => {
                    let Some(dev) = input.audio_devices.get(start + rel_idx) else {
                        continue;
                    };
                    let is_default = dev.is_default;
                    let fill = if hovered {
                        &self.brushes.surface_hover
                    } else if is_default {
                        &self.brushes.sys_accent_soft
                    } else {
                        &self.brushes.surface_card
                    };
                    let border_brush = if is_default {
                        &self.brushes.sys_accent
                    } else {
                        &self.brushes.card_border
                    };
                    let border_w = if is_default { 1.1 } else { 0.85 };
                    unsafe {
                        canvas.ctx.FillRoundedRectangle(&card, fill);
                        canvas.ctx.DrawRoundedRectangle(&card, border_brush, border_w, None);
                    }

                    let icon_brush = if is_default {
                        if input.volume_snapshot.muted {
                            self.brushes.danger_border.clone()
                        } else {
                            sys_accent.clone()
                        }
                    } else {
                        text_secondary.clone()
                    };
                    let icon_level = if is_default {
                        input.volume_snapshot.level
                    } else {
                        0.65
                    };
                    let icon_muted = is_default && input.volume_snapshot.muted;
                    let name_text = dev.name.clone();
                    let cy = hb.rect.top + hb.rect.height() * 0.5;
                    let text_right = if is_default {
                        hb.rect.right - 82.0
                    } else {
                        hb.rect.right - 14.0
                    };
                    unsafe {
                        self.draw_audio_output_icon(
                            canvas,
                            hb.rect.left + 19.0,
                            cy,
                            dev.kind,
                            icon_level,
                            icon_muted,
                            1.0,
                            &icon_brush,
                        );
                        self.draw_text(
                            canvas,
                            &name_text,
                            TextFormatSlot::Button,
                            &text_primary,
                            UiRectDip {
                                left: hb.rect.left + 35.0,
                                top: hb.rect.top,
                                right: text_right,
                                bottom: hb.rect.bottom,
                            },
                        );
                        if is_default {
                            self.draw_text(
                                canvas,
                                "Đang dùng",
                                TextFormatSlot::SmallRight,
                                &sys_accent,
                                UiRectDip {
                                    left: hb.rect.right - 78.0,
                                    top: hb.rect.top,
                                    right: hb.rect.right - 12.0,
                                    bottom: hb.rect.bottom,
                                },
                            );
                        }
                    }
                }
                _ => {}
            }
        }

        if total == 0 {
            let empty_card = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: p.left + 16.0,
                    top: p.top + 56.0,
                    right: p.left + p.width - 16.0,
                    bottom: p.top + 96.0,
                },
                radiusX: 10.0,
                radiusY: 10.0,
            };
            unsafe {
                canvas
                    .ctx
                    .FillRoundedRectangle(&empty_card, &self.brushes.surface_card);
                self.draw_text(
                    canvas,
                    "Chưa phát hiện thiết bị phát âm thanh",
                    TextFormatSlot::ButtonCenter,
                    &text_secondary,
                    UiRectDip {
                        left: empty_card.rect.left,
                        top: empty_card.rect.top,
                        right: empty_card.rect.right,
                        bottom: empty_card.rect.bottom,
                    },
                );
            }
        } else if total > MAX_AUDIO_DEVICES_VISIBLE {
            let hint = format!("Cuộn chuột để xem thêm ({}-{total}/{total})", start + 1);
            let _ = end;
            unsafe {
                self.draw_text(
                    canvas,
                    &hint,
                    TextFormatSlot::Small,
                    &text_secondary,
                    UiRectDip {
                        left: p.left + 16.0,
                        top: p.top + target_h - 22.0,
                        right: p.left + p.width - 16.0,
                        bottom: p.top + target_h - 6.0,
                    },
                );
            }
        }
    }
}
