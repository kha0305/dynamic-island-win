use super::*;

impl WindowController {
    pub(super) fn reset_pointer_and_drag_state(&mut self) -> bool {
        let was_capturing = self.dragging_timeline
            || self.dragging_volume
            || self.shelf_drag_candidate.is_some();
        self.cancel_shelf_drag_leave();
        self.shelf.drag_hover = false;
        self.shelf_auto_opened_by_drag = false;
        self.mouse_tracked = false;
        self.hovered_action = None;
        self.hovered_header = false;
        self.pressed_action = None;
        self.pointer_down_pos = None;
        self.dragging_timeline = false;
        self.timeline_drag_ratio = None;
        self.dragging_volume = false;
        self.volume_drag_ratio = None;
        self.shelf_drag_candidate = None;
        self.drag_render_dirty = false;
        self.wheel_accum.reset();
        was_capturing
    }

    pub(super) fn is_cursor_actually_over_pill(&self) -> bool {
        if !unsafe { IsWindowVisible(self.hwnd) }.as_bool() {
            return false;
        }
        let mut pt = POINT::default();
        if unsafe { GetCursorPos(&mut pt) }.is_err() {
            return false;
        }
        let hit_hwnd = unsafe { WindowFromPoint(pt) };
        if hit_hwnd != self.hwnd {
            return false;
        }
        if !unsafe { ScreenToClient(self.hwnd, &mut pt) }.as_bool() {
            return false;
        }
        let scale = self.dpi_scale.max(0.5);
        let x_dip = pt.x as f32 / scale;
        let y_dip = pt.y as f32 / scale;
        let pill = self.animator.current_pill();
        pill_contains_dip(pill, x_dip, y_dip, 2.0)
            || self
                .active_shelf_rect(pill)
                .is_some_and(|sr| shelf_contains_dip(sr, x_dip, y_dip, 2.0))
            || self
                .active_drag_bridge_rect(pill)
                .is_some_and(|br| shelf_contains_dip(br, x_dip, y_dip, 1.0))
    }

    pub(super) fn sync_pointer_presence(&mut self) {
        if self.mouse_tracked
            && !self.dragging_timeline
            && !self.dragging_volume
            && self.shelf_drag_candidate.is_none()
            && !self.is_cursor_actually_over_pill()
        {
            self.handle_mouse_leave();
        }
    }

    pub(super) fn client_dip_from_lparam(&self, lparam: LPARAM) -> (f32, f32) {
        let px_x = (lparam.0 & 0xFFFF) as i16 as f32;
        let px_y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
        let scale = self.dpi_scale.max(0.5);
        (px_x / scale, px_y / scale)
    }

    pub(super) fn handle_nchittest(&self, lparam: LPARAM) -> isize {
        if self.animator.should_hide_window() {
            return HTTRANSPARENT as isize;
        }
        let mut pt = POINT {
            x: (lparam.0 & 0xFFFF) as i16 as i32,
            y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
        };
        if !unsafe { ScreenToClient(self.hwnd, &mut pt) }.as_bool() {
            return HTTRANSPARENT as isize;
        }
        let scale = self.dpi_scale.max(0.5);
        let x_dip = pt.x as f32 / scale;
        let y_dip = pt.y as f32 / scale;
        let pill = self.animator.current_pill();
        let over_pill = pill_contains_dip(pill, x_dip, y_dip, 2.0);
        let over_shelf = self
            .active_shelf_rect(pill)
            .is_some_and(|sr| shelf_contains_dip(sr, x_dip, y_dip, 2.0));
        let over_bridge = self
            .active_drag_bridge_rect(pill)
            .is_some_and(|br| shelf_contains_dip(br, x_dip, y_dip, 1.0));
        if over_pill || over_shelf || over_bridge {
            HTCLIENT as isize
        } else {
            HTTRANSPARENT as isize
        }
    }

    pub(super) fn is_pointer_over_collapsible_header(&self, x_dip: f32, y_dip: f32) -> bool {
        let pill = self.animator.current_pill();
        if self.shelf.open
            && self.active_shelf_rect(pill).is_some_and(|sr| {
                should_collapse_shelf_on_header_click(sr, (x_dip, y_dip), (x_dip, y_dip))
            })
        {
            return true;
        }
        matches!(
            self.requested_mode,
            IslandMode::Settings | IslandMode::AudioDevices
        ) && should_close_subview_on_header_click(pill, (x_dip, y_dip), (x_dip, y_dip))
    }

    pub(super) fn hit_test_action(&mut self, x_dip: f32, y_dip: f32) -> Option<UiAction> {
        let pill = self.animator.current_pill();
        if let Some(s_rect) = self.active_shelf_rect(pill)
            && shelf_contains_dip(s_rect, x_dip, y_dip, 1.0)
        {
            fill_shelf_hitboxes(
                s_rect,
                self.shelf.open,
                self.shelf.items.len(),
                self.shelf.scroll,
                self.shelf.selected_count(),
                &mut self.hitbox_scratch,
            );
            return self
                .hitbox_scratch
                .iter()
                .find(|hb| hb.rect.contains(x_dip, y_dip))
                .map(|hb| hb.action);
        }

        let mode = self.current_effective_mode();
        if mode == IslandMode::Compact && matches!(self.active_hud, Some(HudContent::Volume { .. }))
        {
            fill_volume_hud_hitboxes(pill, &mut self.hitbox_scratch);
        } else if mode == IslandMode::AudioDevices {
            let visible = self
                .audio_devices
                .len()
                .saturating_sub(self.audio_device_scroll)
                .min(MAX_AUDIO_DEVICES_VISIBLE);
            fill_audio_devices_hitboxes(pill, visible, &mut self.hitbox_scratch);
        } else if mode == IslandMode::Notice {
            let has_secondary = self
                .active_notice
                .as_ref()
                .is_some_and(|n| n.secondary_label.is_some());
            fill_notice_hitboxes(pill, has_secondary, &mut self.hitbox_scratch);
        } else {
            fill_interactive_hitboxes(pill, mode, &mut self.hitbox_scratch);
        }
        self.hitbox_scratch
            .iter()
            .find(|hb| hb.rect.contains(x_dip, y_dip))
            .map(|hb| hb.action)
    }

    pub(super) fn current_volume_track_rect(&mut self) -> Option<UiRectDip> {
        if !matches!(self.active_hud, Some(HudContent::Volume { .. }))
            || self.current_effective_mode() != IslandMode::Compact
        {
            return None;
        }
        let pill = self.animator.current_pill();
        fill_volume_hud_hitboxes(pill, &mut self.hitbox_scratch);
        self.hitbox_scratch
            .iter()
            .find(|hb| hb.action == UiAction::SeekVolume)
            .map(|hb| hb.rect)
    }

    pub(super) fn handle_mouse_wheel(&mut self, wparam: WPARAM, lparam: LPARAM) -> bool {
        let mods = wparam.0 & 0xFFFF;
        if (mods & (MK_CONTROL_MASK | MK_SHIFT_MASK | MK_LBUTTON_MASK | MK_RBUTTON_MASK)) != 0 {
            return false;
        }

        let mut pt = POINT {
            x: (lparam.0 & 0xFFFF) as i16 as i32,
            y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
        };
        if !unsafe { ScreenToClient(self.hwnd, &mut pt) }.as_bool() {
            return false;
        }
        let scale = self.dpi_scale.max(0.5);
        let x_dip = pt.x as f32 / scale;
        let y_dip = pt.y as f32 / scale;
        let pill = self.animator.current_pill();
        let delta = ((wparam.0 >> 16) & 0xFFFF) as i16;
        if let Some(s_rect) = self.active_shelf_rect(pill)
            && shelf_contains_dip(s_rect, x_dip, y_dip, 2.0)
        {
            if self.shelf.open {
                let max_scroll = self.shelf.items.len().saturating_sub(MAX_SHELF_VISIBLE_ROWS);
                if max_scroll > 0 && delta != 0 {
                    let next_scroll = if delta < 0 {
                        (self.shelf.scroll + 1).min(max_scroll)
                    } else {
                        self.shelf.scroll.saturating_sub(1)
                    };
                    if next_scroll != self.shelf.scroll {
                        self.shelf.scroll = next_scroll;
                        self.hovered_action = self.hit_test_action(x_dip, y_dip);
                        if !self.animator.is_animating() {
                            self.render_current_frame();
                        }
                    }
                }
            }
            return true;
        }
        if !pill_contains_dip(pill, x_dip, y_dip, 2.0) {
            return false;
        }

        let mode = self.current_effective_mode();
        if mode == IslandMode::AudioDevices {
            self.ensure_mouse_tracked();
            let max_scroll = self
                .audio_devices
                .len()
                .saturating_sub(MAX_AUDIO_DEVICES_VISIBLE);
            if max_scroll > 0 && delta != 0 {
                let next_scroll = if delta < 0 {
                    (self.audio_device_scroll + 1).min(max_scroll)
                } else {
                    self.audio_device_scroll.saturating_sub(1)
                };
                if next_scroll != self.audio_device_scroll {
                    self.audio_device_scroll = next_scroll;
                    self.hovered_action = self.hit_test_action(x_dip, y_dip);
                    if !self.animator.is_animating() {
                        self.render_current_frame();
                    }
                }
            }
            return true;
        }

        if !self.config.hud_volume_enabled
            || !self.config.hud_wheel_enabled
            || !self.volume_snapshot.available
        {
            return false;
        }
        if !matches!(mode, IslandMode::Slim | IslandMode::Compact)
            || matches!(
                self.requested_mode,
                IslandMode::Expanded
                    | IslandMode::Settings
                    | IslandMode::AudioDevices
                    | IslandMode::Notice
            )
        {
            return false;
        }

        self.ensure_mouse_tracked();
        let steps = self.wheel_accum.push_delta(delta);
        if steps != 0 {
            let next = apply_volume_step(self.volume_snapshot.level, steps);
            if steps > 0 && self.volume_snapshot.muted {
                self.volume_snapshot.muted = false;
            }
            self.volume_snapshot.level = next;
            self.volume_hold_until =
                Some(Instant::now() + Duration::from_millis(VOLUME_ECHO_HOLD_MS));
            if let Some(mon) = &self.audio_monitor {
                mon.send_command(AudioCommand::StepVolume(steps));
            }
            self.show_hud(HudContent::Volume {
                level: self.volume_snapshot.level,
                muted: self.volume_snapshot.muted,
                show_device_name: false,
            });
        } else if self.active_hud.is_some() {
            self.schedule_hud_expire();
        }
        true
    }

    pub(super) fn ensure_mouse_tracked(&mut self) {
        if !self.mouse_tracked {
            let mut tme = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: self.hwnd,
                dwHoverTime: 0,
            };
            if unsafe { TrackMouseEvent(&mut tme) }.is_ok() {
                self.mouse_tracked = true;
            }
        }
    }

    pub(super) fn current_timeline_track_rect(&mut self) -> Option<UiRectDip> {
        if !self.media_state.has_session || !self.media_state.timeline.available {
            return None;
        }
        let pill = self.animator.current_pill();
        let mode = self.current_effective_mode();
        fill_interactive_hitboxes(pill, mode, &mut self.hitbox_scratch);
        self.hitbox_scratch
            .iter()
            .find(|hb| hb.action == UiAction::SeekTimeline)
            .map(|hb| hb.rect)
    }

    pub(super) fn handle_left_button_down(&mut self, lparam: LPARAM) -> CaptureChange {
        let (x_dip, y_dip) = self.client_dip_from_lparam(lparam);
        let pill = self.animator.current_pill();
        let over_pill = pill_contains_dip(pill, x_dip, y_dip, 2.0);
        let over_shelf = self
            .active_shelf_rect(pill)
            .is_some_and(|sr| shelf_contains_dip(sr, x_dip, y_dip, 2.0));
        if !over_pill && !over_shelf {
            return CaptureChange::None;
        }
        self.ensure_mouse_tracked();
        self.cancel_auto_collapse();
        self.cancel_compact_to_notch();
        self.cancel_hud_expire();
        self.pointer_down_pos = Some((x_dip, y_dip));
        self.pressed_action = None;

        if over_shelf {
            let hit = self.hit_test_action(x_dip, y_dip);
            self.pressed_action = hit;
            if let Some(UiAction::ShelfDragItem(idx)) = hit {
                self.shelf_drag_candidate = Some((idx, x_dip, y_dip));
                return CaptureChange::Acquire;
            }
            return CaptureChange::None;
        }

        if let Some(track) = self.current_volume_track_rect()
            && volume_seek_hit_bounds(track).contains(x_dip, y_dip)
        {
            let ratio = timeline_ratio_for_x(track, x_dip);
            self.dragging_volume = true;
            self.volume_drag_ratio = Some(ratio);
            self.drag_render_dirty = false;
            self.hovered_action = Some(UiAction::SeekVolume);
            self.apply_local_volume(ratio, ratio > 0.004);
            unsafe {
                SetCursor(Some(self.cursor_hand));
            }
            self.update_vsync_activity();
            self.render_current_frame();
            return CaptureChange::Acquire;
        }

        if self.current_effective_mode() == IslandMode::Expanded
            && let Some(track) = self.current_timeline_track_rect()
            && timeline_seek_hit_bounds(track).contains(x_dip, y_dip)
        {
            let ratio = timeline_ratio_for_x(track, x_dip);
            self.dragging_timeline = true;
            self.timeline_drag_ratio = Some(ratio);
            self.drag_render_dirty = false;
            self.hovered_action = Some(UiAction::SeekTimeline);
            unsafe {
                SetCursor(Some(self.cursor_hand));
            }
            self.update_vsync_activity();
            self.render_current_frame();
            return CaptureChange::Acquire;
        }

        self.pressed_action = self.hit_test_action(x_dip, y_dip);
        CaptureChange::None
    }

    pub(super) fn handle_mouse_move(&mut self, lparam: LPARAM) -> Option<Vec<PathBuf>> {
        self.cancel_auto_collapse();
        self.cancel_compact_to_notch();
        self.cancel_notice_expire();
        let (x_dip, y_dip) = self.client_dip_from_lparam(lparam);

        if let Some((idx, down_x, down_y)) = self.shelf_drag_candidate {
            let scale = self.dpi_scale.max(0.5);
            let min_dx = (unsafe { GetSystemMetrics(SM_CXDRAG) }.max(4) as f32) / scale;
            let min_dy = (unsafe { GetSystemMetrics(SM_CYDRAG) }.max(4) as f32) / scale;
            if (x_dip - down_x).abs() >= min_dx || (y_dip - down_y).abs() >= min_dy {
                self.shelf_drag_candidate = None;
                self.pressed_action = None;
                self.pointer_down_pos = None;
                self.mouse_tracked = false;
                self.hovered_action = None;
                let paths = self.shelf.drag_paths_for(idx);
                if !paths.is_empty() {
                    return Some(paths);
                }
                self.render_current_frame();
            }
            return None;
        }

        if self.dragging_volume {
            self.cancel_hud_expire();
            if let Some(track) = self.current_volume_track_rect() {
                let ratio = timeline_ratio_for_x(track, x_dip);
                let changed = self.volume_drag_ratio.is_none_or(|prev| {
                    (prev - ratio).abs() >= 0.004 || ratio == 0.0 || ratio == 1.0
                });
                if changed {
                    self.volume_drag_ratio = Some(ratio);
                    self.apply_local_volume(ratio, ratio > 0.004);
                    self.hovered_action = Some(UiAction::SeekVolume);
                    unsafe {
                        SetCursor(Some(self.cursor_hand));
                    }
                    self.request_smooth_render();
                }
            }
            return None;
        }

        if self.dragging_timeline {
            if let Some(track) = self.current_timeline_track_rect() {
                let ratio = timeline_ratio_for_x(track, x_dip);
                if self.timeline_drag_ratio != Some(ratio) {
                    self.timeline_drag_ratio = Some(ratio);
                    self.hovered_action = Some(UiAction::SeekTimeline);
                    unsafe {
                        SetCursor(Some(self.cursor_hand));
                    }
                    self.request_smooth_render();
                }
            }
            return None;
        }

        let pill = self.animator.current_pill();
        let over_shelf = self
            .active_shelf_rect(pill)
            .is_some_and(|sr| shelf_contains_dip(sr, x_dip, y_dip, 2.0));
        if self.requested_mode == IslandMode::Slim
            && !pill_contains_dip(pill, x_dip, y_dip, 2.0)
            && !over_shelf
        {
            return None;
        }

        self.ensure_mouse_tracked();
        if self.active_hud.is_none()
            && self.requested_mode == IslandMode::Slim
            && pill_contains_dip(pill, x_dip, y_dip, 2.0)
        {
            self.requested_mode = IslandMode::Compact;
            self.apply_target_and_render();
        }

        let next_hover = self.hit_test_action(x_dip, y_dip);
        let next_header_hover =
            next_hover.is_none() && self.is_pointer_over_collapsible_header(x_dip, y_dip);
        if next_hover != self.hovered_action || next_header_hover != self.hovered_header {
            let action_changed = next_hover != self.hovered_action;
            self.hovered_action = next_hover;
            self.hovered_header = next_header_hover;
            if matches!(
                next_hover,
                Some(UiAction::ToggleVolumeMute | UiAction::SeekVolume)
            ) {
                self.schedule_hud_expire();
            }
            let cursor = if next_hover.is_some() || next_header_hover {
                self.cursor_hand
            } else {
                self.cursor_arrow
            };
            unsafe {
                SetCursor(Some(cursor));
            }
            if action_changed && !self.animator.is_animating() {
                self.render_current_frame();
            }
        }
        None
    }

    pub(super) fn finish_shelf_drag_out(&mut self) {
        let pointer_inside = self.is_cursor_actually_over_pill();
        self.finish_shelf_drag_out_with_presence(pointer_inside);
    }

    pub(super) fn finish_shelf_drag_out_with_presence(&mut self, pointer_inside: bool) {
        self.shelf_drag_candidate = None;
        self.pressed_action = None;
        self.pointer_down_pos = None;
        self.hovered_header = false;
        if pointer_inside {
            self.mouse_tracked = false;
            self.hovered_action = None;
            self.render_current_frame();
        } else {
            let had_hover = self.hovered_action.is_some();
            self.handle_mouse_leave();
            if !had_hover {
                self.render_current_frame();
            }
        }
    }

    pub(super) fn handle_mouse_leave(&mut self) {
        if self.dragging_timeline || self.dragging_volume || self.shelf_drag_candidate.is_some() {
            return;
        }
        self.mouse_tracked = false;
        self.wheel_accum.reset();
        self.hovered_header = false;
        let hover_changed = self.hovered_action.take().is_some();
        if self.active_hud.is_some() {
            self.schedule_hud_expire();
            if hover_changed && !self.animator.is_animating() {
                self.render_current_frame();
            }
            return;
        }
        if self.requested_mode == IslandMode::Compact {
            if !self.media_state.has_session {
                if self.should_collapse_to_slim() {
                    self.cancel_compact_to_notch();
                    self.requested_mode = IslandMode::Slim;
                    self.apply_target_and_render();
                } else if hover_changed && !self.animator.is_animating() {
                    self.render_current_frame();
                }
            } else {
                if self.config.notch_collapse_after_4s {
                    self.schedule_compact_to_notch();
                }
                if hover_changed && !self.animator.is_animating() {
                    self.render_current_frame();
                }
            }
        } else if self.requested_mode == IslandMode::Expanded {
            self.schedule_auto_collapse(AUTO_COLLAPSE_DELAY_MS);
            if hover_changed && !self.animator.is_animating() {
                self.render_current_frame();
            }
        } else if self.requested_mode == IslandMode::Notice {
            if self.active_notice.as_ref().is_some_and(|n| n.auto_expire) {
                self.schedule_notice_expire();
            }
            if hover_changed && !self.animator.is_animating() {
                self.render_current_frame();
            }
        } else if hover_changed && !self.animator.is_animating() {
            self.render_current_frame();
        }
    }

    pub(super) fn handle_capture_changed(&mut self) {
        if !self.dragging_timeline && !self.dragging_volume && self.shelf_drag_candidate.is_none() {
            return;
        }
        let was_hud = self.active_hud.is_some();
        self.dragging_timeline = false;
        self.timeline_drag_ratio = None;
        self.dragging_volume = false;
        self.volume_drag_ratio = None;
        self.shelf_drag_candidate = None;
        self.drag_render_dirty = false;
        self.pointer_down_pos = None;
        self.pressed_action = None;
        self.mouse_tracked = false;
        self.hovered_action = None;
        if was_hud {
            self.schedule_hud_expire();
        } else if self.requested_mode == IslandMode::Expanded {
            self.schedule_auto_collapse(AUTO_COLLAPSE_DELAY_MS);
        }
        self.update_vsync_activity();
        self.render_current_frame();
    }

    pub(super) fn handle_left_button_up(&mut self, lparam: LPARAM) -> (CaptureChange, bool) {
        let (x_dip, y_dip) = self.client_dip_from_lparam(lparam);
        let down_pos = self.pointer_down_pos.take();
        let down_action = self.pressed_action.take();

        if self.shelf_drag_candidate.take().is_some() {
            let inside = self.is_cursor_actually_over_pill();
            self.mouse_tracked = false;
            self.hovered_action = if inside {
                self.hit_test_action(x_dip, y_dip)
            } else {
                None
            };
            return (CaptureChange::Release, inside);
        }

        if self.dragging_volume {
            self.dragging_volume = false;
            self.drag_render_dirty = false;
            let fallback_ratio = self.volume_drag_ratio.take().unwrap_or(self.volume_snapshot.level);
            let ratio = self
                .current_volume_track_rect()
                .map_or(fallback_ratio, |track| timeline_ratio_for_x(track, x_dip));
            self.apply_local_volume(ratio, ratio > 0.004);

            let pill = self.animator.current_pill();
            let inside_pill = pill_contains_dip(pill, x_dip, y_dip, 0.0);
            self.mouse_tracked = false;
            if inside_pill {
                self.hovered_action = self.hit_test_action(x_dip, y_dip);
            } else {
                self.hovered_action = None;
            }
            self.schedule_hud_expire();
            self.update_vsync_activity();
            self.render_current_frame();
            return (CaptureChange::Release, inside_pill);
        }

        if self.dragging_timeline {
            self.dragging_timeline = false;
            self.drag_render_dirty = false;
            let fallback_ratio = self.timeline_drag_ratio.take().unwrap_or(0.0);
            let ratio = self
                .current_timeline_track_rect()
                .map_or(fallback_ratio, |track| timeline_ratio_for_x(track, x_dip));
            self.commit_timeline_seek_ratio(ratio);

            let pill = self.animator.current_pill();
            let inside_pill = pill_contains_dip(pill, x_dip, y_dip, 0.0);
            self.mouse_tracked = false;
            if inside_pill {
                self.hovered_action = self.hit_test_action(x_dip, y_dip);
                self.cancel_auto_collapse();
            } else {
                self.hovered_action = None;
                if self.requested_mode == IslandMode::Expanded {
                    self.schedule_auto_collapse(AUTO_COLLAPSE_DELAY_MS);
                }
            }
            self.update_vsync_activity();
            self.render_current_frame();
            return (CaptureChange::Release, inside_pill);
        }

        let up_action = self.hit_test_action(x_dip, y_dip);
        if let Some(pressed) = down_action {
            if up_action == Some(pressed) {
                if pressed == UiAction::SeekTimeline {
                    self.handle_timeline_seek(x_dip);
                    return (CaptureChange::None, false);
                }
                if pressed == UiAction::SeekVolume {
                    if let Some(track) = self.current_volume_track_rect() {
                        let ratio = timeline_ratio_for_x(track, x_dip);
                        self.apply_local_volume(ratio, ratio > 0.004);
                        self.render_current_frame();
                    }
                    return (CaptureChange::None, false);
                }
                self.execute_ui_action(pressed);
            }
            // Nếu bấm vào một nút nhưng nhả chuột trượt ra ngoài nút -> bỏ qua, không thu gọn nhầm.
            return (CaptureChange::None, false);
        }

        if let Some(action) = up_action {
            if action == UiAction::SeekTimeline {
                self.handle_timeline_seek(x_dip);
                return (CaptureChange::None, false);
            }
            if action == UiAction::SeekVolume {
                if let Some(track) = self.current_volume_track_rect() {
                    let ratio = timeline_ratio_for_x(track, x_dip);
                    self.apply_local_volume(ratio, ratio > 0.004);
                    self.render_current_frame();
                }
                return (CaptureChange::None, false);
            }
            self.execute_ui_action(action);
            return (CaptureChange::None, false);
        }

        let pill = self.animator.current_pill();
        let click_down = down_pos.unwrap_or((x_dip, y_dip));
        if self.shelf.open
            && let Some(s_rect) = self.active_shelf_rect(pill)
            && shelf_contains_dip(s_rect, x_dip, y_dip, 2.0)
        {
            if should_collapse_shelf_on_header_click(s_rect, click_down, (x_dip, y_dip)) {
                self.shelf_auto_opened_by_drag = false;
                self.shelf.open = false;
                self.hovered_action = None;
                self.hovered_header = false;
                self.apply_target_and_render();
            }
            return (CaptureChange::None, false);
        }

        if !pill_contains_dip(pill, x_dip, y_dip, 2.0) {
            return (CaptureChange::None, false);
        }

        if self.active_hud.is_some() {
            self.clear_hud();
            return (CaptureChange::None, false);
        }

        if matches!(
            self.requested_mode,
            IslandMode::Settings | IslandMode::AudioDevices
        ) {
            if should_close_subview_on_header_click(pill, click_down, (x_dip, y_dip)) {
                self.hovered_header = false;
                self.execute_ui_action(UiAction::BackToMedia);
            }
            return (CaptureChange::None, false);
        }

        if self.requested_mode == IslandMode::Notice {
            return (CaptureChange::None, false);
        }

        if self.requested_mode == IslandMode::Expanded
            && !should_collapse_expanded_on_blank_click(pill, click_down, (x_dip, y_dip))
        {
            return (CaptureChange::None, false);
        }

        self.requested_mode = match self.requested_mode {
            IslandMode::Slim | IslandMode::Compact => {
                self.cancel_compact_to_notch();
                IslandMode::Expanded
            }
            IslandMode::Expanded => {
                self.cancel_auto_collapse();
                self.collapsed_rest_mode()
            }
            IslandMode::Settings => IslandMode::Settings,
            IslandMode::AudioDevices => IslandMode::AudioDevices,
            IslandMode::Notice => IslandMode::Notice,
            IslandMode::Hidden => IslandMode::Slim,
        };
        if self.requested_mode == IslandMode::Expanded && !self.mouse_tracked {
            self.schedule_auto_collapse(AUTO_COLLAPSE_DELAY_MS);
        } else if self.requested_mode == IslandMode::Compact
            && !self.mouse_tracked
            && self.config.notch_collapse_after_4s
        {
            self.schedule_compact_to_notch();
        }
        self.hovered_action = None;
        self.apply_target_and_render();
        (CaptureChange::None, false)
    }

    pub(super) fn commit_timeline_seek_ratio(&mut self, ratio: f32) {
        let Some(target_ticks) = self
            .media_state
            .timeline
            .seek_ticks_for_ratio(ratio.clamp(0.0, 1.0))
        else {
            return;
        };
        self.media_state.timeline.base_position_ticks = target_ticks;
        self.media_state.timeline.last_updated_filetime_ticks = current_windows_filetime_ticks();
        self.seek_hold_until = Some(Instant::now() + Duration::from_millis(TIMELINE_SEEK_HOLD_MS));
        if let Some(worker) = &self.media_worker {
            worker.send_command(MediaCommand::Seek(target_ticks));
        }
    }

    pub(super) fn handle_timeline_seek(&mut self, x_dip: f32) {
        let Some(track) = self.current_timeline_track_rect() else {
            return;
        };
        let ratio = timeline_ratio_for_x(track, x_dip);
        self.commit_timeline_seek_ratio(ratio);
        self.render_current_frame();
    }

    pub(super) fn handle_right_click(&mut self) {
        self.clear_hud_state_quiet();
        self.cancel_compact_to_notch();
        if self.requested_mode == IslandMode::Settings {
            let next = self.normalized_subview_return_mode(self.settings_return_mode);
            self.apply_return_mode(next);
        } else {
            self.cancel_auto_collapse();
            self.settings_return_mode = match self.requested_mode {
                IslandMode::Expanded => IslandMode::Expanded,
                IslandMode::AudioDevices => {
                    self.normalized_subview_return_mode(self.audio_devices_return_mode)
                }
                _ => self.collapsed_rest_mode(),
            };
            self.requested_mode = IslandMode::Settings;
            self.anti_hooks_installed = are_anti_hooks_installed();
        }
        self.hovered_action = None;
        self.apply_target_and_render();
    }

    pub(super) fn execute_ui_action(&mut self, action: UiAction) {
        match action {
            UiAction::OpenSettings => {
                self.clear_hud_state_quiet();
                self.dismiss_notice_state_quiet();
                self.cancel_compact_to_notch();
                self.cancel_auto_collapse();
                self.settings_return_mode = match self.requested_mode {
                    IslandMode::Expanded => IslandMode::Expanded,
                    IslandMode::AudioDevices => {
                        self.normalized_subview_return_mode(self.audio_devices_return_mode)
                    }
                    _ => self.collapsed_rest_mode(),
                };
                self.hovered_action = None;
                self.anti_hooks_installed = are_anti_hooks_installed();
                self.requested_mode = IslandMode::Settings;
            }
            UiAction::OpenAudioDevices => {
                self.clear_hud_state_quiet();
                self.dismiss_notice_state_quiet();
                self.cancel_compact_to_notch();
                self.cancel_auto_collapse();
                self.audio_devices_return_mode = match self.requested_mode {
                    IslandMode::Settings | IslandMode::Expanded => self.requested_mode,
                    _ => self.collapsed_rest_mode(),
                };
                self.hovered_action = None;
                self.requested_mode = IslandMode::AudioDevices;
            }
            UiAction::SelectAudioDevice(visible_idx) => {
                self.select_audio_device_at(self.audio_device_scroll + visible_idx);
                return;
            }
            UiAction::NoticePrimary => {
                let role = self
                    .active_notice
                    .as_ref()
                    .map(|n| match &n.primary_role {
                        NoticePrimaryRole::Dismiss => None,
                        NoticePrimaryRole::InstallUpdate(rel) => Some(rel.clone()),
                    })
                    .unwrap_or(None);
                match role {
                    Some(release) => self.start_update_download(release),
                    None => self.dismiss_notice(),
                }
                return;
            }
            UiAction::NoticeSecondary => {
                let role = self.active_notice.as_ref().map(|n| match &n.secondary_role {
                    NoticeSecondaryRole::Dismiss => (false, None),
                    NoticeSecondaryRole::RetryCheck => (true, None),
                    NoticeSecondaryRole::RetryDownload(rel) => (false, Some(rel.clone())),
                });
                match role {
                    Some((true, _)) => self.handle_check_update_command(),
                    Some((false, Some(rel))) => self.start_update_download(rel),
                    _ => self.dismiss_notice(),
                }
                return;
            }
            UiAction::BackToMedia => {
                self.hovered_action = None;
                let saved = match self.requested_mode {
                    IslandMode::AudioDevices => self.audio_devices_return_mode,
                    IslandMode::Settings => self.settings_return_mode,
                    _ => self.collapsed_rest_mode(),
                };
                let next = self.normalized_subview_return_mode(saved);
                self.apply_return_mode(next);
            }
            UiAction::ToggleVolumeMute => {
                self.volume_snapshot.muted = !self.volume_snapshot.muted;
                if let Some(mon) = &self.audio_monitor {
                    mon.send_command(AudioCommand::ToggleMute);
                }
                self.show_hud(HudContent::Volume {
                    level: self.volume_snapshot.level,
                    muted: self.volume_snapshot.muted,
                    show_device_name: false,
                });
                return;
            }
            UiAction::ToggleHudVolume => {
                self.config.hud_volume_enabled = !self.config.hud_volume_enabled;
                if !self.config.hud_volume_enabled
                    && matches!(self.active_hud, Some(HudContent::Volume { .. }))
                {
                    self.clear_hud_state_quiet();
                }
                save_config(&self.config);
            }
            UiAction::ToggleHudWheel => {
                self.config.hud_wheel_enabled = !self.config.hud_wheel_enabled;
                self.wheel_accum.reset();
                save_config(&self.config);
            }
            UiAction::ToggleHudPower => {
                self.config.hud_power_enabled = !self.config.hud_power_enabled;
                if !self.config.hud_power_enabled
                    && matches!(self.active_hud, Some(HudContent::Power { .. }))
                {
                    self.clear_hud_state_quiet();
                }
                save_config(&self.config);
            }
            UiAction::ToggleNotchCollapse => {
                self.config.notch_collapse_after_4s = !self.config.notch_collapse_after_4s;
                self.sync_mode_after_collapse_policy_change();
                save_config(&self.config);
            }
            UiAction::ToggleAutoHideIdle => {
                self.config.auto_hide_when_idle = !self.config.auto_hide_when_idle;
                self.sync_mode_after_collapse_policy_change();
                save_config(&self.config);
            }
            UiAction::ToggleAutoHideFullscreen => {
                self.config.auto_hide_fullscreen = !self.config.auto_hide_fullscreen;
                self.sync_fullscreen_timer();
                self.refresh_fullscreen_state();
                save_config(&self.config);
            }
            UiAction::ToggleVisualizer => {
                self.config.visualizer_enabled = !self.config.visualizer_enabled;
                save_config(&self.config);
            }
            UiAction::ToggleAutostart => {
                let target = !self.config.autostart;
                if set_autostart_enabled(target) {
                    self.config.autostart = target;
                }
            }
            UiAction::ToggleAntiMonitor => {
                self.config.anti_monitor_enabled = !self.config.anti_monitor_enabled;
                if !self.config.anti_monitor_enabled {
                    self.anti_banner_active = false;
                    self.cancel_anti_expire();
                } else {
                    self.foreground_is_anti = is_foreground_anti_app(self.hwnd);
                }
                save_config(&self.config);
            }
            UiAction::ToggleAntiHooks => {
                self.handle_toggle_anti_hooks();
                return;
            }
            UiAction::CheckUpdate => {
                self.handle_check_update_command();
                return;
            }
            UiAction::CycleMonitor => {
                if !self.monitors.is_empty() {
                    self.config.monitor_index =
                        (self.config.monitor_index + 1) % self.monitors.len();
                    self.reposition_for_selected_monitor();
                    self.refresh_fullscreen_state();
                    save_config(&self.config);
                }
            }
            UiAction::HideIsland => {
                self.clear_hud_state_quiet();
                self.cancel_compact_to_notch();
                let _ = self.reset_pointer_and_drag_state();
                self.config.visible = false;
                self.sync_fullscreen_timer();
                self.update_tray_tooltip();
                save_config(&self.config);
            }
            UiAction::ExitApp => {
                self.shutdown_and_destroy();
                return;
            }
            UiAction::MediaPrev => {
                if self.media_state.has_session
                    && let Some(worker) = &self.media_worker
                {
                    worker.send_command(MediaCommand::Previous);
                }
                return;
            }
            UiAction::MediaPlayPause => {
                if self.media_state.has_session
                    && let Some(worker) = &self.media_worker
                {
                    worker.send_command(MediaCommand::TogglePlayPause);
                }
                return;
            }
            UiAction::MediaNext => {
                if self.media_state.has_session
                    && let Some(worker) = &self.media_worker
                {
                    worker.send_command(MediaCommand::Next);
                }
                return;
            }
            UiAction::ToggleShelf => {
                self.shelf_auto_opened_by_drag = false;
                self.shelf.open = !self.shelf.open;
            }
            UiAction::ShelfClose => {
                self.shelf_auto_opened_by_drag = false;
                self.shelf.open = false;
            }
            UiAction::ShelfToggleSelect(idx) => {
                self.shelf.toggle_select(idx);
            }
            UiAction::ShelfToggleSelectAll => {
                self.shelf.toggle_select_all();
            }
            UiAction::ShelfDeleteAll => {
                if self.shelf.delete_selected_from_shelf() > 0 {
                    save_shelf_state(&self.shelf);
                }
            }
            UiAction::ShelfDragItem(_) => return,
            UiAction::SeekTimeline | UiAction::SeekVolume => return,
        }
        self.hovered_action = None;
        self.apply_target_and_render();
    }
}
