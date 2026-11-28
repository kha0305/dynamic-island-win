use crate::anti::{AntiAgentState, AntiProgressSnapshot};

use super::*;

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

    pub(super) unsafe fn draw_compact_anti(
        &mut self,
        canvas: DrawCanvas<'_>,
        input: &RenderFrameInput<'_>,
        snap: &AntiProgressSnapshot,
    ) {
        let (cw, ch, _, _) = target_dimensions_for_mode(IslandMode::Compact);
        let p = virtual_pill_anchored(input.pill, cw, ch, true);
        let cy = p.top + p.height * 0.5;
        let icon_cx = p.left + 20.0;

        let anti_brush = self.brushes.anti_accent.clone();
        let text_primary = self.brushes.text_primary.clone();
        unsafe {
            anti_brush.SetColor(&anti_state_color(snap.state));
            anti_brush.SetOpacity(1.0);
            self.draw_anti_spinner(
                canvas,
                icon_cx,
                cy,
                input.anti_spinner_phase,
                snap.state,
                &anti_brush,
            );
        }

        let title = format!("{} · {}", snap.source.display_name(), snap.state.label_vi());
        let subtitle = if snap.detail.trim().is_empty() {
            if snap.state.is_running() {
                "Đang chạy tác vụ nền"
            } else {
                "Bấm để xem hoặc ẩn nhanh"
            }
        } else {
            snap.detail.as_str()
        };

        let content_left = p.left + 39.0;
        let content_right = (p.left + p.width - 26.0).max(content_left + 80.0);

        unsafe {
            self.draw_text(
                canvas,
                &title,
                TextFormatSlot::Compact,
                &text_primary,
                UiRectDip {
                    left: content_left,
                    top: p.top + 4.0,
                    right: content_right,
                    bottom: p.top + 20.0,
                },
            );
            anti_brush.SetOpacity(0.92);
            self.draw_text(
                canvas,
                subtitle,
                TextFormatSlot::CompactSub,
                &anti_brush,
                UiRectDip {
                    left: content_left,
                    top: p.top + 19.0,
                    right: content_right,
                    bottom: p.top + 33.0,
                },
            );

            let badge_cx = p.left + p.width - 15.0;
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
            anti_brush.SetOpacity(if snap.state.is_running() { 0.95 } else { 0.65 });
            canvas.ctx.FillRoundedRectangle(&badge_dot, &anti_brush);
            anti_brush.SetOpacity(1.0);
        }
    }
}
