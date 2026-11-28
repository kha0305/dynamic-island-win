use std::fs;
use std::path::PathBuf;

use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
    REG_VALUE_TYPE, RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
    RegSetValueExW,
};
use windows::core::PCWSTR;

use crate::state::AppConfig;
use crate::tray::to_wide_null;

const RUN_KEY_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
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
        "visible={}\nnotch_collapse_after_4s={}\nhud_volume_enabled={}\nhud_wheel_enabled={}\nhud_power_enabled={}\nauto_hide_when_idle={}\nauto_hide_fullscreen={}\nvisualizer_enabled={}\nanti_monitor_enabled={}\nmonitor_index={}\n",
        cfg.visible,
        cfg.notch_collapse_after_4s,
        cfg.hud_volume_enabled,
        cfg.hud_wheel_enabled,
        cfg.hud_power_enabled,
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

pub fn format_autostart_command(exe_path: &std::path::Path) -> String {
    format!("\"{}\"", exe_path.display())
}

pub fn is_autostart_enabled() -> bool {
    let subkey = to_wide_null(RUN_KEY_PATH);
    let value_name = to_wide_null(RUN_VALUE_NAME);
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
        return false;
    }
    let _guard = RegKeyGuard(hkey);

    let mut reg_type = REG_VALUE_TYPE::default();
    let mut byte_len = 0u32;
    let query_err = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(value_name.as_ptr()),
            None,
            Some(&mut reg_type),
            None,
            Some(&mut byte_len),
        )
    };
    query_err.is_ok() && byte_len > 2
}

pub fn set_autostart_enabled(enabled: bool) -> bool {
    let subkey = to_wide_null(RUN_KEY_PATH);
    let value_name = to_wide_null(RUN_VALUE_NAME);

    if enabled {
        let Ok(exe) = std::env::current_exe() else {
            return false;
        };
        let cmd = format_autostart_command(&exe);
        let wide = to_wide_null(&cmd);
        let bytes: Vec<u8> = wide.iter().flat_map(|c| c.to_le_bytes()).collect();

        let mut hkey = HKEY::default();
        let create_err = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut hkey,
                None,
            )
        };
        if create_err.is_err() {
            return false;
        }
        let _guard = RegKeyGuard(hkey);

        let set_err = unsafe {
            RegSetValueExW(
                hkey,
                PCWSTR(value_name.as_ptr()),
                None,
                REG_SZ,
                Some(&bytes),
            )
        };
        set_err.is_ok()
    } else {
        let mut hkey = HKEY::default();
        let open_err = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                None,
                KEY_SET_VALUE,
                &mut hkey,
            )
        };
        if open_err.is_err() {
            return true;
        }
        let _guard = RegKeyGuard(hkey);
        let del_err = unsafe { RegDeleteValueW(hkey, PCWSTR(value_name.as_ptr())) };
        del_err.is_ok() || !is_autostart_enabled()
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
}
