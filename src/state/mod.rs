mod animator;
mod hitboxes;
#[cfg(test)]
mod tests;

pub use animator::{
    IslandAnimator, compute_pill_region_px, compute_surface_size_px, compute_window_origin_px,
    effective_mode, target_dimensions_for_mode, target_dimensions_for_state, target_top_for_mode,
};
pub use hitboxes::{
    MAX_SHELF_VISIBLE_ROWS, fill_audio_devices_hitboxes, fill_interactive_hitboxes,
    fill_notice_hitboxes, fill_shelf_hitboxes, fill_volume_hud_hitboxes, pill_contains_dip,
    settings_section_tops, shelf_contains_dip, shelf_pill_rect,
    should_close_subview_on_header_click, should_collapse_expanded_on_blank_click,
    should_collapse_shelf_on_header_click, timeline_ratio_for_x, timeline_seek_hit_bounds,
    volume_seek_hit_bounds,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IslandMode {
    Hidden,
    Slim,
    Compact,
    Expanded,
    Settings,
    AudioDevices,
    Notice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    Checking,
    UpToDate,
    UpdateAvailable,
    Downloading,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IslandNoticeView<'a> {
    pub kind: NoticeKind,
    pub title: &'a str,
    pub message: &'a str,
    pub primary_label: &'a str,
    pub secondary_label: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiAction {
    OpenSettings,
    OpenAudioDevices,
    SelectAudioDevice(usize),
    NoticePrimary,
    NoticeSecondary,
    BackToMedia,
    MediaPrev,
    MediaPlayPause,
    MediaNext,
    SeekTimeline,
    SeekVolume,
    ToggleVolumeMute,
    ToggleNotchCollapse,
    ToggleHudVolume,
    ToggleHudWheel,
    ToggleHudPower,
    ToggleAutoHideIdle,
    ToggleAutoHideFullscreen,
    ToggleVisualizer,
    ToggleAutostart,
    ToggleAntiMonitor,
    ToggleAntiHooks,
    CheckUpdate,
    CycleMonitor,
    HideIsland,
    ExitApp,
    ToggleShelf,
    ShelfToggleSelect(usize),
    ShelfToggleSelectAll,
    ShelfDeleteAll,
    ShelfDragItem(usize),
    ShelfClose,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HudContent {
    Volume {
        level: f32,
        muted: bool,
        show_device_name: bool,
    },
    Power {
        on_ac: bool,
        percent: Option<u8>,
        low_battery: bool,
    },
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
    pub notch_collapse_after_4s: bool,
    pub hud_volume_enabled: bool,
    pub hud_wheel_enabled: bool,
    pub hud_power_enabled: bool,
    pub auto_hide_when_idle: bool,
    pub auto_hide_fullscreen: bool,
    pub visualizer_enabled: bool,
    pub anti_monitor_enabled: bool,
    pub autostart: bool,
    pub monitor_index: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            visible: true,
            notch_collapse_after_4s: true,
            hud_volume_enabled: true,
            hud_wheel_enabled: true,
            hud_power_enabled: true,
            auto_hide_when_idle: false,
            auto_hide_fullscreen: true,
            visualizer_enabled: true,
            anti_monitor_enabled: true,
            autostart: false,
            monitor_index: 0,
        }
    }
}

/// Kích thước cố định tối đa của bề mặt SwapChain (DIP) đủ chứa Dynamic Island căn giữa và File Shelf cạnh phải trên màn FHD.
pub const MAX_SURFACE_WIDTH_DIP: f32 = 1040.0;
pub const MAX_SURFACE_HEIGHT_DIP: f32 = 440.0;
pub const TOP_MARGIN_DIP: f32 = 6.0;
pub const MAX_AUDIO_DEVICES_VISIBLE: usize = 5;

pub fn audio_devices_height_for_count(total_devices: usize) -> f32 {
    let visible = total_devices.clamp(1, MAX_AUDIO_DEVICES_VISIBLE);
    let list_bottom = 56.0 + visible as f32 * 37.0 + visible.saturating_sub(1) as f32 * 6.0;
    let bottom_pad = if total_devices > MAX_AUDIO_DEVICES_VISIBLE {
        28.0
    } else {
        14.0
    };
    list_bottom + bottom_pad
}

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
