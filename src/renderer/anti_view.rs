use crate::anti::{AntiAgentState, AntiProgressSnapshot, AntiSource};

use super::*;

pub(super) fn anti_source_color(source: AntiSource) -> D2D1_COLOR_F {
    let (r, g, b) = match source {
        AntiSource::AgyCli => (0.28, 0.86, 0.96),
        AntiSource::AntiIde => (0.38, 0.68, 1.00),
        AntiSource::AntiApp => (0.74, 0.56, 1.00),
        AntiSource::Antigravity => (0.52, 0.76, 1.00),
    };
    D2D1_COLOR_F { r, g, b, a: 1.0 }
}

pub(super) fn anti_state_color(state: AntiAgentState) -> D2D1_COLOR_F {
    let (r, g, b) = match state {
        AntiAgentState::ReadingProject => (0.36, 0.84, 1.00),
        AntiAgentState::Analyzing => (0.64, 0.52, 1.00),
        AntiAgentState::Coding => (0.30, 0.66, 1.00),
        AntiAgentState::Testing => (0.20, 0.88, 0.72),
        AntiAgentState::Building => (1.00, 0.64, 0.28),
        AntiAgentState::WaitingApproval => (1.00, 0.82, 0.36),
        AntiAgentState::FixingError => (1.00, 0.48, 0.32),
        AntiAgentState::Error => (1.00, 0.38, 0.44),
        AntiAgentState::Canceled => (0.62, 0.67, 0.74),
        AntiAgentState::Done => (0.36, 0.90, 0.56),
    };
    D2D1_COLOR_F { r, g, b, a: 1.0 }
}

impl IslandRenderer {
    pub(super) unsafe fn draw_anti_spinner(
        &self,
        canvas: DrawCanvas<'_>,
        cx: f32,
        cy: f32,
        phase: f32,
        state: AntiAgentState,
        brush: &ID2D1SolidColorBrush,
    ) {
        let ring_bg = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - 12.5,
                top: cy - 12.5,
                right: cx + 12.5,
                bottom: cy + 12.5,
            },
            radiusX: 12.5,
            radiusY: 12.5,
        };
        unsafe {
            brush.SetOpacity(0.16);
            canvas.ctx.FillRoundedRectangle(&ring_bg, brush);
            brush.SetOpacity(0.32);
            canvas.ctx.DrawRoundedRectangle(&ring_bg, brush, 0.9, None);
        }

        if state.is_running() {
            const DOTS: usize = 6;
            let orbit_r = 6.5_f32;
            for idx in 0..DOTS {
                let frac = idx as f32 / DOTS as f32;
                let angle = phase - frac * std::f32::consts::TAU;
                let (sin_a, cos_a) = angle.sin_cos();
                let dot_x = cx + cos_a * orbit_r;
                let dot_y = cy + sin_a * orbit_r;
                let dot_r = 2.0 - frac * 1.05;
                let alpha = (1.0 - frac * 0.78).clamp(0.18, 1.0);
                let dot = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: dot_x - dot_r,
                        top: dot_y - dot_r,
                        right: dot_x + dot_r,
                        bottom: dot_y + dot_r,
                    },
                    radiusX: dot_r,
                    radiusY: dot_r,
                };
                unsafe {
                    brush.SetOpacity(alpha);
                    canvas.ctx.FillRoundedRectangle(&dot, brush);
                }
            }
            let center_dot = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: cx - 1.7,
                    top: cy - 1.7,
                    right: cx + 1.7,
                    bottom: cy + 1.7,
                },
                radiusX: 1.7,
                radiusY: 1.7,
            };
            unsafe {
                brush.SetOpacity(0.75);
                canvas.ctx.FillRoundedRectangle(&center_dot, brush);
                brush.SetOpacity(1.0);
            }
        } else {
            let halo = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: cx - 5.6,
                    top: cy - 5.6,
                    right: cx + 5.6,
                    bottom: cy + 5.6,
                },
                radiusX: 5.6,
                radiusY: 5.6,
            };
            let core = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: cx - 3.0,
                    top: cy - 3.0,
                    right: cx + 3.0,
                    bottom: cy + 3.0,
                },
                radiusX: 3.0,
                radiusY: 3.0,
            };
            unsafe {
                brush.SetOpacity(0.28);
                canvas.ctx.FillRoundedRectangle(&halo, brush);
                brush.SetOpacity(1.0);
                canvas.ctx.FillRoundedRectangle(&core, brush);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) unsafe fn draw_left_anti_surface(
        &mut self,
        canvas: DrawCanvas<'_>,
        left_rect: PillRectDip,
        snap: &AntiProgressSnapshot,
        anti_spinner_phase: f32,
        opacity: f32,
        has_backdrop: bool,
        hovered: bool,
    ) {
        let opacity = opacity.clamp(0.0, 1.0);
        if opacity <= 0.02 || left_rect.height < 2.0 {
            return;
        }
        let emphasis = match snap.state {
            AntiAgentState::WaitingApproval => Some(self.brushes.power_warn.clone()),
            AntiAgentState::Error => Some(self.brushes.danger_border.clone()),
            _ => None,
        };
        unsafe {
            self.draw_pill_shell(
                canvas,
                left_rect,
                opacity,
                has_backdrop,
                false,
                emphasis.as_ref(),
            );
        }
        if hovered {
            let hl = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: left_rect.left + 2.0,
                    top: left_rect.top + 2.0,
                    right: left_rect.left + left_rect.width - 2.0,
                    bottom: left_rect.top + left_rect.height - 2.0,
                },
                radiusX: (left_rect.corner_radius - 1.5).max(2.0),
                radiusY: (left_rect.corner_radius - 1.5).max(2.0),
            };
            unsafe {
                self.brushes.surface_hover.SetOpacity(opacity * 0.42);
                canvas
                    .ctx
                    .FillRoundedRectangle(&hl, &self.brushes.surface_hover);
            }
        }
        if opacity <= 0.08 {
            return;
        }
        unsafe {
            self.draw_anti_compact_body(canvas, left_rect, snap, anti_spinner_phase, opacity);
        }
    }

    pub(super) unsafe fn draw_compact_anti(
        &mut self,
        canvas: DrawCanvas<'_>,
        input: &RenderFrameInput<'_>,
        snap: &AntiProgressSnapshot,
    ) {
        let (cw, ch, _, _) = target_dimensions_for_mode(IslandMode::Compact);
        let p = virtual_pill_anchored(input.pill, cw, ch, true);
        unsafe {
            self.draw_anti_compact_body(canvas, p, snap, input.anti_spinner_phase, 1.0);
        }
    }

    unsafe fn draw_anti_compact_body(
        &mut self,
        canvas: DrawCanvas<'_>,
        p: PillRectDip,
        snap: &AntiProgressSnapshot,
        anti_spinner_phase: f32,
        alpha: f32,
    ) {
        let cy = p.top + p.height * 0.5;
        let icon_cx = p.left + 20.0;

        let anti_brush = self.brushes.anti_accent.clone();
        let text_primary = self.brushes.text_primary.clone();
        let text_secondary = self.brushes.text_secondary.clone();
        let source_brush = self.brushes.accent.clone();
        unsafe {
            anti_brush.SetColor(&anti_state_color(snap.state));
            anti_brush.SetOpacity(alpha);
            source_brush.SetColor(&anti_source_color(snap.source));
            source_brush.SetOpacity(alpha);
            text_primary.SetOpacity(alpha);
            text_secondary.SetOpacity(alpha);
            self.draw_anti_spinner(
                canvas,
                icon_cx,
                cy,
                anti_spinner_phase,
                snap.state,
                &anti_brush,
            );
        }

        let subtitle = if snap.detail.trim().is_empty() {
            if snap.state.is_running() {
                "Đang chạy tác vụ nền"
            } else {
                "Bấm để xem chi tiết hoặc mở cửa sổ"
            }
        } else {
            snap.detail.as_str()
        };

        let content_left = p.left + 39.0;
        let content_right = (p.left + p.width - 22.0).max(content_left + 72.0);
        let source_name = snap.source.display_name();
        let src_w = unsafe {
            self.measure_text_width(source_name, TextFormatSlot::Compact, 96.0, 16.0)
        }
        .unwrap_or(56.0)
        .clamp(38.0, 96.0);
        let state_text = format!("· {}", snap.state.label_vi());

        unsafe {
            self.draw_text(
                canvas,
                source_name,
                TextFormatSlot::Compact,
                &source_brush,
                UiRectDip {
                    left: content_left,
                    top: p.top + 3.5,
                    right: (content_left + src_w + 4.0).min(content_right),
                    bottom: p.top + 19.5,
                },
            );
            let state_left = (content_left + src_w + 5.0).min(content_right);
            if content_right - state_left > 24.0 {
                self.draw_text(
                    canvas,
                    &state_text,
                    TextFormatSlot::Compact,
                    &text_primary,
                    UiRectDip {
                        left: state_left,
                        top: p.top + 3.5,
                        right: content_right,
                        bottom: p.top + 19.5,
                    },
                );
            }
            self.draw_text(
                canvas,
                subtitle,
                TextFormatSlot::CompactSub,
                &text_secondary,
                UiRectDip {
                    left: content_left,
                    top: p.top + 18.0,
                    right: content_right,
                    bottom: p.top + 32.0,
                },
            );
            self.draw_anti_activity_bar(
                canvas,
                (content_left, content_right, p.top + p.height - 5.0),
                snap.state,
                anti_spinner_phase,
                &anti_brush,
            );

            let badge_cx = p.left + p.width - 13.0;
            let badge_dot = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: badge_cx - 2.6,
                    top: cy - 2.6,
                    right: badge_cx + 2.6,
                    bottom: cy + 2.6,
                },
                radiusX: 2.6,
                radiusY: 2.6,
            };
            anti_brush.SetOpacity(alpha * if snap.state.is_running() { 0.95 } else { 0.72 });
            canvas.ctx.FillRoundedRectangle(&badge_dot, &anti_brush);
            anti_brush.SetOpacity(1.0);
            text_primary.SetOpacity(1.0);
            text_secondary.SetOpacity(1.0);
        }
    }

    unsafe fn draw_anti_activity_bar(
        &self,
        canvas: DrawCanvas<'_>,
        bounds: (f32, f32, f32),
        state: AntiAgentState,
        phase: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let (left, right, cy) = bounds;
        let track_w = (right - left).max(8.0);
        let h = 2.0_f32;
        let bg = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left,
                top: cy - h * 0.5,
                right,
                bottom: cy + h * 0.5,
            },
            radiusX: 1.0,
            radiusY: 1.0,
        };
        unsafe {
            canvas.ctx.FillRoundedRectangle(&bg, &self.brushes.switch_off);
        }
        if state.is_running() && state != AntiAgentState::WaitingApproval {
            let seg_w = (track_w * 0.28).max(24.0);
            let travel = (track_w - seg_w).max(0.0);
            let t = 0.5 - 0.5 * phase.cos();
            let seg_left = left + travel * t;
            let fg = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: seg_left,
                    top: cy - h * 0.5,
                    right: (seg_left + seg_w).min(right),
                    bottom: cy + h * 0.5,
                },
                radiusX: 1.0,
                radiusY: 1.0,
            };
            unsafe {
                brush.SetOpacity(0.92);
                canvas.ctx.FillRoundedRectangle(&fg, brush);
                brush.SetOpacity(1.0);
            }
        } else {
            let fg = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left,
                    top: cy - h * 0.5,
                    right,
                    bottom: cy + h * 0.5,
                },
                radiusX: 1.0,
                radiusY: 1.0,
            };
            unsafe {
                brush.SetOpacity(if state == AntiAgentState::Done || state == AntiAgentState::Error || state == AntiAgentState::WaitingApproval {
                    0.85
                } else {
                    0.45
                });
                canvas.ctx.FillRoundedRectangle(&fg, brush);
                brush.SetOpacity(1.0);
            }
        }
    }

    pub(super) unsafe fn draw_anti_expanded(
        &mut self,
        canvas: DrawCanvas<'_>,
        input: &RenderFrameInput<'_>,
    ) {
        let p = virtual_pill_anchored(input.pill, 424.0, 154.0, false);
        let text_primary = self.brushes.text_primary.clone();
        let text_secondary = self.brushes.text_secondary.clone();
        let anti_brush = self.brushes.anti_accent.clone();
        let source_brush = self.brushes.accent.clone();

        if let Some(snap) = input.anti {
            unsafe {
                anti_brush.SetColor(&anti_state_color(snap.state));
                anti_brush.SetOpacity(1.0);
                source_brush.SetColor(&anti_source_color(snap.source));
                source_brush.SetOpacity(1.0);
                self.draw_anti_spinner(
                    canvas,
                    p.left + 29.0,
                    p.top + 28.0,
                    input.anti_spinner_phase,
                    snap.state,
                    &anti_brush,
                );
            }

            let source_name = snap.source.display_name();
            let src_w = unsafe {
                self.measure_text_width(source_name, TextFormatSlot::Title, 110.0, 22.0)
            }
            .unwrap_or(68.0)
            .clamp(42.0, 110.0);
            let title_left = p.left + 50.0;
            let title_right = p.left + p.width - 18.0;
            let state_text = format!("· {}", snap.state.label_vi());
            let detail_text = if snap.detail.trim().is_empty() {
                "Đang theo dõi phiên làm việc của Anti"
            } else {
                snap.detail.as_str()
            };

            let steps_line = if snap.recent_steps.len() > 1 {
                snap.recent_steps
                    .iter()
                    .skip(1)
                    .take(2)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("  |  ")
            } else {
                String::from("Bấm Mở cửa sổ để chuyển nhanh về ứng dụng")
            };

            unsafe {
                self.draw_text(
                    canvas,
                    source_name,
                    TextFormatSlot::Title,
                    &source_brush,
                    UiRectDip {
                        left: title_left,
                        top: p.top + 16.0,
                        right: (title_left + src_w + 4.0).min(title_right),
                        bottom: p.top + 38.0,
                    },
                );
                let st_left = (title_left + src_w + 6.0).min(title_right);
                if title_right - st_left > 30.0 {
                    self.draw_text(
                        canvas,
                        &state_text,
                        TextFormatSlot::Title,
                        &text_primary,
                        UiRectDip {
                            left: st_left,
                            top: p.top + 16.0,
                            right: title_right,
                            bottom: p.top + 38.0,
                        },
                    );
                }
                self.draw_text(
                    canvas,
                    detail_text,
                    TextFormatSlot::BodyLeft,
                    &text_primary,
                    UiRectDip {
                        left: title_left,
                        top: p.top + 41.0,
                        right: title_right,
                        bottom: p.top + 78.0,
                    },
                );
                self.draw_text(
                    canvas,
                    &steps_line,
                    TextFormatSlot::CompactSub,
                    &text_secondary,
                    UiRectDip {
                        left: title_left,
                        top: p.top + 82.0,
                        right: title_right,
                        bottom: p.top + 98.0,
                    },
                );
            }
        }

        fill_anti_expanded_hitboxes(p, &mut self.hitbox_scratch);
        let count = self.hitbox_scratch.len();
        for idx in 0..count {
            let hb = self.hitbox_scratch[idx];
            let hovered = input.hovered_action == Some(hb.action);
            let is_focus = hb.action == UiAction::FocusAntiWindow;
            let fill = if hovered {
                &self.brushes.surface_hover
            } else if is_focus {
                &self.brushes.sys_accent_soft
            } else {
                &self.brushes.surface_card
            };
            let border = if is_focus {
                &self.brushes.sys_accent
            } else {
                &self.brushes.card_border
            };
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
            let label = match hb.action {
                UiAction::FocusAntiWindow => "Mở cửa sổ",
                UiAction::DismissAnti => "Ẩn nhanh",
                UiAction::OpenSettings => "Cài đặt",
                _ => continue,
            };
            unsafe {
                canvas.ctx.FillRoundedRectangle(&card, fill);
                canvas.ctx.DrawRoundedRectangle(&card, border, 0.9, None);
                self.draw_text(
                    canvas,
                    label,
                    TextFormatSlot::ButtonCenter,
                    &text_primary,
                    hb.rect,
                );
            }
        }
    }
}
