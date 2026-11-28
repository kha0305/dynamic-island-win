use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::D2D1_ROUNDED_RECT;
use windows_numerics::Vector2;

use super::{
    DrawCanvas, IslandRenderer, MAX_SHELF_VISIBLE_ROWS, ShelfRenderInput, TextFormatSlot,
};
use crate::state::{PillRectDip, UiAction, UiHitbox, UiRectDip, fill_shelf_hitboxes};

impl IslandRenderer {
    pub(super) unsafe fn draw_shelf_surface(
        &mut self,
        canvas: DrawCanvas<'_>,
        shelf_rect: PillRectDip,
        shelf: ShelfRenderInput<'_>,
        opacity: f32,
        has_backdrop: bool,
        hovered_action: Option<UiAction>,
    ) {
        let opacity = opacity.clamp(0.0, 1.0);
        if opacity <= 0.02 || shelf_rect.height < 2.0 {
            return;
        }
        unsafe {
            self.draw_pill_shell(canvas, shelf_rect, opacity, has_backdrop, false);
        }

        if shelf.drag_hover {
            let glow_rect = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: shelf_rect.left + 1.0,
                    top: shelf_rect.top + 1.0,
                    right: shelf_rect.left + shelf_rect.width - 1.0,
                    bottom: shelf_rect.top + shelf_rect.height - 1.0,
                },
                radiusX: (shelf_rect.corner_radius - 0.5).max(2.0),
                radiusY: (shelf_rect.corner_radius - 0.5).max(2.0),
            };
            unsafe {
                self.brushes.sys_accent.SetOpacity(opacity * 0.92);
                canvas
                    .ctx
                    .DrawRoundedRectangle(&glow_rect, &self.brushes.sys_accent, 1.6, None);
            }
        }

        if opacity <= 0.08 {
            return;
        }

        if !shelf.open {
            unsafe {
                self.draw_shelf_dock(canvas, shelf_rect, shelf, opacity, hovered_action);
            }
        } else {
            unsafe {
                self.draw_shelf_panel(canvas, shelf_rect, shelf, opacity, hovered_action);
            }
        }
    }

    unsafe fn draw_shelf_dock(
        &mut self,
        canvas: DrawCanvas<'_>,
        rect: PillRectDip,
        shelf: ShelfRenderInput<'_>,
        alpha: f32,
        hovered_action: Option<UiAction>,
    ) {
        let hovered = hovered_action == Some(UiAction::ToggleShelf) || shelf.drag_hover;
        if hovered {
            let hl = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: rect.left + 2.0,
                    top: rect.top + 2.0,
                    right: rect.left + rect.width - 2.0,
                    bottom: rect.top + rect.height - 2.0,
                },
                radiusX: (rect.corner_radius - 1.5).max(2.0),
                radiusY: (rect.corner_radius - 1.5).max(2.0),
            };
            unsafe {
                self.brushes.surface_hover.SetOpacity(alpha * 0.62);
                canvas
                    .ctx
                    .FillRoundedRectangle(&hl, &self.brushes.surface_hover);
            }
        }

        let tray_x = rect.left + 12.0;
        let tray_y = rect.top + (rect.height - 10.0) * 0.5;
        let tray_box = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: tray_x,
                top: tray_y,
                right: tray_x + 12.0,
                bottom: tray_y + 10.0,
            },
            radiusX: 2.5,
            radiusY: 2.5,
        };
        let text_primary = self.brushes.text_primary.clone();
        unsafe {
            let icon_brush = if shelf.drag_hover || !shelf.items.is_empty() {
                &self.brushes.sys_accent
            } else {
                &self.brushes.text_secondary
            };
            icon_brush.SetOpacity(alpha * 0.95);
            canvas
                .ctx
                .DrawRoundedRectangle(&tray_box, icon_brush, 1.35, None);
            canvas.ctx.DrawLine(
                Vector2 {
                    X: tray_x + 2.6,
                    Y: tray_y + 5.2,
                },
                Vector2 {
                    X: tray_x + 9.4,
                    Y: tray_y + 5.2,
                },
                icon_brush,
                1.2,
                Some(&self.icons.round_stroke),
            );
        }

        let label = if shelf.items.is_empty() {
            String::from("Khay tệp")
        } else {
            format!("Khay ({})", shelf.items.len())
        };
        let label_rect = UiRectDip {
            left: tray_x + 15.0,
            top: rect.top,
            right: rect.left + rect.width - 8.0,
            bottom: rect.top + rect.height,
        };
        unsafe {
            text_primary.SetOpacity(alpha);
            self.draw_text(
                canvas,
                &label,
                TextFormatSlot::SmallLeft,
                &text_primary,
                label_rect,
            );
        }
    }

    unsafe fn draw_shelf_panel(
        &mut self,
        canvas: DrawCanvas<'_>,
        rect: PillRectDip,
        shelf: ShelfRenderInput<'_>,
        alpha: f32,
        hovered_action: Option<UiAction>,
    ) {
        let pad_x = 12.0;
        let inner_left = rect.left + pad_x;
        let inner_right = rect.left + rect.width - pad_x;
        let selected_count = shelf.items.iter().filter(|item| item.selected).count();

        let text_primary = self.brushes.text_primary.clone();
        let text_secondary = self.brushes.text_secondary.clone();
        let surface_card = self.brushes.surface_card.clone();
        let surface_hover = self.brushes.surface_hover.clone();
        let card_border = self.brushes.card_border.clone();
        let sys_accent = self.brushes.sys_accent.clone();
        let sys_accent_soft = self.brushes.sys_accent_soft.clone();
        let danger_bg = self.brushes.danger_bg.clone();
        let danger_hover = self.brushes.danger_hover.clone();
        let danger_border = self.brushes.danger_border.clone();

        let title = if shelf.items.is_empty() {
            String::from("Khay tệp")
        } else {
            format!("Khay tệp ({})", shelf.items.len())
        };
        unsafe {
            text_primary.SetOpacity(alpha);
            self.draw_text(
                canvas,
                &title,
                TextFormatSlot::Compact,
                &text_primary,
                UiRectDip {
                    left: inner_left,
                    top: rect.top + 8.0,
                    right: inner_right - 72.0,
                    bottom: rect.top + 30.0,
                },
            );
        }

        let mut hitboxes: Vec<UiHitbox> = Vec::with_capacity(14);
        fill_shelf_hitboxes(
            rect,
            true,
            shelf.items.len(),
            shelf.scroll,
            selected_count,
            &mut hitboxes,
        );

        let close_rect = hitboxes
            .iter()
            .find(|hb| hb.action == UiAction::ShelfClose)
            .map_or(
                UiRectDip {
                    left: inner_right - 66.0,
                    top: rect.top + 8.0,
                    right: inner_right,
                    bottom: rect.top + 30.0,
                },
                |hb| hb.rect,
            );
        let close_pill = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: close_rect.left,
                top: close_rect.top,
                right: close_rect.right,
                bottom: close_rect.bottom,
            },
            radiusX: 11.0,
            radiusY: 11.0,
        };
        unsafe {
            let bg = if hovered_action == Some(UiAction::ShelfClose) {
                &surface_hover
            } else {
                &surface_card
            };
            bg.SetOpacity(alpha * 0.9);
            canvas.ctx.FillRoundedRectangle(&close_pill, bg);
            card_border.SetOpacity(alpha * 0.72);
            canvas
                .ctx
                .DrawRoundedRectangle(&close_pill, &card_border, 0.9, None);
            text_primary.SetOpacity(alpha);
            self.draw_text(
                canvas,
                "Thu gọn",
                TextFormatSlot::Small,
                &text_primary,
                close_rect,
            );
        }

        if shelf.items.is_empty() {
            let drop_zone = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: inner_left,
                    top: rect.top + 38.0,
                    right: inner_right,
                    bottom: rect.top + rect.height - 12.0,
                },
                radiusX: 12.0,
                radiusY: 12.0,
            };
            unsafe {
                surface_card.SetOpacity(alpha * 0.7);
                canvas
                    .ctx
                    .FillRoundedRectangle(&drop_zone, &surface_card);
                let border_brush = if shelf.drag_hover {
                    &sys_accent
                } else {
                    &card_border
                };
                border_brush.SetOpacity(alpha * if shelf.drag_hover { 0.95 } else { 0.7 });
                canvas
                    .ctx
                    .DrawRoundedRectangle(&drop_zone, border_brush, 1.15, None);

                text_primary.SetOpacity(alpha * 0.95);
                self.draw_text(
                    canvas,
                    "Kéo thả tệp vào đây",
                    TextFormatSlot::Small,
                    &text_primary,
                    UiRectDip {
                        left: inner_left + 8.0,
                        top: rect.top + 52.0,
                        right: inner_right - 8.0,
                        bottom: rect.top + 72.0,
                    },
                );
                text_secondary.SetOpacity(alpha * 0.85);
                self.draw_text(
                    canvas,
                    "Nhận nhiều tệp hoặc thư mục",
                    TextFormatSlot::Small,
                    &text_secondary,
                    UiRectDip {
                        left: inner_left + 8.0,
                        top: rect.top + 73.0,
                        right: inner_right - 8.0,
                        bottom: rect.top + 92.0,
                    },
                );
            }
            return;
        }

        let select_all_rect = hitboxes
            .iter()
            .find(|hb| hb.action == UiAction::ShelfToggleSelectAll)
            .map_or(
                UiRectDip {
                    left: inner_left,
                    top: rect.top + 34.0,
                    right: inner_left + 126.0,
                    bottom: rect.top + 56.0,
                },
                |hb| hb.rect,
            );
        let all_selected = selected_count == shelf.items.len();
        unsafe {
            self.draw_shelf_checkbox(
                canvas,
                select_all_rect.left + 2.0,
                select_all_rect.top + 3.0,
                all_selected,
                hovered_action == Some(UiAction::ShelfToggleSelectAll),
                alpha,
            );
            text_secondary.SetOpacity(alpha * 0.96);
            self.draw_text(
                canvas,
                "Chọn tất cả",
                TextFormatSlot::SmallLeft,
                &text_secondary,
                UiRectDip {
                    left: select_all_rect.left + 24.0,
                    top: select_all_rect.top,
                    right: select_all_rect.right,
                    bottom: select_all_rect.bottom,
                },
            );
            let status_hint = if selected_count > 0 {
                format!("Đã chọn {selected_count}")
            } else {
                String::from("Kéo để gửi")
            };
            text_secondary.SetOpacity(alpha * 0.78);
            self.draw_text(
                canvas,
                &status_hint,
                TextFormatSlot::SmallRight,
                &text_secondary,
                UiRectDip {
                    left: select_all_rect.right + 4.0,
                    top: select_all_rect.top,
                    right: inner_right,
                    bottom: select_all_rect.bottom,
                },
            );
        }

        let visible = shelf
            .items
            .len()
            .saturating_sub(shelf.scroll)
            .min(MAX_SHELF_VISIBLE_ROWS);
        let list_top = rect.top + 60.0;
        let row_h = 32.0;
        let gap = 4.0;
        for vis_idx in 0..visible {
            let actual_idx = shelf.scroll + vis_idx;
            let item = &shelf.items[actual_idx];
            let top = list_top + vis_idx as f32 * (row_h + gap);
            let bottom = top + row_h;

            let row_hover = hovered_action == Some(UiAction::ShelfDragItem(actual_idx));
            let tick_hover = hovered_action == Some(UiAction::ShelfToggleSelect(actual_idx));
            let row_bg = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: inner_left,
                    top,
                    right: inner_right,
                    bottom,
                },
                radiusX: 10.0,
                radiusY: 10.0,
            };
            unsafe {
                let card_brush = if item.selected || row_hover {
                    &surface_hover
                } else {
                    &surface_card
                };
                card_brush.SetOpacity(alpha * if item.selected { 0.85 } else { 0.72 });
                canvas.ctx.FillRoundedRectangle(&row_bg, card_brush);

                let border_brush = if item.selected {
                    &sys_accent
                } else {
                    &card_border
                };
                border_brush.SetOpacity(alpha * if item.selected { 0.78 } else { 0.45 });
                canvas
                    .ctx
                    .DrawRoundedRectangle(&row_bg, border_brush, 0.9, None);

                self.draw_shelf_checkbox(
                    canvas,
                    inner_left + 4.0,
                    top + (row_h - 16.0) * 0.5,
                    item.selected,
                    tick_hover,
                    alpha,
                );

                let badge_rect = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: inner_left + 26.0,
                        top: top + 7.0,
                        right: inner_left + 60.0,
                        bottom: bottom - 7.0,
                    },
                    radiusX: 5.0,
                    radiusY: 5.0,
                };
                sys_accent_soft.SetOpacity(alpha * 0.72);
                canvas
                    .ctx
                    .FillRoundedRectangle(&badge_rect, &sys_accent_soft);
                text_primary.SetOpacity(alpha);
                self.draw_text(
                    canvas,
                    &item.ext_badge,
                    TextFormatSlot::Small,
                    &text_primary,
                    UiRectDip {
                        left: badge_rect.rect.left,
                        top: badge_rect.rect.top,
                        right: badge_rect.rect.right,
                        bottom: badge_rect.rect.bottom,
                    },
                );

                text_primary.SetOpacity(alpha);
                self.draw_text(
                    canvas,
                    &item.name,
                    TextFormatSlot::SmallLeft,
                    &text_primary,
                    UiRectDip {
                        left: inner_left + 66.0,
                        top,
                        right: inner_right - 56.0,
                        bottom,
                    },
                );

                text_secondary.SetOpacity(alpha * 0.88);
                self.draw_text(
                    canvas,
                    &item.size_label,
                    TextFormatSlot::SmallRight,
                    &text_secondary,
                    UiRectDip {
                        left: inner_right - 54.0,
                        top,
                        right: inner_right - 6.0,
                        bottom,
                    },
                );
            }
        }

        if let Some(delete_hb) = hitboxes
            .iter()
            .find(|hb| hb.action == UiAction::ShelfDeleteAll)
        {
            let del_hovered = hovered_action == Some(UiAction::ShelfDeleteAll);
            let del_rounded = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: delete_hb.rect.left,
                    top: delete_hb.rect.top,
                    right: delete_hb.rect.right,
                    bottom: delete_hb.rect.bottom,
                },
                radiusX: 11.0,
                radiusY: 11.0,
            };
            unsafe {
                let bg = if del_hovered {
                    &danger_hover
                } else {
                    &danger_bg
                };
                bg.SetOpacity(alpha);
                canvas.ctx.FillRoundedRectangle(&del_rounded, bg);
                danger_border.SetOpacity(alpha);
                canvas
                    .ctx
                    .DrawRoundedRectangle(&del_rounded, &danger_border, 1.0, None);
                text_primary.SetOpacity(alpha);
                self.draw_text(
                    canvas,
                    "Delete All",
                    TextFormatSlot::ButtonCenter,
                    &text_primary,
                    delete_hb.rect,
                );
            }
        } else {
            let footer_top =
                list_top + visible as f32 * row_h + visible.saturating_sub(1) as f32 * gap + 8.0;
            unsafe {
                text_secondary.SetOpacity(alpha * 0.72);
                self.draw_text(
                    canvas,
                    "Tick chọn để hiện Delete All · Kéo để gửi",
                    TextFormatSlot::Small,
                    &text_secondary,
                    UiRectDip {
                        left: inner_left,
                        top: footer_top,
                        right: inner_right,
                        bottom: rect.top + rect.height - 6.0,
                    },
                );
            }
        }
    }

    unsafe fn draw_shelf_checkbox(
        &self,
        canvas: DrawCanvas<'_>,
        left: f32,
        top: f32,
        checked: bool,
        hovered: bool,
        alpha: f32,
    ) {
        let box_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left,
                top,
                right: left + 16.0,
                bottom: top + 16.0,
            },
            radiusX: 4.5,
            radiusY: 4.5,
        };
        unsafe {
            if checked {
                self.brushes.sys_accent.SetOpacity(alpha);
                canvas
                    .ctx
                    .FillRoundedRectangle(&box_rect, &self.brushes.sys_accent);
                self.brushes.knob.SetColor(&D2D1_COLOR_F {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: alpha,
                });
                canvas.ctx.DrawLine(
                    Vector2 {
                        X: left + 3.8,
                        Y: top + 8.2,
                    },
                    Vector2 {
                        X: left + 6.8,
                        Y: top + 11.4,
                    },
                    &self.brushes.knob,
                    1.7,
                    Some(&self.icons.round_stroke),
                );
                canvas.ctx.DrawLine(
                    Vector2 {
                        X: left + 6.8,
                        Y: top + 11.4,
                    },
                    Vector2 {
                        X: left + 12.2,
                        Y: top + 4.8,
                    },
                    &self.brushes.knob,
                    1.7,
                    Some(&self.icons.round_stroke),
                );
            } else {
                let fill = if hovered {
                    &self.brushes.surface_hover
                } else {
                    &self.brushes.switch_off
                };
                fill.SetOpacity(alpha * 0.9);
                canvas.ctx.FillRoundedRectangle(&box_rect, fill);
                self.brushes.card_border.SetOpacity(alpha * 0.85);
                canvas
                    .ctx
                    .DrawRoundedRectangle(&box_rect, &self.brushes.card_border, 1.0, None);
            }
        }
    }
}
