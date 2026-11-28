use super::*;

impl IslandRenderer {
    pub(super) unsafe fn draw_expanded(&mut self, canvas: DrawCanvas<'_>, input: &RenderFrameInput<'_>) {
        let p = virtual_pill_anchored(input.pill, 424.0, 160.0, false);
        let art_size = 64.0;
        let art_x = p.left + 18.0;
        let art_y = p.top + 16.0;
        unsafe {
            self.draw_artwork_or_placeholder(
                canvas,
                art_x,
                art_y,
                art_size,
                14.0,
                input.media.is_playing,
            );
        }

        let text_left = art_x + art_size + 14.0;
        let text_right = p.left + p.width - 18.0;

        let text_primary = self.brushes.text_primary.clone();
        let text_secondary = self.brushes.text_secondary.clone();
        let accent = self.brushes.accent.clone();

        unsafe {
            self.draw_text(
                canvas,
                &input.media.title,
                TextFormatSlot::Title,
                &text_primary,
                UiRectDip {
                    left: text_left,
                    top: p.top + 16.0,
                    right: text_right,
                    bottom: p.top + 38.0,
                },
            );
            self.draw_text(
                canvas,
                &input.media.artist,
                TextFormatSlot::Subtitle,
                &text_secondary,
                UiRectDip {
                    left: text_left,
                    top: p.top + 40.0,
                    right: text_right,
                    bottom: p.top + 58.0,
                },
            );
        }

        let status_label = if input.media.has_session {
            if input.media.source_app.is_empty() {
                if input.media.is_playing {
                    "Đang phát"
                } else {
                    "Đã tạm dừng"
                }
            } else {
                input.media.source_app.as_str()
            }
        } else {
            "Bấm Cài đặt hoặc chuột phải mở menu"
        };
        unsafe {
            self.draw_text(
                canvas,
                status_label,
                TextFormatSlot::SmallLeft,
                &accent,
                UiRectDip {
                    left: text_left,
                    top: p.top + 60.0,
                    right: text_right,
                    bottom: p.top + 78.0,
                },
            );
        }

        let now_ft = current_windows_filetime_ticks();
        let tl = input.media.timeline;
        let is_scrubbing = input.timeline_scrub_ratio.is_some();
        let (pos_label, dur_label, ratio) = if input.media.has_session && tl.available {
            let dur = tl.duration_ticks();
            if let Some(scrub) = input.timeline_scrub_ratio {
                let clamped = scrub.clamp(0.0, 1.0);
                let pos = tl.seek_ticks_for_ratio(clamped).unwrap_or(0);
                (
                    format_duration_ticks(pos),
                    format_duration_ticks(dur),
                    clamped,
                )
            } else {
                let pos = tl.current_position_ticks(now_ft, input.media.is_playing);
                (
                    format_duration_ticks(pos),
                    format_duration_ticks(dur),
                    tl.progress_ratio(now_ft, input.media.is_playing),
                )
            }
        } else {
            (String::from("--:--"), String::from("--:--"), 0.0)
        };

        unsafe {
            self.draw_text(
                canvas,
                &pos_label,
                TextFormatSlot::SmallLeft,
                &text_secondary,
                UiRectDip {
                    left: p.left + 18.0,
                    top: p.top + 85.5,
                    right: p.left + 58.0,
                    bottom: p.top + 105.5,
                },
            );
            self.draw_text(
                canvas,
                &dur_label,
                TextFormatSlot::SmallRight,
                &text_secondary,
                UiRectDip {
                    left: p.left + p.width - 58.0,
                    top: p.top + 85.5,
                    right: p.left + p.width - 18.0,
                    bottom: p.top + 105.5,
                },
            );
        }

        fill_interactive_hitboxes(p, IslandMode::Expanded, &mut self.hitbox_scratch);
        let count = self.hitbox_scratch.len();
        for idx in 0..count {
            let hb = self.hitbox_scratch[idx];
            let hovered = input.hovered_action == Some(hb.action);
            if hb.action == UiAction::SeekTimeline {
                unsafe {
                    self.draw_timeline_bar(
                        canvas,
                        hb.rect,
                        ratio,
                        input.media.has_session && tl.available,
                        hovered || is_scrubbing,
                        is_scrubbing,
                    );
                }
                continue;
            }
            let fill = if hovered {
                &self.brushes.surface_hover
            } else if hb.action == UiAction::MediaPlayPause {
                &self.brushes.accent_soft
            } else {
                &self.brushes.surface_card
            };
            let radius = hb.rect.height() * 0.5;
            let card = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: hb.rect.left,
                    top: hb.rect.top,
                    right: hb.rect.right,
                    bottom: hb.rect.bottom,
                },
                radiusX: radius,
                radiusY: radius,
            };
            unsafe {
                canvas.ctx.FillRoundedRectangle(&card, fill);
            }
            if hb.action == UiAction::OpenSettings {
                let icon_brush = text_secondary.clone();
                let icon_cx = hb.rect.left + 21.0;
                let icon_cy = hb.rect.top + hb.rect.height() * 0.5;
                unsafe {
                    self.draw_gear_icon(canvas, icon_cx, icon_cy, &icon_brush);
                    self.draw_text(
                        canvas,
                        "Cài đặt",
                        TextFormatSlot::Button,
                        &text_primary,
                        UiRectDip {
                            left: hb.rect.left + 32.0,
                            top: hb.rect.top,
                            right: hb.rect.right - 10.0,
                            bottom: hb.rect.bottom,
                        },
                    );
                }
            } else if hb.action == UiAction::OpenAudioDevices {
                let speaker_brush = if input.volume_snapshot.muted {
                    self.brushes.danger_border.clone()
                } else {
                    accent.clone()
                };
                let dev_label = input
                    .current_audio_device_name
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Âm thanh");
                unsafe {
                    self.draw_speaker_icon(
                        canvas,
                        hb.rect.left + 14.0,
                        hb.rect.top + hb.rect.height() * 0.5,
                        if input.volume_snapshot.available {
                            input.volume_snapshot.level
                        } else {
                            0.6
                        },
                        input.volume_snapshot.muted,
                        &speaker_brush,
                    );
                    self.draw_text(
                        canvas,
                        dev_label,
                        TextFormatSlot::SmallLeft,
                        &text_primary,
                        UiRectDip {
                            left: hb.rect.left + 26.0,
                            top: hb.rect.top,
                            right: hb.rect.right - 8.0,
                            bottom: hb.rect.bottom,
                        },
                    );
                }
            } else {
                let icon_brush = if input.media.has_session {
                    &text_primary
                } else {
                    &text_secondary
                };
                unsafe {
                    self.draw_media_action_icon(
                        canvas,
                        hb.action,
                        hb.rect,
                        input.media.is_playing,
                        icon_brush,
                    );
                }
            }
        }
    }

    pub(super) unsafe fn draw_gear_icon(
        &self,
        canvas: DrawCanvas<'_>,
        cx: f32,
        cy: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let ring = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - 3.6,
                top: cy - 3.6,
                right: cx + 3.6,
                bottom: cy + 3.6,
            },
            radiusX: 3.6,
            radiusY: 3.6,
        };
        let core = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - 1.3,
                top: cy - 1.3,
                right: cx + 1.3,
                bottom: cy + 1.3,
            },
            radiusX: 1.3,
            radiusY: 1.3,
        };
        unsafe {
            canvas.ctx.DrawRoundedRectangle(&ring, brush, 1.45, None);
            canvas.ctx.FillRoundedRectangle(&core, brush);
            for idx in 0..8 {
                let angle = idx as f32 * std::f32::consts::FRAC_PI_4;
                let (sin_a, cos_a) = angle.sin_cos();
                canvas.ctx.DrawLine(
                    Vector2 {
                        X: cx + cos_a * 3.7,
                        Y: cy + sin_a * 3.7,
                    },
                    Vector2 {
                        X: cx + cos_a * 5.6,
                        Y: cy + sin_a * 5.6,
                    },
                    brush,
                    1.55,
                    Some(&self.icons.round_stroke),
                );
            }
        }
    }

    pub(super) unsafe fn draw_media_action_icon(
        &self,
        canvas: DrawCanvas<'_>,
        action: UiAction,
        rect: UiRectDip,
        is_playing: bool,
        brush: &ID2D1SolidColorBrush,
    ) {
        let cx = rect.left + rect.width() * 0.5;
        let cy = rect.top + rect.height() * 0.5;
        match action {
            UiAction::MediaPlayPause if is_playing => {
                for offset in [-3.4f32, 3.4f32] {
                    let bar = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: cx + offset - 1.7,
                            top: cy - 6.4,
                            right: cx + offset + 1.7,
                            bottom: cy + 6.4,
                        },
                        radiusX: 1.7,
                        radiusY: 1.7,
                    };
                    unsafe {
                        canvas.ctx.FillRoundedRectangle(&bar, brush);
                    }
                }
            }
            UiAction::MediaPlayPause => unsafe {
                self.draw_centered_triangle(
                    canvas,
                    &self.icons.play_triangle,
                    cx + 0.8,
                    cy,
                    2.6,
                    brush,
                );
            },
            UiAction::MediaPrev => {
                let bar = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: cx - 6.0,
                        top: cy - 5.6,
                        right: cx - 3.6,
                        bottom: cy + 5.6,
                    },
                    radiusX: 1.2,
                    radiusY: 1.2,
                };
                unsafe {
                    canvas.ctx.FillRoundedRectangle(&bar, brush);
                    self.draw_centered_triangle(
                        canvas,
                        &self.icons.prev_triangle,
                        cx + 1.8,
                        cy,
                        2.2,
                        brush,
                    );
                }
            }
            UiAction::MediaNext => {
                let bar = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: cx + 3.6,
                        top: cy - 5.6,
                        right: cx + 6.0,
                        bottom: cy + 5.6,
                    },
                    radiusX: 1.2,
                    radiusY: 1.2,
                };
                unsafe {
                    canvas.ctx.FillRoundedRectangle(&bar, brush);
                    self.draw_centered_triangle(
                        canvas,
                        &self.icons.next_triangle,
                        cx - 1.8,
                        cy,
                        2.2,
                        brush,
                    );
                }
            }
            _ => {}
        }
    }

    pub(super) unsafe fn draw_centered_triangle(
        &self,
        canvas: DrawCanvas<'_>,
        geom: &ID2D1PathGeometry,
        cx: f32,
        cy: f32,
        round_stroke_w: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let Ok(base) = geom.cast::<ID2D1Geometry>() else {
            return;
        };
        canvas.set_local_translation(cx, cy);
        unsafe {
            canvas.ctx.FillGeometry(&base, brush, None);
            canvas
                .ctx
                .DrawGeometry(&base, brush, round_stroke_w, &self.icons.round_stroke);
        }
        canvas.reset_transform();
    }

    pub(super) unsafe fn draw_timeline_bar(
        &self,
        canvas: DrawCanvas<'_>,
        rect: UiRectDip,
        ratio: f32,
        active: bool,
        hovered: bool,
        dragging: bool,
    ) {
        let track_h = if dragging && active {
            6.5
        } else if hovered && active {
            5.5
        } else {
            4.0
        };
        let cy = rect.top + rect.height() * 0.5;
        let bg_bar = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: rect.left,
                top: cy - track_h * 0.5,
                right: rect.right,
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

        if !active {
            return;
        }

        let fill_w = (rect.width() * ratio.clamp(0.0, 1.0)).max(track_h);
        let fill_right = (rect.left + fill_w).min(rect.right);
        let fg_bar = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: rect.left,
                top: cy - track_h * 0.5,
                right: fill_right,
                bottom: cy + track_h * 0.5,
            },
            radiusX: track_h * 0.5,
            radiusY: track_h * 0.5,
        };
        unsafe {
            canvas
                .ctx
                .FillRoundedRectangle(&fg_bar, &self.brushes.accent);
        }

        let knob_r = if dragging {
            6.5
        } else if hovered {
            5.5
        } else {
            4.0
        };
        let knob_x = (rect.left + rect.width() * ratio.clamp(0.0, 1.0))
            .clamp(rect.left + knob_r, rect.right - knob_r);
        if hovered || dragging {
            let halo_r = knob_r + 3.5;
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
            unsafe {
                canvas
                    .ctx
                    .FillRoundedRectangle(&halo, &self.brushes.accent_soft);
            }
        }
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
            canvas.ctx.FillRoundedRectangle(&knob, &self.brushes.knob);
        }
    }
}
