use super::*;

impl WindowController {
    pub(super) fn active_monitor(&self) -> DisplayMonitor {
        select_monitor_or_default(&self.monitors, self.config.monitor_index)
    }

    pub(super) fn anti_display_snapshot(&self) -> Option<&AntiProgressSnapshot> {
        if !self.config.anti_monitor_enabled || !self.anti_banner_active || self.foreground_is_anti
        {
            return None;
        }
        let snap = self.anti_snapshot.as_ref()?;
        if snap.is_stale(now_unix_ms()) {
            return None;
        }
        Some(snap)
    }

    pub(super) fn is_anti_spinner_active(&self) -> bool {
        self.active_hud.is_none()
            && self.anti_display_snapshot().is_some_and(|s| s.state.is_running())
            && matches!(
                self.current_effective_mode(),
                IslandMode::Compact | IslandMode::Slim
            )
    }

    pub(super) fn refresh_anti_foreground_state(&mut self) {
        let next = if self.config.anti_monitor_enabled {
            is_foreground_anti_app(self.hwnd)
        } else {
            false
        };
        if next != self.foreground_is_anti {
            self.foreground_is_anti = next;
            if self.anti_banner_active && self.anti_snapshot.is_some() {
                self.apply_target_and_render();
            }
        }
    }

    pub(super) fn current_effective_mode(&self) -> IslandMode {
        effective_mode(
            self.requested_mode,
            self.config,
            self.fullscreen_active,
            self.media_state.has_session,
            self.active_hud.is_some() || self.anti_display_snapshot().is_some(),
        )
    }

    pub(super) fn refresh_monitors(&mut self) {
        self.monitors = enumerate_monitors();
        if !self.monitors.is_empty() {
            self.config.monitor_index %= self.monitors.len();
        }
        self.reposition_for_selected_monitor();
    }

    pub(super) fn reposition_for_selected_monitor(&mut self) {
        let mon = self.active_monitor();
        if (mon.dpi_scale - self.dpi_scale).abs() >= 0.001 {
            self.dpi_scale = mon.dpi_scale;
            let _ = self.renderer.resize_for_dpi(mon.dpi_scale);
            self.last_region = None;
        }

        let (w, h) = compute_surface_size_px(self.dpi_scale);
        let (x, y) = compute_window_origin_px(mon.monitor_rect.0, mon.monitor_rect.2, mon.work_top, w);
        let next_rect = (x, y, w, h);
        if self.last_window_rect != Some(next_rect) {
            self.last_window_rect = Some(next_rect);
            unsafe {
                let _ = SetWindowPos(
                    self.hwnd,
                    None,
                    x,
                    y,
                    w,
                    h,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }
    }

    pub(super) fn sync_fullscreen_timer(&self) {
        if self.config.visible && self.config.auto_hide_fullscreen {
            unsafe {
                let _ = SetTimer(
                    Some(self.hwnd),
                    TIMER_FULLSCREEN_CHECK_ID,
                    FULLSCREEN_CHECK_INTERVAL_MS,
                    None,
                );
            }
        } else {
            let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_FULLSCREEN_CHECK_ID) };
        }
    }

    pub(super) fn refresh_fullscreen_state(&mut self) -> bool {
        let next = if self.config.auto_hide_fullscreen {
            let mon_rect = self.active_monitor().monitor_rect;
            is_foreground_fullscreen_on_monitor(self.hwnd, mon_rect)
        } else {
            false
        };
        if next != self.fullscreen_active {
            self.fullscreen_active = next;
            if next {
                // Ẩn ngay khi vào Fullscreen để không đè khung hình lên game hoặc video.
                let need_release = self.reset_pointer_and_drag_state();
                self.clear_hud_state_quiet();
                self.animator = IslandAnimator::new(IslandMode::Hidden);
                unsafe {
                    let _ = ShowWindow(self.hwnd, SW_HIDE);
                }
                self.apply_empty_region();
                self.update_vsync_activity();
                self.update_tray_tooltip();
                return need_release;
            } else {
                self.apply_target_and_render();
                self.update_tray_tooltip();
            }
        }
        false
    }

    pub(super) fn toggle_visibility_from_tray(&mut self) {
        if self.current_effective_mode() == IslandMode::Hidden {
            self.config.visible = true;
            self.requested_mode = IslandMode::Compact;
            if !self.mouse_tracked && self.should_collapse_to_slim() {
                self.schedule_compact_to_notch();
            }
        } else {
            self.cancel_compact_to_notch();
            let _ = self.reset_pointer_and_drag_state();
            self.config.visible = false;
        }
        self.sync_fullscreen_timer();
        self.apply_target_and_render();
        self.update_tray_tooltip();
        save_config(&self.config);
    }

    pub(super) fn wake_from_duplicate_launch(&mut self) {
        self.config.visible = true;
        self.requested_mode = if self.media_state.has_session {
            self.cancel_compact_to_notch();
            IslandMode::Expanded
        } else {
            if !self.mouse_tracked && self.should_collapse_to_slim() {
                self.schedule_compact_to_notch();
            }
            IslandMode::Compact
        };
        if self.requested_mode == IslandMode::Expanded && !self.mouse_tracked {
            self.schedule_auto_collapse(AUTO_COLLAPSE_DELAY_MS);
        }
        self.sync_fullscreen_timer();
        self.apply_target_and_render();
        self.update_tray_tooltip();
    }

    pub(super) fn apply_target_and_render(&mut self) {
        let was_animating = self.animator.is_animating();
        let mode = self.current_effective_mode();
        self.animator
            .set_target_mode_with_audio_count(mode, self.audio_devices.len());

        if mode != IslandMode::Hidden {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            }
        }

        if !was_animating && self.animator.is_animating() {
            // Vẽ ngay khung hình chuyển động đầu tiên thay vì chờ nhịp thread nền tiếp theo.
            let _ = self.animator.step(Instant::now());
            self.render_current_frame();
        } else if !self.animator.is_animating() || self.vsync.is_none() || mode == IslandMode::Hidden {
            self.render_current_frame();
        }
        if self.animator.should_hide_window() {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
            self.apply_empty_region();
        }
        self.update_vsync_activity();
        self.sync_timeline_timer();
    }

    pub(super) fn request_smooth_render(&mut self) {
        if self.animator.is_animating() && self.vsync.is_some() {
            return;
        }
        if (self.dragging_timeline || self.dragging_volume) && self.vsync.is_some() {
            self.drag_render_dirty = true;
            self.update_vsync_activity();
            return;
        }
        self.render_current_frame();
    }

    pub(super) fn sync_timeline_timer(&self) {
        let mode = self.current_effective_mode();
        let need_timeline = self.media_state.has_session
            && self.media_state.is_playing
            && self.media_state.timeline.available
            && (mode == IslandMode::Expanded
                || (matches!(mode, IslandMode::Slim | IslandMode::Compact)
                    && !self.config.visualizer_enabled
                    && self.active_hud.is_none()));
        if need_timeline {
            unsafe {
                let _ = SetTimer(
                    Some(self.hwnd),
                    TIMER_TIMELINE_TICK_ID,
                    TIMELINE_TICK_INTERVAL_MS,
                    None,
                );
            }
        } else {
            let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_TIMELINE_TICK_ID) };
        }
    }

    pub(super) fn handle_vsync_tick(&mut self) {
        if let Some(vsync) = &self.vsync {
            vsync.clear_pending();
        }
        let was_animating = self.animator.is_animating();
        let still_animating = self.animator.step(Instant::now());
        if self.config.visualizer_enabled && self.media_state.is_playing {
            self.visualizer_phase = (self.visualizer_phase + 0.32) % std::f32::consts::TAU;
        }
        let anti_spinning = self.is_anti_spinner_active();
        if anti_spinning {
            self.anti_spinner_phase = (self.anti_spinner_phase + 0.34) % std::f32::consts::TAU;
        }

        let is_dragging = self.dragging_timeline || self.dragging_volume;
        let should_render = still_animating
            || was_animating
            || (is_dragging && self.drag_render_dirty)
            || (!is_dragging && self.config.visualizer_enabled && self.media_state.is_playing)
            || (!is_dragging && anti_spinning);
        if should_render {
            self.drag_render_dirty = false;
            self.render_current_frame();
        }

        if self.animator.should_hide_window() {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
            self.apply_empty_region();
        }

        if was_animating && !still_animating {
            self.sync_pointer_presence();
        }
        if !still_animating {
            self.update_vsync_activity();
        }
    }

    pub(super) fn update_vsync_activity(&self) {
        let vis_active = self.config.visualizer_enabled
            && self.media_state.has_session
            && self.media_state.is_playing
            && matches!(
                self.current_effective_mode(),
                IslandMode::Compact | IslandMode::Slim
            );
        let ticker_mode =
            if self.animator.is_animating() || self.dragging_timeline || self.dragging_volume {
                TickerMode::VSync
            } else if vis_active || self.is_anti_spinner_active() {
                TickerMode::VisualizerSlow
            } else {
                TickerMode::Off
            };
        if let Some(vsync) = &self.vsync {
            vsync.set_mode(ticker_mode);
        }
    }

    pub(super) fn active_shelf_rect(&self, pill: PillRectDip) -> Option<PillRectDip> {
        if !self.config.visible || self.fullscreen_active {
            return None;
        }
        shelf_pill_rect(
            pill,
            self.current_effective_mode(),
            self.shelf.open,
            self.shelf.items.len(),
        )
    }

    pub(super) fn render_current_frame(&mut self) {
        let mode = self.current_effective_mode();
        if mode == IslandMode::Hidden && self.animator.should_hide_window() {
            self.renderer.hide_backdrop();
            return;
        }

        let pill = self.animator.current_pill();
        let display_mode = if mode == IslandMode::Hidden {
            self.animator.target_mode
        } else {
            mode
        };
        let mon_label = self.active_monitor().short_label.clone();
        let notice_view = self.active_notice.as_ref().map(ActiveNotice::as_view);
        let anti_snap = if matches!(display_mode, IslandMode::Compact | IslandMode::Slim) {
            self.anti_display_snapshot().cloned()
        } else {
            None
        };
        let shelf_visible = self.config.visible
            && !self.fullscreen_active
            && matches!(
                display_mode,
                IslandMode::Slim | IslandMode::Compact | IslandMode::Expanded
            );
        let shelf_view = ShelfRenderInput {
            visible: shelf_visible,
            open: self.shelf.open,
            drag_hover: self.shelf.drag_hover,
            items: &self.shelf.items,
            scroll: self.shelf.scroll,
        };

        let frame = RenderFrameInput {
            pill,
            mode: display_mode,
            pill_opacity: self.animator.opacity.value,
            content_alpha: self.animator.content_alpha.value,
            config: self.config,
            media: &self.media_state,
            active_hud: if display_mode == IslandMode::Compact {
                self.active_hud
            } else {
                None
            },
            volume_scrub_ratio: if self.dragging_volume {
                self.volume_drag_ratio
            } else {
                None
            },
            hovered_action: self.hovered_action,
            timeline_scrub_ratio: if self.dragging_timeline {
                self.timeline_drag_ratio
            } else {
                None
            },
            monitor_label: &mon_label,
            visualizer_phase: self.visualizer_phase,
            audio_devices: &self.audio_devices,
            audio_device_scroll: self.audio_device_scroll,
            volume_snapshot: self.volume_snapshot,
            current_audio_device_name: self.current_audio_device_name.as_deref(),
            notice: if display_mode == IslandMode::Notice {
                notice_view
            } else {
                None
            },
            available_update_tag: self
                .available_update
                .as_ref()
                .map(|r| r.version_tag.as_str()),
            anti: anti_snap.as_ref(),
            anti_hooks_installed: self.anti_hooks_installed,
            anti_spinner_phase: self.anti_spinner_phase,
            shelf: shelf_view,
        };
        let _ = self.renderer.render(&frame);

        if self.animator.opacity.value > 0.02 {
            self.update_window_region(pill);
        }
    }

    pub(super) fn update_window_region(&mut self, pill: PillRectDip) {
        let shelf_rgn = self
            .active_shelf_rect(pill)
            .map(|sr| compute_pill_region_px(sr, self.dpi_scale));

        if self.animator.is_animating() {
            if self.animator.target_mode == IslandMode::Hidden {
                return;
            }
            let cur_rgn = compute_pill_region_px(pill, self.dpi_scale);
            let shelf_unchanged = self.last_shelf_region == shelf_rgn;
            let already_covers_rect = self.last_region.is_some_and(|(prev, diam)| {
                diam == 0
                    && prev.left <= cur_rgn.0.left
                    && prev.top <= cur_rgn.0.top
                    && prev.right >= cur_rgn.0.right
                    && prev.bottom >= cur_rgn.0.bottom
            });
            if already_covers_rect && shelf_unchanged {
                return;
            }
            let (tw, th, _, _) =
                target_dimensions_for_state(self.animator.target_mode, self.audio_devices.len());
            let t_top = target_top_for_mode(self.animator.target_mode);
            let t_left = (MAX_SURFACE_WIDTH_DIP - tw) * 0.5;
            let left = (pill.left.min(t_left) - 14.0).max(0.0);
            let top = (pill.top.min(t_top) - 6.0).max(0.0);
            let right = ((pill.left + pill.width).max(t_left + tw) + 14.0).min(MAX_SURFACE_WIDTH_DIP);
            let bottom = ((pill.top + pill.height).max(t_top + th) + 10.0).min(MAX_SURFACE_HEIGHT_DIP);
            let union_pill = PillRectDip {
                left,
                top,
                width: (right - left).max(24.0),
                height: (bottom - top).max(8.0),
                corner_radius: 1.0,
            };
            let (rect, _) = compute_pill_region_px(union_pill, self.dpi_scale);
            self.apply_combined_regions((rect, 0), shelf_rgn);
        } else {
            let exact = compute_pill_region_px(pill, self.dpi_scale);
            if self.last_region != Some(exact) || self.last_shelf_region != shelf_rgn {
                self.apply_combined_regions(exact, shelf_rgn);
            }
        }
    }

    pub(super) fn apply_combined_regions(
        &mut self,
        primary: (PixelRect, i32),
        secondary: Option<(PixelRect, i32)>,
    ) {
        let make_hrgn = |(rect, diameter): (PixelRect, i32)| -> HRGN {
            if diameter == 0 {
                unsafe { CreateRectRgn(rect.left, rect.top, rect.right, rect.bottom) }
            } else {
                unsafe {
                    CreateRoundRectRgn(
                        rect.left,
                        rect.top,
                        rect.right,
                        rect.bottom,
                        diameter,
                        diameter,
                    )
                }
            }
        };

        let rgn_main = make_hrgn(primary);
        if rgn_main.is_invalid() {
            return;
        }
        if let Some(sec) = secondary {
            let rgn_sec = make_hrgn(sec);
            if !rgn_sec.is_invalid() {
                unsafe {
                    let _ = CombineRgn(Some(rgn_main), Some(rgn_main), Some(rgn_sec), RGN_OR);
                    let _ = DeleteObject(HGDIOBJ(rgn_sec.0));
                }
            }
        }
        if unsafe { SetWindowRgn(self.hwnd, Some(rgn_main), false) } == 0 {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(rgn_main.0));
            }
        } else {
            self.last_region = Some(primary);
            self.last_shelf_region = secondary;
        }
    }

    pub(super) fn apply_empty_region(&mut self) {
        self.last_region = None;
        self.last_shelf_region = None;
        let rgn = unsafe { CreateRectRgn(0, 0, 0, 0) };
        if !rgn.is_invalid() && unsafe { SetWindowRgn(self.hwnd, Some(rgn), false) } == 0 {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(rgn.0));
            }
        }
    }

    pub(super) fn update_tray_tooltip(&self) {
        if let Some(tray) = &self.tray {
            let status = if !self.config.visible {
                "Đang ẩn"
            } else if self.fullscreen_active {
                "Tạm ẩn (Toàn màn hình)"
            } else if self.media_state.has_session && self.media_state.is_playing {
                "Đang phát"
            } else {
                "Sẵn sàng"
            };
            let glass_tag = if self.renderer.has_blur_effect() {
                "Kính mờ Acrylic - "
            } else if self.renderer.has_glass_backdrop() {
                "Kính trong - "
            } else {
                ""
            };
            let update_tag = match &self.available_update {
                Some(rel) => format!(" - Có bản mới {}", rel.version_tag),
                None if self.update_in_progress => String::from(" - Đang kiểm tra cập nhật"),
                None => String::new(),
            };
            let msg = format!(
                "Win Dynamic Island - {}{} ({}{})",
                status,
                update_tag,
                glass_tag,
                self.renderer.adapter_name()
            );
            tray.update_tooltip(&msg);
        }
    }

    pub(super) fn should_collapse_to_slim(&self) -> bool {
        self.config.notch_collapse_after_4s
            || (self.config.auto_hide_when_idle && !self.media_state.has_session)
    }

    pub(super) fn collapsed_rest_mode(&self) -> IslandMode {
        if self.media_state.has_session || !self.should_collapse_to_slim() {
            IslandMode::Compact
        } else {
            IslandMode::Slim
        }
    }

    pub(super) fn schedule_auto_collapse(&self, delay_ms: u32) {
        unsafe {
            let _ = SetTimer(Some(self.hwnd), TIMER_AUTO_COLLAPSE_ID, delay_ms, None);
        }
    }

    pub(super) fn cancel_auto_collapse(&self) {
        let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_AUTO_COLLAPSE_ID) };
    }

    pub(super) fn schedule_compact_to_notch(&self) {
        unsafe {
            let _ = SetTimer(
                Some(self.hwnd),
                TIMER_COMPACT_TO_NOTCH_ID,
                COMPACT_TO_NOTCH_DELAY_MS,
                None,
            );
        }
    }

    pub(super) fn cancel_compact_to_notch(&self) {
        let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_COMPACT_TO_NOTCH_ID) };
    }

    pub(super) fn handle_compact_to_notch_timer(&mut self) {
        self.cancel_compact_to_notch();
        if !self.mouse_tracked
            && !self.dragging_timeline
            && self.requested_mode == IslandMode::Compact
            && self.should_collapse_to_slim()
        {
            self.requested_mode = IslandMode::Slim;
            self.hovered_action = None;
            self.apply_target_and_render();
        }
    }

    pub(super) fn handle_auto_collapse_timer(&mut self) {
        self.cancel_auto_collapse();
        if !self.mouse_tracked && !self.dragging_timeline && self.requested_mode == IslandMode::Expanded
        {
            self.requested_mode = if self.should_collapse_to_slim() {
                IslandMode::Slim
            } else {
                self.collapsed_rest_mode()
            };
            self.hovered_action = None;
            self.apply_target_and_render();
        }
    }

    pub(super) fn sync_mode_after_collapse_policy_change(&mut self) {
        if self.should_collapse_to_slim() {
            if self.requested_mode == IslandMode::Compact && !self.mouse_tracked {
                self.schedule_compact_to_notch();
            }
        } else {
            self.cancel_compact_to_notch();
            if self.requested_mode == IslandMode::Slim {
                self.requested_mode = IslandMode::Compact;
            }
            if self.settings_return_mode == IslandMode::Slim {
                self.settings_return_mode = IslandMode::Compact;
            }
            if self.audio_devices_return_mode == IslandMode::Slim {
                self.audio_devices_return_mode = IslandMode::Compact;
            }
        }
    }

    pub(super) fn normalized_subview_return_mode(&self, saved: IslandMode) -> IslandMode {
        match saved {
            IslandMode::Expanded => IslandMode::Expanded,
            IslandMode::Settings => IslandMode::Settings,
            _ => self.collapsed_rest_mode(),
        }
    }

    pub(super) fn apply_return_mode(&mut self, next: IslandMode) {
        self.requested_mode = next;
        if next == IslandMode::Expanded && !self.mouse_tracked {
            self.schedule_auto_collapse(AUTO_COLLAPSE_DELAY_MS);
        } else if next == IslandMode::Compact
            && !self.mouse_tracked
            && self.config.notch_collapse_after_4s
        {
            self.schedule_compact_to_notch();
        }
    }
}
