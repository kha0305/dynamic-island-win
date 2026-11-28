use std::mem::size_of;
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Gdi::{CreateBitmap, DeleteObject, HGDIOBJ};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconIndirect, CreatePopupMenu, DestroyIcon, DestroyMenu, GetCursorPos,
    HICON, HMENU, ICONINFO, MF_CHECKED, MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING, MF_UNCHECKED,
    PostMessageW, SetForegroundWindow, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RIGHTBUTTON,
    TrackPopupMenu, WM_APP, WM_NULL,
};
use windows::core::PCWSTR;

use crate::hud::AudioOutputDevice;
use crate::state::{AppConfig, MediaDisplayState};
use crate::updater::APP_VERSION;
use crate::watcher::DisplayMonitor;

pub const WM_APP_TRAY: u32 = WM_APP + 1;
pub const TRAY_ICON_ID: u32 = 1;

pub const IDM_TOGGLE_VISIBLE: usize = 1001;
pub const IDM_OPEN_SETTINGS: usize = 1002;
pub const IDM_TOGGLE_EXPANDED: usize = 1003;
pub const IDM_TOGGLE_AUTO_HIDE: usize = 1004;
pub const IDM_TOGGLE_FULLSCREEN: usize = 1005;
pub const IDM_TOGGLE_VISUALIZER: usize = 1006;
pub const IDM_TOGGLE_AUTOSTART: usize = 1007;
pub const IDM_CHECK_UPDATE: usize = 1008;
pub const IDM_TOGGLE_NOTCH_COLLAPSE: usize = 1009;
pub const IDM_TOGGLE_HUD_VOLUME: usize = 1010;
pub const IDM_TOGGLE_HUD_WHEEL: usize = 1011;
pub const IDM_TOGGLE_HUD_POWER: usize = 1012;
pub const IDM_EXIT: usize = 1099;
pub const IDM_MONITOR_BASE: usize = 2000;
pub const IDM_MONITOR_MAX: usize = 2031;
pub const IDM_AUDIO_DEVICE_BASE: usize = 2100;
pub const IDM_AUDIO_DEVICE_MAX: usize = 2131;

pub struct TrayIcon {
    hwnd: HWND,
    icon: HICON,
    added: bool,
}

impl TrayIcon {
    pub fn new(hwnd: HWND, tooltip: &str) -> windows::core::Result<Self> {
        let icon = create_pill_tray_icon()?;
        let mut nid = build_notify_icon_data(hwnd, icon, tooltip);
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;

        let ok = unsafe { Shell_NotifyIconW(NIM_ADD, &nid) }.as_bool();
        if !ok {
            unsafe {
                let _ = DestroyIcon(icon);
            }
            return Err(windows::core::Error::from_thread());
        }

        Ok(Self {
            hwnd,
            icon,
            added: true,
        })
    }

    pub fn update_tooltip(&self, tooltip: &str) {
        if !self.added {
            return;
        }
        let mut nid = build_notify_icon_data(self.hwnd, self.icon, tooltip);
        nid.uFlags = NIF_TIP | NIF_SHOWTIP;
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    pub fn show_context_menu(
        &self,
        config: AppConfig,
        media: &MediaDisplayState,
        monitors: &[DisplayMonitor],
        audio_devices: &[AudioOutputDevice],
        is_expanded: bool,
        available_update: Option<&str>,
    ) {
        let Ok(menu) = (unsafe { CreatePopupMenu() }) else {
            return;
        };
        let _menu_guard = MenuGuard(menu);

        let header = if media.has_session {
            let state_word = if media.is_playing {
                "Đang phát"
            } else {
                "Tạm dừng"
            };
            format!("Win Dynamic Island - {state_word}")
        } else {
            String::from("Win Dynamic Island - Sẵn sàng")
        };
        append_disabled_header(menu, &header);
        let _ = unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()) };

        let toggle_label = if config.visible {
            "Ẩn Dynamic Island"
        } else {
            "Hiện Dynamic Island"
        };
        append_menu_item(menu, IDM_TOGGLE_VISIBLE, toggle_label, false);
        append_menu_item(menu, IDM_OPEN_SETTINGS, "Mở bảng điều khiển nhanh", false);
        let expand_label = if is_expanded {
            if config.notch_collapse_after_4s || (config.auto_hide_when_idle && !media.has_session)
            {
                "Thu gọn thành thanh mảnh"
            } else {
                "Thu gọn viên thuốc"
            }
        } else {
            "Mở rộng thẻ phát nhạc"
        };
        append_menu_item(menu, IDM_TOGGLE_EXPANDED, expand_label, false);

        let _ = unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()) };
        append_menu_item(
            menu,
            IDM_TOGGLE_NOTCH_COLLAPSE,
            "Thu gọn mép trên sau 4 giây",
            config.notch_collapse_after_4s,
        );
        append_menu_item(
            menu,
            IDM_TOGGLE_AUTO_HIDE,
            "Tự ẩn khi rảnh",
            config.auto_hide_when_idle,
        );
        append_menu_item(
            menu,
            IDM_TOGGLE_FULLSCREEN,
            "Tự ẩn khi toàn màn hình",
            config.auto_hide_fullscreen,
        );
        append_menu_item(
            menu,
            IDM_TOGGLE_VISUALIZER,
            "Hiệu ứng sóng nhạc động",
            config.visualizer_enabled,
        );
        append_menu_item(
            menu,
            IDM_TOGGLE_HUD_VOLUME,
            "HUD Âm lượng",
            config.hud_volume_enabled,
        );
        append_menu_item(
            menu,
            IDM_TOGGLE_HUD_WHEEL,
            "Cuộn chuột trên pill chỉnh loa",
            config.hud_wheel_enabled,
        );
        append_menu_item(
            menu,
            IDM_TOGGLE_HUD_POWER,
            "HUD Pin / Sạc",
            config.hud_power_enabled,
        );

        let _ = unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()) };
        if let Ok(sub_monitors) = unsafe { CreatePopupMenu() } {
            let active_idx = if monitors.is_empty() {
                0
            } else {
                config.monitor_index % monitors.len()
            };
            for (idx, mon) in monitors.iter().enumerate() {
                if IDM_MONITOR_BASE + idx > IDM_MONITOR_MAX {
                    break;
                }
                append_menu_item(
                    sub_monitors,
                    IDM_MONITOR_BASE + idx,
                    &mon.label,
                    idx == active_idx,
                );
            }
            let sub_title = to_wide_null("Màn hình hiển thị");
            if unsafe {
                AppendMenuW(
                    menu,
                    MF_POPUP,
                    sub_monitors.0 as usize,
                    PCWSTR(sub_title.as_ptr()),
                )
            }
            .is_err()
            {
                unsafe {
                    let _ = DestroyMenu(sub_monitors);
                }
            }
        }
        if let Ok(sub_audio) = unsafe { CreatePopupMenu() } {
            if audio_devices.is_empty() {
                append_disabled_header(sub_audio, "Chưa có thiết bị phát");
            } else {
                for (idx, dev) in audio_devices.iter().enumerate() {
                    if IDM_AUDIO_DEVICE_BASE + idx > IDM_AUDIO_DEVICE_MAX {
                        break;
                    }
                    append_menu_item(
                        sub_audio,
                        IDM_AUDIO_DEVICE_BASE + idx,
                        &dev.name,
                        dev.is_default,
                    );
                }
            }
            let audio_title = to_wide_null("Thiết bị phát âm thanh");
            if unsafe {
                AppendMenuW(
                    menu,
                    MF_POPUP,
                    sub_audio.0 as usize,
                    PCWSTR(audio_title.as_ptr()),
                )
            }
            .is_err()
            {
                unsafe {
                    let _ = DestroyMenu(sub_audio);
                }
            }
        }
        append_menu_item(
            menu,
            IDM_TOGGLE_AUTOSTART,
            "Khởi động cùng Windows",
            config.autostart,
        );
        let update_label = match available_update {
            Some(ver) => format!("Cập nhật lên {ver} ngay"),
            None => format!("Kiểm tra cập nhật (v{APP_VERSION})"),
        };
        append_menu_item(menu, IDM_CHECK_UPDATE, &update_label, false);

        let _ = unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()) };
        append_menu_item(menu, IDM_EXIT, "Thoát ứng dụng", false);

        let mut pt = POINT::default();
        if unsafe { GetCursorPos(&mut pt) }.is_err() {
            return;
        }

        unsafe {
            let _ = SetForegroundWindow(self.hwnd);
            let _ = TrackPopupMenu(
                menu,
                TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RIGHTBUTTON,
                pt.x,
                pt.y,
                None,
                self.hwnd,
                None,
            );
            let _ = PostMessageW(Some(self.hwnd), WM_NULL, Default::default(), Default::default());
        }
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        if self.added {
            let nid = build_notify_icon_data(self.hwnd, self.icon, "");
            unsafe {
                let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            }
            self.added = false;
        }
        if !self.icon.is_invalid() {
            unsafe {
                let _ = DestroyIcon(self.icon);
            }
        }
    }
}

struct MenuGuard(HMENU);

impl Drop for MenuGuard {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = DestroyMenu(self.0);
            }
        }
    }
}

fn append_disabled_header(menu: HMENU, text: &str) {
    let wide = to_wide_null(text);
    unsafe {
        let _ = AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, PCWSTR(wide.as_ptr()));
    }
}

fn append_menu_item(menu: HMENU, id: usize, text: &str, checked: bool) {
    let wide = to_wide_null(text);
    let flags = if checked {
        MF_STRING | MF_CHECKED
    } else {
        MF_STRING | MF_UNCHECKED
    };
    unsafe {
        let _ = AppendMenuW(menu, flags, id, PCWSTR(wide.as_ptr()));
    }
}

fn build_notify_icon_data(hwnd: HWND, icon: HICON, tooltip: &str) -> NOTIFYICONDATAW {
    let mut nid = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ICON_ID,
        uCallbackMessage: WM_APP_TRAY,
        hIcon: icon,
        ..Default::default()
    };
    write_wide_slice(&mut nid.szTip, tooltip);
    nid
}

pub fn to_wide_null(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn write_wide_slice(dest: &mut [u16], text: &str) {
    dest.fill(0);
    let max_chars = dest.len().saturating_sub(1);
    for (dst, ch) in dest.iter_mut().take(max_chars).zip(text.encode_utf16()) {
        *dst = ch;
    }
}

/// Tạo biểu tượng viên thuốc 16x16 trong bộ nhớ để không phụ thuộc tệp .ico ngoài.
fn create_pill_tray_icon() -> windows::core::Result<HICON> {
    const SIZE: i32 = 16;
    let mut bgra = vec![0u8; (SIZE * SIZE * 4) as usize];
    let mut mask = vec![0u8; ((SIZE * SIZE) / 8) as usize];

    for y in 0..SIZE {
        for x in 0..SIZE {
            let inside_band = (4..=11).contains(&y);
            let dx = if x < 4 {
                4.0 - x as f32
            } else if x > 11 {
                x as f32 - 11.0
            } else {
                0.0
            };
            let dy = (y as f32) - 7.5;
            let dist = (dx * dx + dy * dy).sqrt();

            let idx = ((y * SIZE + x) * 4) as usize;
            if inside_band && dist <= 3.8 {
                let is_border = dist >= 2.7 || y == 4 || y == 11;
                if is_border {
                    bgra[idx] = 245;
                    bgra[idx + 1] = 245;
                    bgra[idx + 2] = 245;
                    bgra[idx + 3] = 255;
                } else {
                    bgra[idx] = 28;
                    bgra[idx + 1] = 24;
                    bgra[idx + 2] = 22;
                    bgra[idx + 3] = 255;
                }
            } else {
                let bit_index = (y * SIZE + x) as usize;
                mask[bit_index / 8] |= 1 << (7 - (bit_index % 8));
            }
        }
    }

    let color_bmp = unsafe { CreateBitmap(SIZE, SIZE, 1, 32, Some(bgra.as_ptr() as _)) };
    let mask_bmp = unsafe { CreateBitmap(SIZE, SIZE, 1, 1, Some(mask.as_ptr() as _)) };
    if color_bmp.is_invalid() || mask_bmp.is_invalid() {
        unsafe {
            if !color_bmp.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(color_bmp.0));
            }
            if !mask_bmp.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(mask_bmp.0));
            }
        }
        return Err(windows::core::Error::from_thread());
    }

    let info = ICONINFO {
        fIcon: true.into(),
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: mask_bmp,
        hbmColor: color_bmp,
    };

    let icon = unsafe { CreateIconIndirect(&info) };
    unsafe {
        let _ = DeleteObject(HGDIOBJ(color_bmp.0));
        let _ = DeleteObject(HGDIOBJ(mask_bmp.0));
    }
    icon
}
