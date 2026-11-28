use super::*;

impl IslandRenderer {
    pub(super) unsafe fn draw_slim(&mut self, canvas: DrawCanvas<'_>, input: &RenderFrameInput<'_>) {
        let (sw, sh, _, _) = target_dimensions_for_mode(IslandMode::Slim);
        let p = virtual_pill_anchored(input.pill, sw, sh, false);
        let cy = p.top + p.height * 0.5;

        if !input.media.has_session {
            // Chế độ chờ tối giản chuẩn Notch: chấm trạng thái dịu nhẹ + vạch định vị tâm siêu mảnh.
            let dot = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: p.left + 11.0,
                    top: cy - 2.2,
                    right: p.left + 15.4,
                    bottom: cy + 2.2,
                },
                radiusX: 2.2,
                radiusY: 2.2,
            };
            let cx = p.left + p.width * 0.5;
            let notch_bar = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: cx - 14.0,
                    top: cy - 1.2,
                    right: cx + 14.0,
                    bottom: cy + 1.2,
                },
                radiusX: 1.2,
                radiusY: 1.2,
            };
            unsafe {
                if let Some(snap) = input.anti {
                    let anti_brush = self.brushes.anti_accent.clone();
                    anti_brush.SetColor(&anti_view::anti_state_color(snap.state));
                    anti_brush.SetOpacity(1.0);
                    canvas.ctx.FillRoundedRectangle(&dot, &anti_brush);
                } else {
                    canvas
                        .ctx
                        .FillRoundedRectangle(&dot, &self.brushes.sys_accent_soft);
                }
                canvas
                    .ctx
                    .FillRoundedRectangle(&notch_bar, &self.brushes.card_border);
            }
            return;
        }

        let art_size = 10.0;
        let art_x = p.left + 8.0;
        let art_y = p.top + (p.height - art_size) * 0.5;
        unsafe {
            self.draw_artwork_or_placeholder(
                canvas,
                art_x,
                art_y,
                art_size,
                art_size * 0.5,
                input.media.is_playing,
            );
        }

        let vis_active = input.config.visualizer_enabled && input.media.is_playing;
        let track_left = art_x + art_size + 7.0;
        let right_reserve = if vis_active { 25.0 } else { 22.0 };
        let track_right = (p.left + p.width - right_reserve).max(track_left + 12.0);
        let track_h = 2.4;

        let bg_bar = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: track_left,
                top: cy - track_h * 0.5,
                right: track_right,
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

        let track_w = (track_right - track_left).max(1.0);
        let tl = input.media.timeline;
        let (fill_w, fill_brush) = if tl.available && tl.duration_ticks() > 0 {
            let ratio =
                tl.progress_ratio(current_windows_filetime_ticks(), input.media.is_playing);
            ((track_w * ratio).max(track_h), &self.brushes.accent)
        } else {
            (track_w, &self.brushes.accent_soft)
        };
        let fg_bar = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: track_left,
                top: cy - track_h * 0.5,
                right: (track_left + fill_w).min(track_right),
                bottom: cy + track_h * 0.5,
            },
            radiusX: track_h * 0.5,
            radiusY: track_h * 0.5,
        };
        unsafe {
            canvas.ctx.FillRoundedRectangle(&fg_bar, fill_brush);
        }

        if vis_active {
            unsafe {
                self.draw_visualizer_bars_count(
                    canvas,
                    p.left + p.width - 20.0,
                    cy,
                    input.visualizer_phase,
                    2.5,
                    6.5,
                    4.0,
                    2.2,
                    3,
                );
            }
        } else if !input.media.is_playing {
            let pause_cx = p.left + p.width - 14.0;
            for dx in [-1.8_f32, 1.8_f32] {
                let bar = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: pause_cx + dx - 0.85,
                        top: cy - 3.0,
                        right: pause_cx + dx + 0.85,
                        bottom: cy + 3.0,
                    },
                    radiusX: 0.85,
                    radiusY: 0.85,
                };
                unsafe {
                    canvas
                        .ctx
                        .FillRoundedRectangle(&bar, &self.brushes.text_secondary);
                }
            }
        }
    }

    pub(super) unsafe fn draw_compact(&mut self, canvas: DrawCanvas<'_>, input: &RenderFrameInput<'_>) {
        if let Some(hud) = input.active_hud {
            unsafe {
                self.draw_compact_hud(canvas, input, hud);
            }
            return;
        }
        if let Some(snap) = input.anti {
            unsafe {
                self.draw_compact_anti(canvas, input, snap);
            }
            return;
        }
        let (cw, ch, _, _) = target_dimensions_for_mode(IslandMode::Compact);
        let p = virtual_pill_anchored(input.pill, cw, ch, true);
        let art_size = 26.0;
        let art_x = p.left + 7.0;
        let art_y = p.top + (p.height - art_size) * 0.5;
        unsafe {
            self.draw_artwork_with_rim(
                canvas,
                art_x,
                art_y,
                art_size,
                art_size * 0.5,
                input.media.is_playing,
                true,
            );
        }

        let content_left = art_x + art_size + 9.0;
        let content_right = (p.left + p.width - 38.0).max(content_left + 40.0);
        let cy = p.top + p.height * 0.5;
        let right_cap_cx = p.left + p.width - 20.0;

        let text_primary = self.brushes.text_primary.clone();
        let text_secondary = self.brushes.text_secondary.clone();
        let accent = self.brushes.accent.clone();

        if input.media.has_session {
            let sub_label = if !input.media.artist.trim().is_empty() {
                input.media.artist.as_str()
            } else if !input.media.source_app.trim().is_empty() {
                input.media.source_app.as_str()
            } else if input.media.is_playing {
                "Đang phát"
            } else {
                "Đã tạm dừng"
            };
            unsafe {
                self.draw_text(
                    canvas,
                    &input.media.title,
                    TextFormatSlot::Compact,
                    &text_primary,
                    UiRectDip {
                        left: content_left,
                        top: p.top + 4.0,
                        right: content_right,
                        bottom: p.top + 20.0,
                    },
                );
                self.draw_text(
                    canvas,
                    sub_label,
                    TextFormatSlot::CompactSub,
                    &text_secondary,
                    UiRectDip {
                        left: content_left,
                        top: p.top + 19.0,
                        right: content_right,
                        bottom: p.top + 33.0,
                    },
                );
            }

            let tl = input.media.timeline;
            if tl.available && tl.duration_ticks() > 0 {
                let ratio = tl.progress_ratio(
                    current_windows_filetime_ticks(),
                    input.media.is_playing,
                );
                let bar_top = p.top + p.height - 5.4;
                let bar_bot = p.top + p.height - 3.4;
                let bg = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: content_left,
                        top: bar_top,
                        right: content_right,
                        bottom: bar_bot,
                    },
                    radiusX: 1.0,
                    radiusY: 1.0,
                };
                unsafe {
                    canvas
                        .ctx
                        .FillRoundedRectangle(&bg, &self.brushes.switch_off);
                }
                let track_w = (content_right - content_left).max(1.0);
                let fill_w = (track_w * ratio).clamp(0.0, track_w);
                if fill_w > 0.8 {
                    let fg = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: content_left,
                            top: bar_top,
                            right: (content_left + fill_w.max(2.0)).min(content_right),
                            bottom: bar_bot,
                        },
                        radiusX: 1.0,
                        radiusY: 1.0,
                    };
                    let fill_brush = if input.media.is_playing {
                        &self.brushes.accent
                    } else {
                        &self.brushes.text_secondary
                    };
                    unsafe {
                        canvas.ctx.FillRoundedRectangle(&fg, fill_brush);
                    }
                }
            }

            if input.config.visualizer_enabled && input.media.is_playing {
                unsafe {
                    self.draw_visualizer_bars(
                        canvas,
                        right_cap_cx - 9.1,
                        cy,
                        input.visualizer_phase,
                    );
                }
            } else if !input.media.is_playing {
                for dx in [-2.8_f32, 2.8_f32] {
                    let bar = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: right_cap_cx + dx - 1.2,
                            top: cy - 5.2,
                            right: right_cap_cx + dx + 1.2,
                            bottom: cy + 5.2,
                        },
                        radiusX: 1.2,
                        radiusY: 1.2,
                    };
                    unsafe {
                        canvas.ctx.FillRoundedRectangle(&bar, &text_secondary);
                    }
                }
            } else {
                unsafe {
                    self.draw_centered_triangle(
                        canvas,
                        &self.icons.play_triangle,
                        right_cap_cx + 0.5,
                        cy,
                        2.1,
                        &accent,
                    );
                }
            }
        } else {
            unsafe {
                self.draw_text(
                    canvas,
                    "Win Dynamic Island",
                    TextFormatSlot::Compact,
                    &text_primary,
                    UiRectDip {
                        left: content_left,
                        top: p.top + 5.0,
                        right: content_right,
                        bottom: p.top + 21.0,
                    },
                );
                self.draw_text(
                    canvas,
                    "Bấm để mở rộng hoặc cuộn chỉnh loa",
                    TextFormatSlot::CompactSub,
                    &text_secondary,
                    UiRectDip {
                        left: content_left,
                        top: p.top + 20.0,
                        right: content_right,
                        bottom: p.top + 34.0,
                    },
                );
            }
            let outer = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: right_cap_cx - 5.5,
                    top: cy - 5.5,
                    right: right_cap_cx + 5.5,
                    bottom: cy + 5.5,
                },
                radiusX: 5.5,
                radiusY: 5.5,
            };
            let inner = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: right_cap_cx - 2.6,
                    top: cy - 2.6,
                    right: right_cap_cx + 2.6,
                    bottom: cy + 2.6,
                },
                radiusX: 2.6,
                radiusY: 2.6,
            };
            unsafe {
                canvas
                    .ctx
                    .FillRoundedRectangle(&outer, &self.brushes.accent_soft);
                canvas.ctx.FillRoundedRectangle(&inner, &accent);
            }
        }
    }

    pub(super) unsafe fn draw_artwork_or_placeholder(
        &self,
        canvas: DrawCanvas<'_>,
        x: f32,
        y: f32,
        size: f32,
        radius: f32,
        is_playing: bool,
    ) {
        unsafe {
            self.draw_artwork_with_rim(canvas, x, y, size, radius, is_playing, false);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) unsafe fn draw_artwork_with_rim(
        &self,
        canvas: DrawCanvas<'_>,
        x: f32,
        y: f32,
        size: f32,
        radius: f32,
        is_playing: bool,
        soft_rim: bool,
    ) {
        let Some(art_brush) = &self.artwork_brush else {
            unsafe {
                self.draw_placeholder_art(canvas, x, y, size, radius, is_playing);
            }
            return;
        };

        let box_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: x,
                top: y,
                right: x + size,
                bottom: y + size,
            },
            radiusX: radius,
            radiusY: radius,
        };

        let (src_l, src_t, src_r, src_b) = match compute_center_crop_rect(self.artwork_size_px) {
            Some(r) => (r.left, r.top, r.right, r.bottom),
            None => (
                0.0,
                0.0,
                self.artwork_size_px.0.max(1) as f32,
                self.artwork_size_px.1.max(1) as f32,
            ),
        };
        let mat = bitmap_rect_transform(src_l, src_t, src_r, src_b, box_rect.rect);
        unsafe {
            art_brush.SetTransform(&mat);
            canvas.ctx.FillRoundedRectangle(&box_rect, art_brush);
            if soft_rim {
                canvas
                    .ctx
                    .DrawRoundedRectangle(&box_rect, &self.brushes.glass_inner_rim, 1.0, None);
            } else {
                canvas
                    .ctx
                    .DrawRoundedRectangle(&box_rect, &self.brushes.border, 1.0, None);
            }
        }
    }

    pub(super) unsafe fn draw_placeholder_art(
        &self,
        canvas: DrawCanvas<'_>,
        x: f32,
        y: f32,
        size: f32,
        radius: f32,
        is_playing: bool,
    ) {
        let box_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: x,
                top: y,
                right: x + size,
                bottom: y + size,
            },
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            canvas
                .ctx
                .FillRoundedRectangle(&box_rect, &self.brushes.art_bg);
            canvas
                .ctx
                .DrawRoundedRectangle(&box_rect, &self.brushes.border, 1.0, None);
        }

        let core_size = size * 0.34;
        let core_x = x + (size - core_size) * 0.5;
        let core_y = y + (size - core_size) * 0.5;
        let core = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: core_x,
                top: core_y,
                right: core_x + core_size,
                bottom: core_y + core_size,
            },
            radiusX: core_size * 0.5,
            radiusY: core_size * 0.5,
        };
        let brush = if is_playing {
            &self.brushes.accent
        } else {
            &self.brushes.text_secondary
        };
        unsafe {
            canvas.ctx.FillRoundedRectangle(&core, brush);
        }
    }

    pub(super) unsafe fn draw_visualizer_bars(
        &self,
        canvas: DrawCanvas<'_>,
        start_x: f32,
        center_y: f32,
        phase: f32,
    ) {
        unsafe {
            self.draw_visualizer_bars_count(
                canvas,
                start_x,
                center_y,
                phase,
                3.2,
                11.6,
                4.0,
                2.2,
                5,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) unsafe fn draw_visualizer_bars_count(
        &self,
        canvas: DrawCanvas<'_>,
        start_x: f32,
        center_y: f32,
        phase: f32,
        min_h: f32,
        amp_h: f32,
        step_x: f32,
        bar_w: f32,
        count: usize,
    ) {
        const ENVELOPE_5: [f32; 5] = [0.55, 0.88, 1.0, 0.88, 0.55];
        for (i, &env5) in ENVELOPE_5.iter().enumerate().take(count) {
            let env = if count == 5 { env5 } else { 1.0 };
            let wave = ((phase + i as f32 * 1.35).sin() * 0.5 + 0.5).clamp(0.22, 1.0);
            let h = min_h + wave * amp_h * env;
            let bx = start_x + i as f32 * step_x;
            let bar = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: bx,
                    top: center_y - h * 0.5,
                    right: bx + bar_w,
                    bottom: center_y + h * 0.5,
                },
                radiusX: bar_w * 0.5,
                radiusY: bar_w * 0.5,
            };
            unsafe {
                canvas.ctx.FillRoundedRectangle(&bar, &self.brushes.accent);
            }
        }
    }
}
