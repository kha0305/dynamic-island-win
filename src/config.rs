use std::fs;
use std::path::{Path, PathBuf};

use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, REG_BINARY, REG_SZ, REG_VALUE_TYPE, RegCloseKey,
    RegOpenKeyExW, RegQueryValueExW,
};
use windows::core::PCWSTR;

#[cfg(test)]
use crate::install::format_autostart_command;
pub use crate::install::set_autostart_for_exe;
use crate::state::AppConfig;
use crate::tray::to_wide_null;

const RUN_KEY_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const STARTUP_APPROVED_RUN_KEY_PATH: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run";
const RUN_VALUE_NAME: &str = "WinDynamicIsland";

struct RegKeyGuard(HKEY);

impl Drop for RegKeyGuard {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = RegCloseKey(self.0);
            }
        }
    }
}

pub fn config_file_path() -> Option<PathBuf> {
    if cfg!(test) {
        return Some(
            std::env::temp_dir()
                .join("wdi_test_profile")
                .join("config.ini"),
        );
    }
    let base = std::env::var_os("LOCALAPPDATA")?;
    if base.is_empty() {
        return None;
    }
    Some(PathBuf::from(base).join("WinDynamicIsland").join("config.ini"))
}

pub fn load_config() -> AppConfig {
    let mut cfg = match config_file_path().and_then(|p| fs::read_to_string(p).ok()) {
        Some(content) => parse_config(&content),
        None => AppConfig::default(),
    };
    if !cfg!(test) {
        sync_autostart_path_if_needed();
    }
    cfg.autostart = is_autostart_enabled();
    cfg
}

pub fn save_config(cfg: &AppConfig) {
    let Some(path) = config_file_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, serialize_config(cfg));
}

pub fn parse_config(text: &str) -> AppConfig {
    let mut cfg = AppConfig::default();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let key = k.trim();
        let val = v.trim();
        match key {
            "visible" => {
                if let Some(b) = parse_bool(val) {
                    cfg.visible = b;
                }
            }
            "notch_collapse_after_4s" => {
                if let Some(b) = parse_bool(val) {
                    cfg.notch_collapse_after_4s = b;
                }
            }
            "hud_volume_enabled" => {
                if let Some(b) = parse_bool(val) {
                    cfg.hud_volume_enabled = b;
                }
            }
            "hud_wheel_enabled" => {
                if let Some(b) = parse_bool(val) {
                    cfg.hud_wheel_enabled = b;
                }
            }
            "hud_power_enabled" => {
                if let Some(b) = parse_bool(val) {
                    cfg.hud_power_enabled = b;
                }
            }
            "hud_device_enabled" => {
                if let Some(b) = parse_bool(val) {
                    cfg.hud_device_enabled = b;
                }
            }
            "auto_hide_when_idle" => {
                if let Some(b) = parse_bool(val) {
                    cfg.auto_hide_when_idle = b;
                }
            }
            "auto_hide_fullscreen" => {
                if let Some(b) = parse_bool(val) {
                    cfg.auto_hide_fullscreen = b;
                }
            }
            "visualizer_enabled" => {
                if let Some(b) = parse_bool(val) {
                    cfg.visualizer_enabled = b;
                }
            }
            "anti_monitor_enabled" => {
                if let Some(b) = parse_bool(val) {
                    cfg.anti_monitor_enabled = b;
                }
            }
            "monitor_index" => {
                if let Ok(idx) = val.parse::<usize>() {
                    cfg.monitor_index = idx;
                }
            }
            _ => {}
        }
    }
    cfg
}

pub fn serialize_config(cfg: &AppConfig) -> String {
    format!(
        "visible={}\nnotch_collapse_after_4s={}\nhud_volume_enabled={}\nhud_wheel_enabled={}\nhud_power_enabled={}\nhud_device_enabled={}\nauto_hide_when_idle={}\nauto_hide_fullscreen={}\nvisualizer_enabled={}\nanti_monitor_enabled={}\nmonitor_index={}\n",
        cfg.visible,
        cfg.notch_collapse_after_4s,
        cfg.hud_volume_enabled,
        cfg.hud_wheel_enabled,
        cfg.hud_power_enabled,
        cfg.hud_device_enabled,
        cfg.auto_hide_when_idle,
        cfg.auto_hide_fullscreen,
        cfg.visualizer_enabled,
        cfg.anti_monitor_enabled,
        cfg.monitor_index,
    )
}

fn parse_bool(raw: &str) -> Option<bool> {
    match raw.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

pub fn extract_autostart_exe_path(command: &str) -> Option<PathBuf> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix('"') {
        let end = rest.find('"')?;
        let inner = rest[..end].trim();
        if inner.is_empty() {
            return None;
        }
        return Some(PathBuf::from(inner));
    }
    let lower = trimmed.to_ascii_lowercase();
    if let Some(idx) = lower.find(".exe") {
        let end = idx + 4;
        return Some(PathBuf::from(trimmed[..end].trim()));
    }
    trimmed.split_whitespace().next().map(PathBuf::from)
}

pub fn is_startup_approved_bytes_enabled(bytes: &[u8]) -> bool {
    match bytes.first() {
        Some(&first) => (first & 1) == 0,
        None => true,
    }
}

fn query_reg_sz_value(subkey_path: &str, value_name: &str) -> Option<String> {
    let subkey = to_wide_null(subkey_path);
    let val_w = to_wide_null(value_name);
    let mut hkey = HKEY::default();
    let open_err = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            None,
            KEY_QUERY_VALUE,
            &mut hkey,
        )
    };
    if open_err.is_err() {
        return None;
    }
    let _guard = RegKeyGuard(hkey);

    let mut reg_type = REG_VALUE_TYPE::default();
    let mut byte_len = 0u32;
    let probe = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(val_w.as_ptr()),
            None,
            Some(&mut reg_type),
            None,
            Some(&mut byte_len),
        )
    };
    if probe.is_err() || !(2..=8192).contains(&byte_len) {
        return None;
    }
    let mut buf = vec![0u8; byte_len as usize];
    let read = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(val_w.as_ptr()),
            None,
            Some(&mut reg_type),
            Some(buf.as_mut_ptr()),
            Some(&mut byte_len),
        )
    };
    if read.is_err() || reg_type != REG_SZ {
        return None;
    }
    let u16_count = (byte_len as usize) / 2;
    let mut units = Vec::with_capacity(u16_count);
    for &[lo, hi] in buf[..u16_count * 2].as_chunks::<2>().0 {
        let u = u16::from_le_bytes([lo, hi]);
        if u == 0 {
            break;
        }
        units.push(u);
    }
    Some(String::from_utf16_lossy(&units))
}

fn is_startup_approved_enabled_in_registry() -> bool {
    let subkey = to_wide_null(STARTUP_APPROVED_RUN_KEY_PATH);
    let val_w = to_wide_null(RUN_VALUE_NAME);
    let mut hkey = HKEY::default();
    let open_err = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            None,
            KEY_QUERY_VALUE,
            &mut hkey,
        )
    };
    if open_err.is_err() {
        return true;
    }
    let _guard = RegKeyGuard(hkey);

    let mut reg_type = REG_VALUE_TYPE::default();
    let mut byte_len = 0u32;
    let probe = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(val_w.as_ptr()),
            None,
            Some(&mut reg_type),
            None,
            Some(&mut byte_len),
        )
    };
    if probe.is_err() || byte_len == 0 || byte_len > 256 {
        return true;
    }
    let mut buf = vec![0u8; byte_len as usize];
    let read = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(val_w.as_ptr()),
            None,
            Some(&mut reg_type),
            Some(buf.as_mut_ptr()),
            Some(&mut byte_len),
        )
    };
    if read.is_err() || reg_type != REG_BINARY {
        return true;
    }
    is_startup_approved_bytes_enabled(&buf[..byte_len as usize])
}

pub fn sync_autostart_path_if_needed() {
    let Some(cmd) = query_reg_sz_value(RUN_KEY_PATH, RUN_VALUE_NAME) else {
        return;
    };
    if !is_startup_approved_enabled_in_registry() {
        return;
    }
    let Some(registered_exe) = extract_autostart_exe_path(&cmd) else {
        return;
    };
    let Ok(current_exe) = std::env::current_exe() else {
        return;
    };
    if !current_exe.is_file() {
        return;
    }
    let current_lower = current_exe.to_string_lossy().to_ascii_lowercase();
    if current_lower.contains("\\target\\debug\\") {
        return;
    }
    let is_broken = !registered_exe.is_file();
    let is_installed_copy = current_lower.contains("\\programs\\windynamicisland\\");
    if (is_broken || is_installed_copy) && registered_exe != current_exe {
        let _ = set_autostart_for_exe(&current_exe, true);
    }
}

pub fn is_autostart_enabled() -> bool {
    let Some(cmd) = query_reg_sz_value(RUN_KEY_PATH, RUN_VALUE_NAME) else {
        return false;
    };
    let Some(exe_path) = extract_autostart_exe_path(&cmd) else {
        return false;
    };
    exe_path.is_file() && is_startup_approved_enabled_in_registry()
}

pub fn set_autostart_enabled(enabled: bool) -> bool {
    if enabled {
        let Ok(exe) = std::env::current_exe() else {
            return false;
        };
        set_autostart_for_exe(&exe, true)
    } else {
        set_autostart_for_exe(Path::new(""), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_roundtrip_and_invalid_input_resilience() {
        let cfg = AppConfig {
            visible: false,
            notch_collapse_after_4s: false,
            hud_volume_enabled: false,
            hud_wheel_enabled: false,
            hud_power_enabled: false,
            hud_device_enabled: false,
            auto_hide_when_idle: true,
            auto_hide_fullscreen: false,
            visualizer_enabled: false,
            anti_monitor_enabled: false,
            autostart: true,
            monitor_index: 2,
        };
        let serialized = serialize_config(&cfg);
        let parsed = parse_config(&serialized);
        assert!(!parsed.visible);
        assert!(!parsed.notch_collapse_after_4s);
        assert!(!parsed.hud_volume_enabled);
        assert!(!parsed.hud_wheel_enabled);
        assert!(!parsed.hud_power_enabled);
        assert!(!parsed.hud_device_enabled);
        assert!(parsed.auto_hide_when_idle);
        assert!(!parsed.auto_hide_fullscreen);
        assert!(!parsed.visualizer_enabled);
        assert!(!parsed.anti_monitor_enabled);
        assert_eq!(parsed.monitor_index, 2);

        let corrupted = "# comment\nbad_line\nmonitor_index=abc\nvisible=not_a_bool\nauto_hide_when_idle=1\n";
        let fallback = parse_config(corrupted);
        assert!(fallback.visible);
        assert!(fallback.notch_collapse_after_4s);
        assert!(fallback.hud_volume_enabled);
        assert!(fallback.hud_wheel_enabled);
        assert!(fallback.hud_power_enabled);
        assert!(fallback.auto_hide_when_idle);
        assert!(fallback.auto_hide_fullscreen);
        assert_eq!(fallback.monitor_index, 0);
    }

    #[test]
    fn autostart_command_quotes_spaces_safely() {
        let path = std::path::Path::new(r"C:\Program Files\Win Dynamic Island\app.exe");
        assert_eq!(
            format_autostart_command(path),
            r#""C:\Program Files\Win Dynamic Island\app.exe""#
        );
    }

    #[test]
    fn extract_autostart_exe_path_handles_quoted_and_unquoted_commands() {
        assert_eq!(
            extract_autostart_exe_path(
                r#""C:\Users\ASUS\Desktop\New folder\win-dynamic-island.exe" --flag"#
            ),
            Some(PathBuf::from(
                r"C:\Users\ASUS\Desktop\New folder\win-dynamic-island.exe"
            ))
        );
        assert_eq!(
            extract_autostart_exe_path(r"C:\Apps\win-dynamic-island.exe /startup"),
            Some(PathBuf::from(r"C:\Apps\win-dynamic-island.exe"))
        );
        assert_eq!(extract_autostart_exe_path("   "), None);
    }

    #[test]
    fn startup_approved_bytes_detects_enabled_and_disabled_states() {
        assert!(is_startup_approved_bytes_enabled(&[2, 0, 0, 0, 0, 0, 0, 0]));
        assert!(is_startup_approved_bytes_enabled(&[6, 0, 0, 0]));
        assert!(!is_startup_approved_bytes_enabled(&[3, 0, 0, 0, 0, 0, 0, 0]));
        assert!(!is_startup_approved_bytes_enabled(&[7, 0, 0, 0]));
    }
}
