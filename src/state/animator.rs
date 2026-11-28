use std::time::Instant;

use super::{
    AppConfig, IslandMode, MAX_AUDIO_DEVICES_VISIBLE, MAX_SURFACE_HEIGHT_DIP,
    MAX_SURFACE_WIDTH_DIP, PillRectDip, PixelRect, TOP_MARGIN_DIP, audio_devices_height_for_count,
};

pub fn target_top_for_mode(mode: IslandMode) -> f32 {
    match mode {
        IslandMode::Hidden | IslandMode::Slim => 0.0,
        IslandMode::Compact => 2.0,
        IslandMode::Expanded
        | IslandMode::AntiExpanded
        | IslandMode::Settings
        | IslandMode::AudioDevices
        | IslandMode::Notice => TOP_MARGIN_DIP,
    }
}

pub fn target_dimensions_for_mode(mode: IslandMode) -> (f32, f32, f32, f32) {
    match mode {
        IslandMode::Hidden => (116.0, 4.0, 2.0, 0.0),
        IslandMode::Slim => (152.0, 16.0, 8.0, 1.0),
        IslandMode::Compact => (338.0, 40.0, 20.0, 1.0),
        IslandMode::Expanded => (424.0, 160.0, 28.0, 1.0),
        IslandMode::AntiExpanded => (424.0, 154.0, 28.0, 1.0),
        IslandMode::Settings => (424.0, 436.0, 26.0, 1.0),
        IslandMode::AudioDevices => (
            424.0,
            audio_devices_height_for_count(MAX_AUDIO_DEVICES_VISIBLE),
            26.0,
            1.0,
        ),
        IslandMode::Notice => (416.0, 136.0, 26.0, 1.0),
    }
}

pub fn target_dimensions_for_state(
    mode: IslandMode,
    audio_device_count: usize,
) -> (f32, f32, f32, f32) {
    if mode == IslandMode::AudioDevices {
        (424.0, audio_devices_height_for_count(audio_device_count), 26.0, 1.0)
    } else {
        target_dimensions_for_mode(mode)
    }
}

pub fn effective_mode(
    requested: IslandMode,
    config: AppConfig,
    fullscreen_active: bool,
    has_media: bool,
    hud_active: bool,
) -> IslandMode {
    if !config.visible {
        return IslandMode::Hidden;
    }
    if config.auto_hide_fullscreen && fullscreen_active {
        return IslandMode::Hidden;
    }
    if hud_active && matches!(requested, IslandMode::Slim | IslandMode::Compact) {
        return IslandMode::Compact;
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
        (self.target - self.value).abs() < 0.12 && self.velocity.abs() < 0.45
    }

    /// Cập nhật lò xo tắt dần với bước con cố định để bảo toàn ổn định ở mọi tần số quét.
    pub fn step(&mut self, dt_sec: f32, stiffness: f32, damping: f32) -> bool {
        let clamped_dt = dt_sec.clamp(0.0, 0.08);
        if clamped_dt <= 0.0 {
            return !self.is_settled();
        }

        const MAX_SUBSTEP: f32 = 1.0 / 240.0;
        let steps = ((clamped_dt / MAX_SUBSTEP).ceil() as usize).clamp(1, 24);
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
    pub top: SpringScalar,
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
        let top = target_top_for_mode(initial_mode);
        let content_a = if initial_mode == IslandMode::Hidden {
            0.0
        } else {
            1.0
        };
        Self {
            target_mode: initial_mode,
            top: SpringScalar::new(top),
            width: SpringScalar::new(w),
            height: SpringScalar::new(h),
            corner_radius: SpringScalar::new(r),
            opacity: SpringScalar::new(a),
            content_alpha: SpringScalar::new(content_a),
            last_step: None,
        }
    }

    #[cfg(test)]
    pub fn set_target_mode(&mut self, mode: IslandMode) {
        self.set_target_mode_with_audio_count(mode, MAX_AUDIO_DEVICES_VISIBLE);
    }

    pub fn set_target_mode_with_audio_count(
        &mut self,
        mode: IslandMode,
        audio_device_count: usize,
    ) {
        let (w, h, r, a) = target_dimensions_for_state(mode, audio_device_count);
        if self.target_mode == mode {
            if mode == IslandMode::AudioDevices && (self.height.target - h).abs() >= 0.01 {
                self.height.set_target(h);
            }
            return;
        }
        let was_hidden = self.target_mode == IslandMode::Hidden;
        self.target_mode = mode;

        let top = target_top_for_mode(mode);
        if was_hidden && mode != IslandMode::Hidden {
            // Đánh thức từ trạng thái ẩn: đặt kích thước khởi điểm mảnh sát mép trên để bung ra mượt.
            self.top.snap(0.0);
            self.width.snap(116.0);
            self.height.snap(6.0);
            self.corner_radius.snap(3.0);
            self.opacity.snap(0.0);
            self.content_alpha.snap(0.0);
        }

        self.top.set_target(top);
        self.width.set_target(w);
        self.height.set_target(h);
        self.corner_radius.set_target(r);
        self.opacity.set_target(a);
        self.content_alpha
            .set_target(if mode == IslandMode::Hidden { 0.0 } else { 1.0 });
    }

    pub fn is_animating(&self) -> bool {
        !(self.top.is_settled()
            && self.width.is_settled()
            && self.height.is_settled()
            && self.corner_radius.is_settled()
            && self.opacity.is_settled()
            && self.content_alpha.is_settled())
    }

    pub fn step(&mut self, now: Instant) -> bool {
        let dt = match self.last_step {
            Some(prev) => now.saturating_duration_since(prev).as_secs_f32(),
            None => 1.0 / 144.0,
        };
        self.last_step = Some(now);

        // `top` tắt dần tới hạn (không vọt âm đập cạnh trên), kích thước nảy tinh tế dứt khoát.
        let moving_t = self.top.step(dt, 380.0, 39.0);
        let moving_w = self.width.step(dt, 380.0, 34.0);
        let moving_h = self.height.step(dt, 360.0, 33.0);
        let moving_r = self.corner_radius.step(dt, 360.0, 34.0);
        let moving_o = self.opacity.step(dt, 420.0, 41.0);
        let moving_c = self.content_alpha.step(dt, 340.0, 36.0);

        let active = moving_t || moving_w || moving_h || moving_r || moving_o || moving_c;
        if !active {
            self.last_step = None;
        }
        active
    }

    pub fn current_pill(&self) -> PillRectDip {
        let top = self.top.value.clamp(0.0, 24.0);
        let width = self.width.value.clamp(24.0, MAX_SURFACE_WIDTH_DIP - 8.0);
        let height = self.height.value.clamp(2.0, MAX_SURFACE_HEIGHT_DIP - 12.0);
        let max_radius = (height * 0.5).max(1.0);
        let corner_radius = self.corner_radius.value.clamp(1.0, max_radius);
        let left = (MAX_SURFACE_WIDTH_DIP - width) * 0.5;
        PillRectDip {
            left,
            top,
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
