use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::core::GUID;

/// Mã GUID chuẩn của `GUID_ACDC_POWER_SOURCE` dùng cho `RegisterPowerSettingNotification`.
pub const GUID_ACDC_POWER_SOURCE: GUID = GUID::from_u128(0x5d3e9a59_e9d5_4b00_a6bd_ff34ff516548);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerSnapshot {
    pub has_battery: bool,
    pub on_ac: bool,
    pub percent: Option<u8>,
}

impl Default for PowerSnapshot {
    fn default() -> Self {
        Self {
            has_battery: false,
            on_ac: true,
            percent: None,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PowerStateTracker {
    last: Option<PowerSnapshot>,
    warned_low_15: bool,
    warned_low_7: bool,
}

impl PowerStateTracker {
    /// Kích hoạt tức thời theo thông báo AC/DC từ `WM_POWERBROADCAST` không cần chờ ACPI EC.
    pub fn evaluate_ac_hint(&mut self, on_ac: bool) -> Option<(PowerSnapshot, bool)> {
        let mut snap = self.last?;
        if !snap.has_battery || snap.on_ac == on_ac {
            return None;
        }
        snap.on_ac = on_ac;
        if on_ac {
            self.warned_low_15 = false;
            self.warned_low_7 = false;
        }
        self.last = Some(snap);
        Some((snap, false))
    }

    pub fn last_snapshot(&self) -> Option<PowerSnapshot> {
        self.last
    }

    /// Trả về `Some((snapshot, is_low_battery_alert))` chỉ khi đổi trạng thái sạc hoặc tụt xuống mốc pin yếu.
    pub fn evaluate_update(&mut self, next: PowerSnapshot) -> Option<(PowerSnapshot, bool)> {
        let Some(prev) = self.last else {
            self.last = Some(next);
            if next.has_battery
                && !next.on_ac
                && let Some(pct) = next.percent
            {
                if pct <= 15 {
                    self.warned_low_15 = true;
                }
                if pct <= 7 {
                    self.warned_low_7 = true;
                }
            }
            return None;
        };

        self.last = Some(next);
        if !next.has_battery {
            self.warned_low_15 = false;
            self.warned_low_7 = false;
            return None;
        }

        if prev.on_ac != next.on_ac {
            if next.on_ac {
                self.warned_low_15 = false;
                self.warned_low_7 = false;
            }
            return Some((next, false));
        }

        if !next.on_ac
            && let Some(pct) = next.percent
        {
            if pct <= 7 && !self.warned_low_7 {
                self.warned_low_15 = true;
                self.warned_low_7 = true;
                return Some((next, true));
            }
            if pct <= 15 && !self.warned_low_15 {
                self.warned_low_15 = true;
                return Some((next, true));
            }
            if pct > 20 {
                self.warned_low_15 = false;
                self.warned_low_7 = false;
            }
        }

        None
    }
}

pub fn read_system_power_status() -> PowerSnapshot {
    let mut status = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut status) }.is_err() {
        return PowerSnapshot::default();
    }
    let has_battery = (status.BatteryFlag & 128) == 0 && status.BatteryFlag != 255;
    let on_ac = if status.ACLineStatus == 255 {
        true
    } else {
        status.ACLineStatus == 1
    };
    let percent = if status.BatteryLifePercent <= 100 {
        Some(status.BatteryLifePercent)
    } else {
        None
    };
    PowerSnapshot {
        has_battery,
        on_ac,
        percent,
    }
}
