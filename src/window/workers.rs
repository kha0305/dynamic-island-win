use super::*;

impl WindowController {
    pub(super) fn handle_media_update(&mut self) {
        let Some(worker) = &self.media_worker else {
            return;
        };
        let Some(update) = worker.drain_latest_update() else {
            return;
        };

        let was_has_session = self.media_state.has_session;
        let same_track = update.display.has_session
            && was_has_session
            && update.display.title == self.media_state.title
            && update.display.artist == self.media_state.artist;
        let track_changed = update.display.has_session && !same_track;
        let preserve_local_timeline = same_track
            && update.display.timeline.available
            && (self.dragging_timeline
                || self
                    .seek_hold_until
                    .is_some_and(|until| Instant::now() < until));
        let prev_timeline = self.media_state.timeline;
        self.media_state = update.display;
        if preserve_local_timeline {
            self.media_state.timeline = prev_timeline;
        } else if !same_track {
            self.seek_hold_until = None;
            self.dragging_timeline = false;
            self.timeline_drag_ratio = None;
        }
        self.renderer.update_artwork(update.artwork);

        if self.media_state.has_session {
            if track_changed
                && matches!(self.requested_mode, IslandMode::Slim | IslandMode::Compact)
            {
                // Bung Compact 4 giây khi có nhạc mới / đổi bài để nhìn tên bài rồi thu về dải Notch.
                self.requested_mode = IslandMode::Compact;
                if self.config.notch_collapse_after_4s && !self.mouse_tracked {
                    self.schedule_compact_to_notch();
                } else {
                    self.cancel_compact_to_notch();
                }
            } else if self.requested_mode == IslandMode::Slim && !self.should_collapse_to_slim() {
                self.requested_mode = IslandMode::Compact;
                self.cancel_compact_to_notch();
            }
        } else {
            self.cancel_compact_to_notch();
            if self.requested_mode == IslandMode::Compact
                && !self.mouse_tracked
                && self.should_collapse_to_slim()
            {
                self.requested_mode = IslandMode::Slim;
            } else if self.requested_mode == IslandMode::Slim && !self.should_collapse_to_slim() {
                self.requested_mode = IslandMode::Compact;
            }
        }

        self.apply_target_and_render();
        self.update_tray_tooltip();
    }

    pub(super) fn schedule_startup_update_check(&self) {
        unsafe {
            let _ = SetTimer(
                Some(self.hwnd),
                TIMER_STARTUP_UPDATE_ID,
                STARTUP_UPDATE_DELAY_MS,
                None,
            );
        }
    }

    pub(super) fn trigger_update_check(&mut self, silent: bool) {
        if self.update_in_progress {
            return;
        }
        self.update_in_progress = true;
        self.update_rx = Some(check_for_updates_async(self.hwnd, silent));
        if !silent {
            self.update_tray_tooltip();
        }
    }

    pub(super) fn schedule_notice_expire(&self) {
        unsafe {
            let _ = SetTimer(
                Some(self.hwnd),
                TIMER_NOTICE_EXPIRE_ID,
                NOTICE_AUTO_DISMISS_MS,
                None,
            );
        }
    }

    pub(super) fn cancel_notice_expire(&self) {
        let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_NOTICE_EXPIRE_ID) };
    }

    pub(super) fn dismiss_notice_state_quiet(&mut self) {
        self.cancel_notice_expire();
        self.active_notice = None;
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn show_notice(
        &mut self,
        kind: NoticeKind,
        title: impl Into<String>,
        message: impl Into<String>,
        primary_label: &'static str,
        primary_role: NoticePrimaryRole,
        secondary_label: Option<&'static str>,
        secondary_role: NoticeSecondaryRole,
        auto_expire: bool,
    ) {
        let return_mode = if let Some(prev) = &self.active_notice {
            prev.return_mode
        } else if matches!(
            self.requested_mode,
            IslandMode::Notice | IslandMode::Hidden
        ) {
            self.collapsed_rest_mode()
        } else {
            self.requested_mode
        };

        self.clear_hud_state_quiet();
        self.cancel_compact_to_notch();
        self.cancel_auto_collapse();
        self.cancel_notice_expire();
        self.hovered_action = None;
        self.active_notice = Some(ActiveNotice {
            kind,
            title: title.into(),
            message: message.into(),
            primary_label,
            primary_role,
            secondary_label,
            secondary_role,
            return_mode,
            auto_expire,
        });
        self.requested_mode = IslandMode::Notice;
        if auto_expire && !self.mouse_tracked {
            self.schedule_notice_expire();
        }
        self.apply_target_and_render();
    }

    pub(super) fn dismiss_notice(&mut self) {
        let Some(notice) = self.active_notice.take() else {
            return;
        };
        self.cancel_notice_expire();
        self.hovered_action = None;
        let next = match notice.return_mode {
            IslandMode::Notice | IslandMode::Hidden => self.collapsed_rest_mode(),
            IslandMode::Compact => self.collapsed_rest_mode(),
            other => other,
        };
        self.requested_mode = next;
        if next == IslandMode::Expanded && !self.mouse_tracked {
            self.schedule_auto_collapse(AUTO_COLLAPSE_DELAY_MS);
        } else if next == IslandMode::Compact && !self.mouse_tracked && self.config.notch_collapse_after_4s {
            self.schedule_compact_to_notch();
        }
        self.apply_target_and_render();
    }

    pub(super) fn show_update_available_notice(&mut self, release: UpdateReleaseInfo) {
        let msg = format!(
            "Đã có phiên bản {} (hiện tại v{APP_VERSION}).\nBạn có muốn tải về và cập nhật ngay không?",
            release.version_tag
        );
        self.show_notice(
            NoticeKind::UpdateAvailable,
            "Có bản cập nhật mới",
            msg,
            "Cập nhật ngay",
            NoticePrimaryRole::InstallUpdate(release),
            Some("Để sau"),
            NoticeSecondaryRole::Dismiss,
            false,
        );
    }

    pub(super) fn start_update_download(&mut self, release: UpdateReleaseInfo) {
        if self.update_in_progress {
            return;
        }
        self.update_in_progress = true;
        self.downloading_release = Some(release.clone());
        let tag = release.version_tag.clone();
        self.update_rx = Some(download_and_stage_update_async(self.hwnd, release));
        self.update_tray_tooltip();
        self.show_notice(
            NoticeKind::Downloading,
            "Đang tải bản cập nhật",
            format!("Đang tải và xác thực tệp cài đặt cho {tag}..."),
            "Thu gọn",
            NoticePrimaryRole::Dismiss,
            None,
            NoticeSecondaryRole::Dismiss,
            false,
        );
    }

    pub(super) fn handle_check_update_command(&mut self) {
        self.config.visible = true;
        self.sync_fullscreen_timer();
        if self.update_in_progress {
            if self.active_notice.is_some() {
                self.requested_mode = IslandMode::Notice;
                self.apply_target_and_render();
            } else {
                self.show_notice(
                    NoticeKind::Checking,
                    "Kiểm tra cập nhật",
                    "Đang kiểm tra hoặc tải bản cập nhật ở nền...",
                    "Thu gọn",
                    NoticePrimaryRole::Dismiss,
                    None,
                    NoticeSecondaryRole::Dismiss,
                    false,
                );
            }
            return;
        }
        if let Some(release) = self.available_update.clone() {
            self.show_update_available_notice(release);
            return;
        }
        self.show_notice(
            NoticeKind::Checking,
            "Kiểm tra cập nhật",
            format!("Đang kết nối GitHub Releases (hiện tại v{APP_VERSION})..."),
            "Thu gọn",
            NoticePrimaryRole::Dismiss,
            None,
            NoticeSecondaryRole::Dismiss,
            false,
        );
        self.trigger_update_check(false);
    }

    pub(super) fn handle_update_event(&mut self) {
        let Some(rx) = self.update_rx.take() else {
            return;
        };
        let Ok(event) = rx.try_recv() else {
            self.update_rx = Some(rx);
            return;
        };
        self.update_in_progress = false;
        let failed_release = self.downloading_release.take();
        self.update_tray_tooltip();

        match event {
            UpdaterEvent::CheckUpToDate { silent } => {
                if !silent {
                    self.show_notice(
                        NoticeKind::UpToDate,
                        "Đã là bản mới nhất",
                        format!("Bạn đang sử dụng phiên bản mới nhất (v{APP_VERSION})."),
                        "Đã hiểu",
                        NoticePrimaryRole::Dismiss,
                        None,
                        NoticeSecondaryRole::Dismiss,
                        true,
                    );
                }
            }
            UpdaterEvent::CheckAvailable { release, silent } => {
                self.available_update = Some(release.clone());
                self.update_tray_tooltip();
                let allow_startup_popup = self.config.visible
                    && !self.fullscreen_active
                    && !matches!(
                        self.requested_mode,
                        IslandMode::Settings | IslandMode::AudioDevices | IslandMode::Notice
                    );
                if !silent || allow_startup_popup {
                    self.show_update_available_notice(release);
                }
            }
            UpdaterEvent::CheckError { message, silent } => {
                if !silent {
                    self.show_notice(
                        NoticeKind::Error,
                        "Không thể kiểm tra cập nhật",
                        message,
                        "Đóng",
                        NoticePrimaryRole::Dismiss,
                        Some("Thử lại"),
                        NoticeSecondaryRole::RetryCheck,
                        false,
                    );
                }
            }
            UpdaterEvent::DownloadReadyToRestart => {
                if let Err(err) = spawn_updated_process() {
                    let retry_role = failed_release.map_or(
                        NoticeSecondaryRole::RetryCheck,
                        NoticeSecondaryRole::RetryDownload,
                    );
                    self.show_notice(
                        NoticeKind::Error,
                        "Không thể khởi chạy bản mới",
                        err,
                        "Đóng",
                        NoticePrimaryRole::Dismiss,
                        Some("Thử lại"),
                        retry_role,
                        false,
                    );
                    return;
                }
                self.shutdown_and_destroy();
            }
            UpdaterEvent::DownloadError { message } => {
                let retry_role = failed_release.map_or(
                    NoticeSecondaryRole::RetryCheck,
                    NoticeSecondaryRole::RetryDownload,
                );
                self.show_notice(
                    NoticeKind::Error,
                    "Tải bản cập nhật thất bại",
                    message,
                    "Đóng",
                    NoticePrimaryRole::Dismiss,
                    Some("Thử lại"),
                    retry_role,
                    false,
                );
            }
        }
    }

    pub(super) fn schedule_hud_expire(&self) {
        unsafe {
            let _ = SetTimer(
                Some(self.hwnd),
                TIMER_HUD_EXPIRE_ID,
                HUD_EXPIRE_DELAY_MS,
                None,
            );
        }
    }

    pub(super) fn cancel_hud_expire(&self) {
        let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_HUD_EXPIRE_ID) };
    }

    pub(super) fn clear_hud_state_quiet(&mut self) {
        self.cancel_hud_expire();
        self.active_hud = None;
        self.dragging_volume = false;
        self.volume_drag_ratio = None;
    }

    pub(super) fn clear_hud(&mut self) {
        let had_hud = self.active_hud.is_some();
        self.clear_hud_state_quiet();
        if had_hud {
            self.sync_pointer_presence();
            self.hovered_action = None;
            if self.mouse_tracked {
                // Chuột đang nằm trên viên thuốc khi HUD tắt: giữ Compact để không tụt hẫng dưới con trỏ.
                if self.requested_mode == IslandMode::Slim {
                    self.requested_mode = IslandMode::Compact;
                }
            } else if self.requested_mode == IslandMode::Compact && self.should_collapse_to_slim() {
                self.requested_mode = IslandMode::Slim;
            } else if self.requested_mode == IslandMode::Slim && !self.should_collapse_to_slim() {
                self.requested_mode = IslandMode::Compact;
            }
            self.apply_target_and_render();
        }
    }

    pub(super) fn show_hud(&mut self, hud: HudContent) {
        if !self.config.visible || (self.config.auto_hide_fullscreen && self.fullscreen_active) {
            return;
        }
        if matches!(
            self.requested_mode,
            IslandMode::Expanded
                | IslandMode::Settings
                | IslandMode::AudioDevices
                | IslandMode::Notice
        ) {
            return;
        }
        self.cancel_compact_to_notch();
        self.active_hud = Some(hud);
        if self.dragging_volume {
            self.cancel_hud_expire();
        } else {
            // Luôn đặt hạn 2 giây để HUD tự thu về nhạc khi người dùng ngừng cuộn dù con trỏ đứng yên trên pill.
            self.schedule_hud_expire();
        }
        self.apply_target_and_render();
    }

    pub(super) fn handle_hud_expire_timer(&mut self) {
        self.cancel_hud_expire();
        if self.dragging_volume {
            return;
        }
        self.clear_hud();
    }

    pub(super) fn handle_hud_worker_event(&mut self) {
        let Some(mon) = self.audio_monitor.as_ref() else {
            return;
        };
        let mut latest_sync = None;
        let mut latest_devices: Option<Vec<AudioOutputDevice>> = None;
        let mut latest_power: Option<(crate::hud::PowerSnapshot, bool)> = None;
        let mut unavailable = false;
        while let Some(ev) = mon.try_recv() {
            match ev {
                HudWorkerEvent::VolumeSync {
                    level,
                    muted,
                    device_name,
                    switched_device,
                    show_hud,
                } => {
                    latest_sync = Some((level, muted, device_name, switched_device, show_hud));
                    unavailable = false;
                }
                HudWorkerEvent::DevicesChanged { devices } => {
                    latest_devices = Some(devices);
                }
                HudWorkerEvent::PowerSync { snap, initial } => {
                    latest_power = Some((snap, initial));
                }
                HudWorkerEvent::AudioUnavailable => {
                    unavailable = true;
                }
            }
        }
        if let Some(devices) = latest_devices {
            if let Some(def) = devices.iter().find(|d| d.is_default) {
                self.current_audio_device_name = Some(def.name.clone());
            } else if devices.is_empty() {
                self.current_audio_device_name = None;
            }
            let max_scroll = devices.len().saturating_sub(MAX_AUDIO_DEVICES_VISIBLE);
            self.audio_device_scroll = self.audio_device_scroll.min(max_scroll);
            self.audio_devices = devices;
            if latest_sync.is_none() {
                let mode = self.current_effective_mode();
                if mode == IslandMode::AudioDevices {
                    self.apply_target_and_render();
                } else if mode == IslandMode::Expanded && !self.animator.is_animating() {
                    self.render_current_frame();
                }
            }
        }
        if let Some((snap, initial)) = latest_power {
            if initial {
                if self.power_tracker.last_snapshot().is_none() {
                    let _ = self.power_tracker.evaluate_update(snap);
                }
            } else if let Some((next, low_battery)) = self.power_tracker.evaluate_update(snap)
                && self.config.hud_power_enabled
            {
                self.show_hud(HudContent::Power {
                    on_ac: next.on_ac,
                    percent: next.percent,
                    low_battery,
                });
            } else if let Some(HudContent::Power {
                on_ac,
                percent,
                low_battery,
            }) = self.active_hud
                && on_ac == snap.on_ac
                && percent != snap.percent
            {
                self.active_hud = Some(HudContent::Power {
                    on_ac: snap.on_ac,
                    percent: snap.percent,
                    low_battery,
                });
                self.request_smooth_render();
            }
        }
        if unavailable && latest_sync.is_none() {
            self.volume_snapshot.available = false;
            return;
        }
        if let Some((level, muted, device_name, switched_device, show_hud)) = latest_sync {
            if let Some(name) = device_name {
                self.current_audio_device_name = Some(name);
            }
            let holding = self.dragging_volume
                || self
                    .volume_hold_until
                    .is_some_and(|until| Instant::now() < until);
            if !holding {
                self.volume_snapshot = VolumeSnapshot {
                    level,
                    muted,
                    available: true,
                };
                if show_hud && self.config.hud_volume_enabled {
                    self.show_hud(HudContent::Volume {
                        level,
                        muted,
                        show_device_name: switched_device,
                    });
                } else if matches!(self.active_hud, Some(HudContent::Volume { .. })) {
                    self.active_hud = Some(HudContent::Volume {
                        level,
                        muted,
                        show_device_name: switched_device,
                    });
                    self.request_smooth_render();
                } else if matches!(
                    self.current_effective_mode(),
                    IslandMode::Expanded | IslandMode::AudioDevices
                ) && !self.animator.is_animating()
                {
                    self.render_current_frame();
                }
            } else {
                self.volume_snapshot.available = true;
            }
        }
    }

    pub(super) fn select_audio_device_at(&mut self, index: usize) {
        let Some(target) = self.audio_devices.get(index).cloned() else {
            return;
        };
        if target.is_default {
            return;
        }
        for (idx, dev) in self.audio_devices.iter_mut().enumerate() {
            dev.is_default = idx == index;
        }
        self.current_audio_device_name = Some(target.name);
        if let Some(mon) = &self.audio_monitor {
            mon.send_command(AudioCommand::SetDefaultDevice(target.id));
        }
        if matches!(
            self.current_effective_mode(),
            IslandMode::AudioDevices | IslandMode::Expanded
        ) && !self.animator.is_animating()
        {
            self.render_current_frame();
        }
    }

    pub(super) fn handle_power_ac_hint(&mut self, on_ac: bool) {
        if let Some((next, low_battery)) = self.power_tracker.evaluate_ac_hint(on_ac)
            && self.config.hud_power_enabled
        {
            self.show_hud(HudContent::Power {
                on_ac: next.on_ac,
                percent: next.percent,
                low_battery,
            });
        }
    }

    pub(super) fn request_power_refresh(&self) {
        if let Some(mon) = &self.audio_monitor {
            mon.send_command(AudioCommand::RefreshPower);
        }
    }

    pub(super) fn apply_local_volume(&mut self, ratio: f32, unmute_if_positive: bool) {
        let clamped = ratio.clamp(0.0, 1.0);
        if unmute_if_positive && clamped > 0.004 && self.volume_snapshot.muted {
            self.volume_snapshot.muted = false;
        }
        self.volume_snapshot.level = clamped;
        self.volume_hold_until = Some(Instant::now() + Duration::from_millis(VOLUME_ECHO_HOLD_MS));
        self.active_hud = Some(HudContent::Volume {
            level: self.volume_snapshot.level,
            muted: self.volume_snapshot.muted,
            show_device_name: false,
        });
        if let Some(mon) = &self.audio_monitor {
            mon.send_command(AudioCommand::SetVolume(clamped));
        }
    }

    pub(super) fn schedule_anti_expire(&self) {
        unsafe {
            let _ = SetTimer(
                Some(self.hwnd),
                TIMER_ANTI_EXPIRE_ID,
                ANTI_TERMINAL_DISPLAY_MS,
                None,
            );
        }
    }

    pub(super) fn cancel_anti_expire(&self) {
        let _ = unsafe { KillTimer(Some(self.hwnd), TIMER_ANTI_EXPIRE_ID) };
    }

    pub(super) fn handle_anti_expire_timer(&mut self) {
        self.cancel_anti_expire();
        self.anti_banner_active = false;
        self.anti_snapshot = None;
        self.apply_target_and_render();
    }

    pub(super) fn handle_anti_update(&mut self) {
        let Some(latest) = self.anti_watcher.as_ref().and_then(AntiWatcher::drain_latest) else {
            return;
        };
        self.anti_snapshot = latest;
        match &self.anti_snapshot {
            Some(snap) if snap.state.is_running() => {
                self.cancel_anti_expire();
                self.anti_banner_active = true;
            }
            Some(_) => {
                self.anti_banner_active = true;
                self.schedule_anti_expire();
            }
            None => {
                self.cancel_anti_expire();
                self.anti_banner_active = false;
            }
        }
        self.foreground_is_anti =
            self.config.anti_monitor_enabled && is_foreground_anti_app(self.hwnd);
        self.apply_target_and_render();
    }

    pub(super) fn handle_toggle_anti_hooks(&mut self) {
        let installed = are_anti_hooks_installed();
        let res = if installed {
            uninstall_anti_hooks().map(|()| false)
        } else {
            install_anti_hooks().map(|()| true)
        };
        match res {
            Ok(now_installed) => {
                self.anti_hooks_installed = now_installed;
                let (title, msg) = if now_installed {
                    (
                        String::from("Đã cài Hook Anti"),
                        String::from(
                            "Đã gắn an toàn vào ~/.gemini/config/hooks.json cho agy CLI, Anti IDE và Anti App.",
                        ),
                    )
                } else {
                    (
                        String::from("Đã gỡ Hook Anti"),
                        String::from("Đã gỡ cấu hình Hook Anti khỏi ~/.gemini/config/hooks.json."),
                    )
                };
                self.show_notice(
                    NoticeKind::UpToDate,
                    title,
                    msg,
                    "Đã hiểu",
                    NoticePrimaryRole::Dismiss,
                    None,
                    NoticeSecondaryRole::Dismiss,
                    true,
                );
            }
            Err(err) => {
                self.anti_hooks_installed = are_anti_hooks_installed();
                self.show_notice(
                    NoticeKind::Error,
                    String::from("Không cấu hình được Hook Anti"),
                    err,
                    "Đóng",
                    NoticePrimaryRole::Dismiss,
                    None,
                    NoticeSecondaryRole::Dismiss,
                    false,
                );
            }
        }
    }

    pub(super) fn handle_shelf_drag_hover(&mut self, active: bool) {
        let mut changed = self.shelf.drag_hover != active;
        self.shelf.drag_hover = active;
        if active {
            if !self.shelf.open {
                self.shelf.open = true;
                self.shelf_auto_opened_by_drag = true;
                changed = true;
            }
        } else if self.shelf_auto_opened_by_drag {
            self.shelf_auto_opened_by_drag = false;
            if self.shelf.open {
                self.shelf.open = false;
                changed = true;
            }
        }
        if changed {
            self.render_current_frame();
        }
    }

    pub(super) fn handle_shelf_drop(&mut self) {
        let mut all_paths = Vec::new();
        while let Ok(mut paths) = self.shelf_drop_rx.try_recv() {
            all_paths.append(&mut paths);
        }
        self.handle_shelf_drop_paths(all_paths);
    }

    pub(super) fn handle_shelf_drop_paths(&mut self, paths: Vec<PathBuf>) {
        let added_total = self.shelf.add_paths(&paths);
        self.shelf.drag_hover = false;
        self.shelf_auto_opened_by_drag = false;
        if added_total > 0 {
            save_shelf_state(&self.shelf);
            self.config.visible = true;
            self.hovered_action = None;
            self.apply_target_and_render();
        } else {
            self.render_current_frame();
        }
    }
}
