use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IslandMode {
    Hidden,
    Slim,
    Compact,
    Expanded,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiAction {
    OpenSettings,
    BackToMedia,
    MediaPrev,
    MediaPlayPause,
    MediaNext,
    SeekTimeline,
    ToggleAutoHideIdle,
    ToggleAutoHideFullscreen,
    ToggleVisualizer,
    ToggleAutostart,
    CycleMonitor,
    HideIsland,
    ExitApp,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiRectDip {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl UiRectDip {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }

    pub fn width(&self) -> f32 {
        (self.right - self.left).max(0.0)
    }

    pub fn height(&self) -> f32 {
        (self.bottom - self.top).max(0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiHitbox {
    pub rect: UiRectDip,
    pub action: UiAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimelineState {
    pub available: bool,
    pub start_ticks: i64,
    pub end_ticks: i64,
    pub base_position_ticks: i64,
    pub last_updated_filetime_ticks: i64,
}

impl TimelineState {
    pub fn duration_ticks(&self) -> i64 {
        (self.end_ticks - self.start_ticks).max(0)
    }

    pub fn current_position_ticks(&self, now_filetime_ticks: i64, is_playing: bool) -> i64 {
        if !self.available || self.duration_ticks() <= 0 {
            return 0;
        }
        let elapsed = if is_playing && self.last_updated_filetime_ticks > 0 {
            (now_filetime_ticks - self.last_updated_filetime_ticks).max(0)
        } else {
            0
        };
        (self.base_position_ticks + elapsed).clamp(self.start_ticks, self.end_ticks)
    }

    pub fn progress_ratio(&self, now_filetime_ticks: i64, is_playing: bool) -> f32 {
        let dur = self.duration_ticks();
        if !self.available || dur <= 0 {
            return 0.0;
        }
        let pos = self.current_position_ticks(now_filetime_ticks, is_playing);
        let rel = (pos - self.start_ticks).max(0) as f64 / dur as f64;
        (rel as f32).clamp(0.0, 1.0)
    }

    pub fn seek_ticks_for_ratio(&self, ratio: f32) -> Option<i64> {
        let dur = self.duration_ticks();
        if !self.available || dur <= 0 {
            return None;
        }
        let clamped = ratio.clamp(0.0, 1.0) as f64;
        Some(self.start_ticks + (dur as f64 * clamped).round() as i64)
    }
}

pub fn current_windows_filetime_ticks() -> i64 {
    const UNIX_TO_FILETIME_TICKS: i64 = 116_444_736_000_000_000;
    let Ok(dur) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) else {
        return UNIX_TO_FILETIME_TICKS;
    };
    let unix_ticks = (dur.as_secs() as i64)
        .saturating_mul(10_000_000)
        .saturating_add((dur.subsec_nanos() / 100) as i64);
    UNIX_TO_FILETIME_TICKS.saturating_add(unix_ticks)
}

pub fn format_duration_ticks(ticks: i64) -> String {
    let total_secs = (ticks / 10_000_000).max(0);
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    if hours > 0 {
        format!("{hours}:{mins:02}:{secs:02}")
    } else {
        format!("{mins:02}:{secs:02}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaDisplayState {
    pub has_session: bool,
    pub is_playing: bool,
    pub title: String,
    pub artist: String,
    pub source_app: String,
    pub timeline: TimelineState,
}

impl Default for MediaDisplayState {
    fn default() -> Self {
        Self {
            has_session: false,
            is_playing: false,
            title: String::from("Chưa có phương tiện đang phát"),
            artist: String::from("Mở nhạc trên trình duyệt hoặc ứng dụng phát"),
            source_app: String::new(),
            timeline: TimelineState::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppConfig {
    pub visible: bool,
    pub auto_hide_when_idle: bool,
    pub auto_hide_fullscreen: bool,
    pub visualizer_enabled: bool,
    pub autostart: bool,
    pub monitor_index: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            visible: true,
            auto_hide_when_idle: false,
            auto_hide_fullscreen: true,
            visualizer_enabled: true,
            autostart: false,
            monitor_index: 0,
        }
    }
}

/// Kích thước cố định tối đa của bề mặt SwapChain (DIP) đủ chứa biên nảy lò xo ở chế độ Settings.
pub const MAX_SURFACE_WIDTH_DIP: f32 = 464.0;
pub const MAX_SURFACE_HEIGHT_DIP: f32 = 264.0;
pub const TOP_MARGIN_DIP: f32 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PillRectDip {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
    pub corner_radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

pub fn target_dimensions_for_mode(mode: IslandMode) -> (f32, f32, f32, f32) {
    match mode {
        IslandMode::Hidden => (116.0, 4.0, 2.0, 0.0),
        IslandMode::Slim => (196.0, 30.0, 15.0, 1.0),
        IslandMode::Compact => (276.0, 36.0, 18.0, 1.0),
        IslandMode::Expanded => (416.0, 160.0, 28.0, 1.0),
        IslandMode::Settings => (424.0, 232.0, 26.0, 1.0),
    }
}

pub fn effective_mode(
    requested: IslandMode,
    config: AppConfig,
    fullscreen_active: bool,
    has_media: bool,
) -> IslandMode {
    if !config.visible {
        return IslandMode::Hidden;
    }
    if config.auto_hide_fullscreen && fullscreen_active {
        return IslandMode::Hidden;
    }
    if config.auto_hide_when_idle && !has_media && requested == IslandMode::Slim {
        return IslandMode::Hidden;
    }
    requested
}

#[derive(Debug, Clone, Copy)]
pub struct SpringScalar {
    pub value: f32,
    pub velocity: f32,
    pub target: f32,
}

impl SpringScalar {
    pub fn new(initial: f32) -> Self {
        Self {
            value: initial,
            velocity: 0.0,
            target: initial,
        }
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    pub fn snap(&mut self, value: f32) {
        self.value = value;
        self.velocity = 0.0;
        self.target = value;
    }

    pub fn is_settled(&self) -> bool {
        (self.target - self.value).abs() < 0.08 && self.velocity.abs() < 0.25
    }

    /// Cập nhật lò xo tắt dần với bước con cố định để bảo toàn ổn định ở mọi tần số quét.
    pub fn step(&mut self, dt_sec: f32, stiffness: f32, damping: f32) -> bool {
        let clamped_dt = dt_sec.clamp(0.0, 0.05);
        if clamped_dt <= 0.0 {
            return !self.is_settled();
        }

        const MAX_SUBSTEP: f32 = 1.0 / 240.0;
        let steps = ((clamped_dt / MAX_SUBSTEP).ceil() as usize).clamp(1, 16);
        let sub_dt = clamped_dt / steps as f32;

        for _ in 0..steps {
            let accel = stiffness * (self.target - self.value) - damping * self.velocity;
            self.velocity += accel * sub_dt;
            self.value += self.velocity * sub_dt;
        }

        if self.is_settled() {
            self.value = self.target;
            self.velocity = 0.0;
            false
        } else {
            true
        }
    }
}

pub struct IslandAnimator {
    pub target_mode: IslandMode,
    pub width: SpringScalar,
    pub height: SpringScalar,
    pub corner_radius: SpringScalar,
    pub opacity: SpringScalar,
    pub content_alpha: SpringScalar,
    last_step: Option<Instant>,
}

impl IslandAnimator {
    pub fn new(initial_mode: IslandMode) -> Self {
        let (w, h, r, a) = target_dimensions_for_mode(initial_mode);
        let content_a = if initial_mode == IslandMode::Hidden {
            0.0
        } else {
            1.0
        };
        Self {
            target_mode: initial_mode,
            width: SpringScalar::new(w),
            height: SpringScalar::new(h),
            corner_radius: SpringScalar::new(r),
            opacity: SpringScalar::new(a),
            content_alpha: SpringScalar::new(content_a),
            last_step: None,
        }
    }

    pub fn set_target_mode(&mut self, mode: IslandMode) {
        if self.target_mode == mode {
            return;
        }
        let was_hidden = self.target_mode == IslandMode::Hidden;
        self.target_mode = mode;

        let (w, h, r, a) = target_dimensions_for_mode(mode);
        if was_hidden && mode != IslandMode::Hidden {
            // Đánh thức từ trạng thái ẩn: đặt kích thước khởi điểm mảnh sát mép trên để bung ra mượt.
            self.width.snap(116.0);
            self.height.snap(6.0);
            self.corner_radius.snap(3.0);
            self.opacity.snap(0.0);
            self.content_alpha.snap(0.0);
        }

        self.width.set_target(w);
        self.height.set_target(h);
        self.corner_radius.set_target(r);
        self.opacity.set_target(a);
        if !was_hidden && mode != IslandMode::Hidden {
            self.content_alpha.snap(0.2);
        }
        self.content_alpha
            .set_target(if mode == IslandMode::Hidden { 0.0 } else { 1.0 });

        if self.last_step.is_none() {
            self.last_step = Some(Instant::now());
        }
    }

    pub fn is_animating(&self) -> bool {
        !(self.width.is_settled()
            && self.height.is_settled()
            && self.corner_radius.is_settled()
            && self.opacity.is_settled()
            && self.content_alpha.is_settled())
    }

    pub fn step(&mut self, now: Instant) -> bool {
        let dt = match self.last_step {
            Some(prev) => now.saturating_duration_since(prev).as_secs_f32(),
            None => 1.0 / 120.0,
        };
        self.last_step = Some(now);

        // Lò xo kích thước nảy nhẹ chuẩn Dynamic Island (~zeta ~ 0.78).
        let moving_w = self.width.step(dt, 330.0, 28.5);
        let moving_h = self.height.step(dt, 310.0, 27.5);
        let moving_r = self.corner_radius.step(dt, 320.0, 29.0);
        let moving_o = self.opacity.step(dt, 380.0, 36.0);
        let moving_c = self.content_alpha.step(dt, 280.0, 32.0);

        let active = moving_w || moving_h || moving_r || moving_o || moving_c;
        if !active {
            self.last_step = None;
        }
        active
    }

    pub fn current_pill(&self) -> PillRectDip {
        let width = self.width.value.clamp(24.0, MAX_SURFACE_WIDTH_DIP - 8.0);
        let height = self.height.value.clamp(2.0, MAX_SURFACE_HEIGHT_DIP - 12.0);
        let max_radius = (height * 0.5).max(1.0);
        let corner_radius = self.corner_radius.value.clamp(1.0, max_radius);
        let left = (MAX_SURFACE_WIDTH_DIP - width) * 0.5;
        PillRectDip {
            left,
            top: TOP_MARGIN_DIP,
            width,
            height,
            corner_radius,
        }
    }

    pub fn should_hide_window(&self) -> bool {
        self.target_mode == IslandMode::Hidden && !self.is_animating()
    }
}

pub fn dip_to_px_round(dip: f32, dpi_scale: f32) -> i32 {
    (dip * dpi_scale).round().max(0.0) as i32
}

pub fn compute_surface_size_px(dpi_scale: f32) -> (i32, i32) {
    let w = dip_to_px_round(MAX_SURFACE_WIDTH_DIP, dpi_scale).max(1);
    let h = dip_to_px_round(MAX_SURFACE_HEIGHT_DIP, dpi_scale).max(1);
    (w, h)
}

pub fn compute_window_origin_px(
    monitor_left: i32,
    monitor_right: i32,
    work_top: i32,
    surface_width_px: i32,
) -> (i32, i32) {
    let monitor_width = (monitor_right - monitor_left).max(0);
    let x = monitor_left + (monitor_width - surface_width_px) / 2;
    let y = work_top;
    (x, y)
}

/// Tính vùng cắt Win32 (`SetWindowRgn`) theo pixel, nới dư 2 px để giữ nguyên khử răng cưa Direct2D.
pub fn compute_pill_region_px(pill: PillRectDip, dpi_scale: f32) -> (PixelRect, i32) {
    const AA_PAD_PX: i32 = 2;
    let left = (pill.left * dpi_scale).floor() as i32 - AA_PAD_PX;
    let top = (pill.top * dpi_scale).floor() as i32 - AA_PAD_PX;
    let right = ((pill.left + pill.width) * dpi_scale).ceil() as i32 + AA_PAD_PX;
    let bottom = ((pill.top + pill.height) * dpi_scale).ceil() as i32 + AA_PAD_PX;
    let diameter = ((pill.corner_radius * 2.0 * dpi_scale).round() as i32 + AA_PAD_PX * 2).max(2);
    (
        PixelRect {
            left: left.max(0),
            top: top.max(0),
            right: right.max(left.max(0) + 1),
            bottom: bottom.max(top.max(0) + 1),
        },
        diameter,
    )
}

/// Bố cục nút bấm dùng chung giữa lớp vẽ (`renderer.rs`) và xử lý chuột (`window.rs`).
pub fn fill_interactive_hitboxes(pill: PillRectDip, mode: IslandMode, out: &mut Vec<UiHitbox>) {
    out.clear();
    match mode {
        IslandMode::Expanded => fill_expanded_hitboxes(pill, out),
        IslandMode::Settings => fill_settings_hitboxes(pill, out),
        _ => {}
    }
}

fn fill_expanded_hitboxes(pill: PillRectDip, out: &mut Vec<UiHitbox>) {
    let settings_btn = UiRectDip {
        left: pill.left + pill.width - 74.0,
        top: pill.top + 14.0,
        right: pill.left + pill.width - 14.0,
        bottom: pill.top + 40.0,
    };

    let timeline_track = UiRectDip {
        left: pill.left + 60.0,
        top: pill.top + 88.0,
        right: pill.left + pill.width - 60.0,
        bottom: pill.top + 108.0,
    };

    let controls_bottom = pill.top + pill.height - 10.0;
    let center_x = pill.left + pill.width * 0.5;

    let prev_btn = UiRectDip {
        left: center_x - 68.0,
        top: controls_bottom - 36.0,
        right: center_x - 34.0,
        bottom: controls_bottom - 2.0,
    };
    let play_btn = UiRectDip {
        left: center_x - 19.0,
        top: controls_bottom - 38.0,
        right: center_x + 19.0,
        bottom: controls_bottom,
    };
    let next_btn = UiRectDip {
        left: center_x + 34.0,
        top: controls_bottom - 36.0,
        right: center_x + 68.0,
        bottom: controls_bottom - 2.0,
    };

    out.extend_from_slice(&[
        UiHitbox {
            rect: settings_btn,
            action: UiAction::OpenSettings,
        },
        UiHitbox {
            rect: timeline_track,
            action: UiAction::SeekTimeline,
        },
        UiHitbox {
            rect: prev_btn,
            action: UiAction::MediaPrev,
        },
        UiHitbox {
            rect: play_btn,
            action: UiAction::MediaPlayPause,
        },
        UiHitbox {
            rect: next_btn,
            action: UiAction::MediaNext,
        },
    ]);
}

fn fill_settings_hitboxes(pill: PillRectDip, out: &mut Vec<UiHitbox>) {
    let pad_x = 16.0;
    let gap = 8.0;
    let inner_left = pill.left + pad_x;
    let inner_right = pill.left + pill.width - pad_x;
    let col_w = ((inner_right - inner_left - gap) * 0.5).max(80.0);

    let back_btn = UiRectDip {
        left: inner_right - 74.0,
        top: pill.top + 12.0,
        right: inner_right,
        bottom: pill.top + 36.0,
    };

    let row1_top = pill.top + 46.0;
    let row_h = 38.0;
    let row2_top = row1_top + row_h + gap;
    let row3_top = row2_top + row_h + gap;
    let bottom_h = 34.0;
    let footer_top = row3_top + row_h + 10.0;

    out.extend_from_slice(&[
        UiHitbox {
            rect: back_btn,
            action: UiAction::BackToMedia,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left,
                top: row1_top,
                right: inner_left + col_w,
                bottom: row1_top + row_h,
            },
            action: UiAction::ToggleAutoHideIdle,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left + col_w + gap,
                top: row1_top,
                right: inner_right,
                bottom: row1_top + row_h,
            },
            action: UiAction::ToggleAutoHideFullscreen,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left,
                top: row2_top,
                right: inner_left + col_w,
                bottom: row2_top + row_h,
            },
            action: UiAction::ToggleVisualizer,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left + col_w + gap,
                top: row2_top,
                right: inner_right,
                bottom: row2_top + row_h,
            },
            action: UiAction::ToggleAutostart,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left,
                top: row3_top,
                right: inner_right,
                bottom: row3_top + row_h,
            },
            action: UiAction::CycleMonitor,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left,
                top: footer_top,
                right: inner_left + col_w,
                bottom: footer_top + bottom_h,
            },
            action: UiAction::HideIsland,
        },
        UiHitbox {
            rect: UiRectDip {
                left: inner_left + col_w + gap,
                top: footer_top,
                right: inner_right,
                bottom: footer_top + bottom_h,
            },
            action: UiAction::ExitApp,
        },
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_converges_and_stays_stable_on_large_dt_spike() {
        let mut spring = SpringScalar::new(156.0);
        spring.set_target(424.0);
        // Giả lập frame bị trễ nặng 1.0 giây vẫn không được nổ giá trị.
        let _ = spring.step(1.0, 330.0, 28.5);
        assert!(spring.value.is_finite());
        assert!(spring.value > 156.0 && spring.value < 600.0);

        for _ in 0..240 {
            if !spring.step(1.0 / 120.0, 330.0, 28.5) {
                break;
            }
        }
        assert!(spring.is_settled());
        assert_eq!(spring.value, 424.0);
        assert_eq!(spring.velocity, 0.0);
    }

    #[test]
    fn spring_preserves_velocity_when_interrupted() {
        let mut spring = SpringScalar::new(156.0);
        spring.set_target(412.0);
        for _ in 0..8 {
            spring.step(1.0 / 120.0, 330.0, 28.5);
        }
        let mid_val = spring.value;
        let mid_vel = spring.velocity;
        assert!(mid_vel > 0.0);

        spring.set_target(156.0);
        assert_eq!(spring.value, mid_val);
        assert_eq!(spring.velocity, mid_vel);
    }

    #[test]
    fn animator_stops_after_reaching_target() {
        let mut anim = IslandAnimator::new(IslandMode::Slim);
        assert!(!anim.is_animating());

        anim.set_target_mode(IslandMode::Expanded);
        assert!(anim.is_animating());

        let mut now = Instant::now();
        let mut ticks = 0;
        while ticks < 300 {
            now += std::time::Duration::from_millis(8);
            ticks += 1;
            if !anim.step(now) {
                break;
            }
        }
        assert!(!anim.is_animating());
        let pill = anim.current_pill();
        assert!((pill.width - 416.0).abs() < 0.5);
        assert!((pill.height - 160.0).abs() < 0.5);
    }

    #[test]
    fn settings_hitboxes_stay_inside_pill_and_do_not_overlap() {
        let mut anim = IslandAnimator::new(IslandMode::Settings);
        let pill = anim.current_pill();
        let mut hitboxes = Vec::with_capacity(8);
        fill_interactive_hitboxes(pill, IslandMode::Settings, &mut hitboxes);
        assert_eq!(hitboxes.len(), 8);

        for hb in &hitboxes {
            assert!(hb.rect.left >= pill.left);
            assert!(hb.rect.right <= pill.left + pill.width);
            assert!(hb.rect.top >= pill.top);
            assert!(hb.rect.bottom <= pill.top + pill.height);
            assert!(hb.rect.width() > 30.0);
            assert!(hb.rect.height() >= 24.0);
        }

        for i in 0..hitboxes.len() {
            for j in (i + 1)..hitboxes.len() {
                let a = hitboxes[i].rect;
                let b = hitboxes[j].rect;
                let overlap_x = a.left < b.right && a.right > b.left;
                let overlap_y = a.top < b.bottom && a.bottom > b.top;
                assert!(
                    !(overlap_x && overlap_y),
                    "Hitbox {i} va {j} bi chong cheo: {:?} vs {:?}",
                    a,
                    b
                );
            }
        }
        anim.set_target_mode(IslandMode::Slim);
    }

    #[test]
    fn expanded_hitboxes_stay_inside_pill_and_do_not_overlap() {
        let anim = IslandAnimator::new(IslandMode::Expanded);
        let pill = anim.current_pill();
        let mut hitboxes = Vec::with_capacity(5);
        fill_interactive_hitboxes(pill, IslandMode::Expanded, &mut hitboxes);
        assert_eq!(hitboxes.len(), 5);

        for hb in &hitboxes {
            assert!(hb.rect.left >= pill.left);
            assert!(hb.rect.right <= pill.left + pill.width);
            assert!(hb.rect.top >= pill.top);
            assert!(hb.rect.bottom <= pill.top + pill.height);
        }

        for i in 0..hitboxes.len() {
            for j in (i + 1)..hitboxes.len() {
                let a = hitboxes[i].rect;
                let b = hitboxes[j].rect;
                let overlap_x = a.left < b.right && a.right > b.left;
                let overlap_y = a.top < b.bottom && a.bottom > b.top;
                assert!(!(overlap_x && overlap_y));
            }
        }
    }

    #[test]
    fn timeline_progress_and_formatting_work_accurately() {
        let tl = TimelineState {
            available: true,
            start_ticks: 0,
            end_ticks: 250 * 10_000_000,
            base_position_ticks: 60 * 10_000_000,
            last_updated_filetime_ticks: 1_000_000_000,
        };
        let now = 1_000_000_000 + 24 * 10_000_000;
        assert_eq!(tl.current_position_ticks(now, true), 84 * 10_000_000);
        assert_eq!(tl.current_position_ticks(now, false), 60 * 10_000_000);
        assert_eq!(format_duration_ticks(84 * 10_000_000), "01:24");
        assert_eq!(format_duration_ticks(3665 * 10_000_000), "1:01:05");
        assert_eq!(tl.seek_ticks_for_ratio(0.5), Some(125 * 10_000_000));
    }

    #[test]
    fn center_window_on_primary_and_negative_secondary_monitors() {
        let (sw, _sh) = compute_surface_size_px(1.0);
        let (x, y) = compute_window_origin_px(0, 1920, 0, sw);
        assert_eq!(x, (1920 - sw) / 2);
        assert_eq!(y, 0);

        let (sw150, _) = compute_surface_size_px(1.5);
        let (x2, y2) = compute_window_origin_px(-1920, 0, 32, sw150);
        assert_eq!(x2, -1920 + (1920 - sw150) / 2);
        assert_eq!(y2, 32);
    }

    #[test]
    fn effective_mode_respects_fullscreen_visibility_and_auto_hide() {
        let cfg = AppConfig::default();
        assert_eq!(
            effective_mode(IslandMode::Slim, cfg, false, false),
            IslandMode::Slim
        );
        assert_eq!(
            effective_mode(IslandMode::Expanded, cfg, true, true),
            IslandMode::Hidden
        );

        let mut auto_idle = cfg;
        auto_idle.auto_hide_when_idle = true;
        assert_eq!(
            effective_mode(IslandMode::Slim, auto_idle, false, false),
            IslandMode::Hidden
        );
        assert_eq!(
            effective_mode(IslandMode::Compact, auto_idle, false, true),
            IslandMode::Compact
        );
    }
}
