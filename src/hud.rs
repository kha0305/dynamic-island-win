use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::HWND;
use windows::Win32::Media::Audio::Endpoints::{
    IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl,
};
use windows::Win32::Media::Audio::{
    AUDIO_VOLUME_NOTIFICATION_DATA, DEVICE_STATE, DEVICE_STATE_ACTIVE, EDataFlow, ERole,
    IMMDevice, IMMDeviceEnumerator, IMMNotificationClient, IMMNotificationClient_Impl,
    MMDeviceEnumerator, eMultimedia, eRender,
};
use windows::Win32::System::Com::StructuredStorage::{
    PROPVARIANT, PropVariantClear, PropVariantToStringAlloc,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    CoUninitialize, STGM_READ,
};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::{BOOL, GUID, PCWSTR, PWSTR, implement};

pub const WM_APP_HUD_EVENT: u32 = WM_APP + 7;
pub const HUD_EXPIRE_DELAY_MS: u32 = 2_000;
pub const WHEEL_STEP_DELTA: i32 = 40;
pub const WHEEL_VOLUME_STEP: f32 = 0.01;

/// Mã GUID chuẩn của `GUID_ACDC_POWER_SOURCE` dùng cho `RegisterPowerSettingNotification`.
pub const GUID_ACDC_POWER_SOURCE: GUID = GUID::from_u128(0x5d3e9a59_e9d5_4b00_a6bd_ff34ff516548);
/// Ngữ cảnh riêng giúp phân biệt thao tác từ chính app để không tự dội lặp HUD.
const APP_VOLUME_EVENT_CONTEXT: GUID = GUID::from_u128(0x9a4b82e1_7f3c_4c68_b29d_317c8d6a5013);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioOutputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumeSnapshot {
    pub level: f32,
    pub muted: bool,
    pub available: bool,
}

impl Default for VolumeSnapshot {
    fn default() -> Self {
        Self {
            level: 0.5,
            muted: false,
            available: false,
        }
    }
}

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

#[derive(Debug, Clone, PartialEq)]
pub enum HudWorkerEvent {
    VolumeSync {
        level: f32,
        muted: bool,
        device_name: Option<String>,
        switched_device: bool,
        show_hud: bool,
    },
    DevicesChanged {
        devices: Vec<AudioOutputDevice>,
    },
    PowerSync {
        snap: PowerSnapshot,
        initial: bool,
    },
    AudioUnavailable,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AudioCommand {
    SetVolume(f32),
    StepVolume(i32),
    ToggleMute,
    SetDefaultDevice(String),
    RefreshPower,
    Shutdown,
}

/// Tích lũy `WHEEL_DELTA` để hỗ trợ cả chuột nấc 120 và Precision Touchpad mịn (<120).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WheelVolumeAccumulator {
    accum: i32,
}

impl WheelVolumeAccumulator {
    pub fn reset(&mut self) {
        self.accum = 0;
    }

    pub fn push_delta(&mut self, delta: i16) -> i32 {
        let d = delta as i32;
        if d == 0 {
            return 0;
        }
        // Không xóa tích lũy khi touchpad rung tay nhẹ (<8); chỉ xóa phần dư khi đảo chiều rõ ràng.
        if (self.accum > 0 && d <= -8) || (self.accum < 0 && d >= 8) {
            self.accum = 0;
        }
        self.accum = self.accum.saturating_add(d);
        let steps = self.accum / WHEEL_STEP_DELTA;
        self.accum %= WHEEL_STEP_DELTA;
        steps
    }
}

pub fn clamp_volume_ratio(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

pub fn apply_volume_step(current: f32, steps: i32) -> f32 {
    let base = clamp_volume_ratio(current);
    let delta = steps.clamp(-50, 50) as f32 * WHEEL_VOLUME_STEP;
    clamp_volume_ratio(base + delta)
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
            if next.has_battery && !next.on_ac
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

enum AudioThreadMsg {
    Command(AudioCommand),
    ReattachDefaultDevice,
    RefreshDeviceList,
    EndpointNotify {
        level: f32,
        muted: bool,
        is_self_event: bool,
    },
}

struct CoTaskMemPwstr(PWSTR);

impl CoTaskMemPwstr {
    fn to_non_empty_string(&self) -> Option<String> {
        if self.0.is_null() {
            return None;
        }
        unsafe { self.0.to_string().ok() }
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }
}

impl Drop for CoTaskMemPwstr {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CoTaskMemFree(Some(self.0.0.cast())) };
        }
    }
}

struct PropVarGuard(PROPVARIANT);

impl Drop for PropVarGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = PropVariantClear(&mut self.0);
        }
    }
}

#[allow(dead_code, non_snake_case, non_camel_case_types)]
mod policy_com {
    use core::ffi::c_void;
    use windows::Win32::Media::Audio::{ERole, eCommunications, eConsole, eMultimedia};
    use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
    use windows::core::{GUID, HRESULT, HSTRING, IUnknown, IUnknown_Vtbl, PCWSTR, interface};

    /// CLSID `CPolicyConfigClient` của Windows Core Audio để đổi thiết bị phát mặc định.
    const CLSID_CPOLICY_CONFIG_CLIENT: GUID =
        GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);

    #[interface("f8679f50-850a-41cf-9c72-430f290290c8")]
    pub(super) unsafe trait IPolicyConfig: IUnknown {
        fn GetMixFormat(&self, psz_device_name: PCWSTR, pp_format: *mut *mut c_void) -> HRESULT;
        fn GetDeviceFormat(
            &self,
            psz_device_name: PCWSTR,
            b_default: i32,
            pp_format: *mut *mut c_void,
        ) -> HRESULT;
        fn ResetDeviceFormat(&self, psz_device_name: PCWSTR) -> HRESULT;
        fn SetDeviceFormat(
            &self,
            psz_device_name: PCWSTR,
            p_endpoint_format: *mut c_void,
            p_mix_format: *mut c_void,
        ) -> HRESULT;
        fn GetProcessingPeriod(
            &self,
            psz_device_name: PCWSTR,
            b_default: i32,
            pmft_default: *mut i64,
            pmft_min: *mut i64,
        ) -> HRESULT;
        fn SetProcessingPeriod(&self, psz_device_name: PCWSTR, pmft_period: *mut i64) -> HRESULT;
        fn GetShareMode(&self, psz_device_name: PCWSTR, p_mode: *mut c_void) -> HRESULT;
        fn SetShareMode(&self, psz_device_name: PCWSTR, p_mode: *mut c_void) -> HRESULT;
        fn GetPropertyValue(
            &self,
            psz_device_name: PCWSTR,
            key: *const c_void,
            pv: *mut c_void,
        ) -> HRESULT;
        fn SetPropertyValue(
            &self,
            psz_device_name: PCWSTR,
            key: *const c_void,
            pv: *const c_void,
        ) -> HRESULT;
        fn SetDefaultEndpoint(&self, psz_device_name: PCWSTR, role: ERole) -> HRESULT;
        fn SetEndpointVisibility(&self, psz_device_name: PCWSTR, b_visible: i32) -> HRESULT;
    }

    pub(super) fn create_policy_config() -> windows::core::Result<IPolicyConfig> {
        unsafe { CoCreateInstance(&CLSID_CPOLICY_CONFIG_CLIENT, None, CLSCTX_ALL) }
    }

    pub(super) fn set_default_audio_device(device_id: &str) -> windows::core::Result<()> {
        let policy = create_policy_config()?;
        let wide_id = HSTRING::from(device_id);
        for role in [eConsole, eMultimedia, eCommunications] {
            unsafe { policy.SetDefaultEndpoint(PCWSTR(wide_id.as_ptr()), role) }.ok()?;
        }
        Ok(())
    }
}

fn read_device_id(device: &IMMDevice) -> Option<String> {
    let raw = unsafe { device.GetId() }.ok()?;
    CoTaskMemPwstr(raw).to_non_empty_string()
}

fn read_device_friendly_name(device: &IMMDevice) -> String {
    if let Ok(store) = unsafe { device.OpenPropertyStore(STGM_READ) }
        && let Ok(pv) = unsafe { store.GetValue(&PKEY_Device_FriendlyName) }
    {
        let guard = PropVarGuard(pv);
        if let Ok(pwstr) = unsafe { PropVariantToStringAlloc(&guard.0) }
            && let Some(name) = CoTaskMemPwstr(pwstr).to_non_empty_string()
        {
            return name;
        }
    }
    String::from("Thiết bị âm thanh")
}

pub fn sort_audio_devices(devices: &mut [AudioOutputDevice]) {
    devices.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
}

fn enumerate_output_devices(enumerator: &IMMDeviceEnumerator) -> (Vec<AudioOutputDevice>, Option<String>) {
    let default_id = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) }
        .ok()
        .and_then(|dev| read_device_id(&dev));

    let Ok(collection) = (unsafe { enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) })
    else {
        return (Vec::new(), None);
    };
    let count = unsafe { collection.GetCount() }.unwrap_or(0);
    let mut devices = Vec::with_capacity(count as usize);
    let mut default_name = None;

    for idx in 0..count {
        let Ok(dev) = (unsafe { collection.Item(idx) }) else {
            continue;
        };
        let Some(id) = read_device_id(&dev) else {
            continue;
        };
        let name = read_device_friendly_name(&dev);
        let is_default = default_id.as_deref() == Some(id.as_str());
        if is_default {
            default_name = Some(name.clone());
        }
        devices.push(AudioOutputDevice {
            id,
            name,
            is_default,
        });
    }
    sort_audio_devices(&mut devices);
    (devices, default_name)
}

#[implement(IAudioEndpointVolumeCallback)]
struct VolumeChangeCallback {
    tx: Sender<AudioThreadMsg>,
}

impl IAudioEndpointVolumeCallback_Impl for VolumeChangeCallback_Impl {
    fn OnNotify(&self, pnotify: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
        if pnotify.is_null() {
            return Ok(());
        }
        let data = unsafe { &*pnotify };
        let level = clamp_volume_ratio(data.fMasterVolume);
        let muted = data.bMuted.as_bool();
        let is_self_event = data.guidEventContext == APP_VOLUME_EVENT_CONTEXT;
        let _ = self.tx.send(AudioThreadMsg::EndpointNotify {
            level,
            muted,
            is_self_event,
        });
        Ok(())
    }
}

#[implement(IMMNotificationClient)]
struct DeviceNotifyCallback {
    tx: Sender<AudioThreadMsg>,
}

impl IMMNotificationClient_Impl for DeviceNotifyCallback_Impl {
    fn OnDeviceStateChanged(
        &self,
        _pwstrdeviceid: &PCWSTR,
        _dwnewstate: DEVICE_STATE,
    ) -> windows::core::Result<()> {
        let _ = self.tx.send(AudioThreadMsg::RefreshDeviceList);
        Ok(())
    }

    fn OnDeviceAdded(&self, _pwstrdeviceid: &PCWSTR) -> windows::core::Result<()> {
        let _ = self.tx.send(AudioThreadMsg::RefreshDeviceList);
        Ok(())
    }

    fn OnDeviceRemoved(&self, _pwstrdeviceid: &PCWSTR) -> windows::core::Result<()> {
        let _ = self.tx.send(AudioThreadMsg::RefreshDeviceList);
        Ok(())
    }

    fn OnDefaultDeviceChanged(
        &self,
        flow: EDataFlow,
        role: ERole,
        _pwstrdefaultdeviceid: &PCWSTR,
    ) -> windows::core::Result<()> {
        if flow == eRender && role == eMultimedia {
            let _ = self.tx.send(AudioThreadMsg::ReattachDefaultDevice);
        }
        Ok(())
    }

    fn OnPropertyValueChanged(
        &self,
        _pwstrdeviceid: &PCWSTR,
        _key: &windows::Win32::Foundation::PROPERTYKEY,
    ) -> windows::core::Result<()> {
        let _ = self.tx.send(AudioThreadMsg::RefreshDeviceList);
        Ok(())
    }
}

struct AttachedEndpoint {
    _device: IMMDevice,
    id: Option<String>,
    name: String,
    volume: IAudioEndpointVolume,
    callback: IAudioEndpointVolumeCallback,
}

impl Drop for AttachedEndpoint {
    fn drop(&mut self) {
        unsafe {
            let _ = self.volume.UnregisterControlChangeNotify(&self.callback);
        }
    }
}

pub struct AudioMonitor {
    msg_tx: Sender<AudioThreadMsg>,
    event_rx: Receiver<HudWorkerEvent>,
    join: Option<JoinHandle<()>>,
}

impl AudioMonitor {
    pub fn start(hwnd: HWND) -> Self {
        let hwnd_raw = hwnd.0 as usize;
        let (msg_tx, msg_rx) = mpsc::channel::<AudioThreadMsg>();
        let (event_tx, event_rx) = mpsc::channel::<HudWorkerEvent>();
        let worker_tx = msg_tx.clone();

        let join = thread::spawn(move || {
            run_audio_worker(hwnd_raw, worker_tx, msg_rx, event_tx);
        });

        Self {
            msg_tx,
            event_rx,
            join: Some(join),
        }
    }

    pub fn try_recv(&self) -> Option<HudWorkerEvent> {
        self.event_rx.try_recv().ok()
    }

    pub fn send_command(&self, cmd: AudioCommand) {
        let _ = self.msg_tx.send(AudioThreadMsg::Command(cmd));
    }
}

impl Drop for AudioMonitor {
    fn drop(&mut self) {
        let _ = self.msg_tx.send(AudioThreadMsg::Command(AudioCommand::Shutdown));
        if let Some(handle) = self.join.take() {
            let _ = handle.join();
        }
    }
}

struct ComMtaGuard(bool);

impl Drop for ComMtaGuard {
    fn drop(&mut self) {
        if self.0 {
            unsafe {
                CoUninitialize();
            }
        }
    }
}

struct DeviceClientGuard {
    enumerator: IMMDeviceEnumerator,
    client: IMMNotificationClient,
}

impl Drop for DeviceClientGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = self
                .enumerator
                .UnregisterEndpointNotificationCallback(&self.client);
        }
    }
}

fn post_hud_wake(hwnd_raw: usize) {
    let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
    unsafe {
        let _ = PostMessageW(
            Some(hwnd),
            WM_APP_HUD_EVENT,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(0),
        );
    }
}

fn attach_default_endpoint(
    enumerator: &IMMDeviceEnumerator,
    tx: &Sender<AudioThreadMsg>,
) -> Option<(AttachedEndpoint, f32, bool)> {
    let device = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) }.ok()?;
    let id = read_device_id(&device);
    let name = read_device_friendly_name(&device);
    let volume: IAudioEndpointVolume = unsafe { device.Activate(CLSCTX_ALL, None) }.ok()?;
    let callback: IAudioEndpointVolumeCallback = VolumeChangeCallback { tx: tx.clone() }.into();
    unsafe { volume.RegisterControlChangeNotify(&callback) }.ok()?;

    let level = unsafe { volume.GetMasterVolumeLevelScalar() }
        .map(clamp_volume_ratio)
        .unwrap_or(0.5);
    let muted = unsafe { volume.GetMute() }.map(BOOL::as_bool).unwrap_or(false);

    Some((
        AttachedEndpoint {
            _device: device,
            id,
            name,
            volume,
            callback,
        },
        level,
        muted,
    ))
}

fn run_audio_worker(
    hwnd_raw: usize,
    worker_tx: Sender<AudioThreadMsg>,
    msg_rx: Receiver<AudioThreadMsg>,
    event_tx: Sender<HudWorkerEvent>,
) {
    let com = ComMtaGuard(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok());
    let _ = event_tx.send(HudWorkerEvent::PowerSync {
        snap: read_system_power_status(),
        initial: true,
    });
    post_hud_wake(hwnd_raw);
    let Ok(enumerator) = (unsafe { CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) }) else {
        let _ = event_tx.send(HudWorkerEvent::AudioUnavailable);
        post_hud_wake(hwnd_raw);
        for msg in msg_rx.iter() {
            match msg {
                AudioThreadMsg::Command(AudioCommand::Shutdown) => break,
                AudioThreadMsg::Command(AudioCommand::RefreshPower) => {
                    let _ = event_tx.send(HudWorkerEvent::PowerSync {
                        snap: read_system_power_status(),
                        initial: false,
                    });
                    post_hud_wake(hwnd_raw);
                }
                _ => {}
            }
        }
        drop(com);
        return;
    };

    let dev_client: IMMNotificationClient = DeviceNotifyCallback {
        tx: worker_tx.clone(),
    }
    .into();
    let _dev_guard = unsafe {
        enumerator
            .RegisterEndpointNotificationCallback(&dev_client)
            .ok()
    }
    .map(|_| DeviceClientGuard {
        enumerator: enumerator.clone(),
        client: dev_client,
    });

    let (initial_devices, _default_name) = enumerate_output_devices(&enumerator);
    let _ = event_tx.send(HudWorkerEvent::DevicesChanged {
        devices: initial_devices,
    });

    let (mut attached, mut current_level, mut current_muted) =
        match attach_default_endpoint(&enumerator, &worker_tx) {
            Some((ep, lvl, m)) => {
                let _ = event_tx.send(HudWorkerEvent::VolumeSync {
                    level: lvl,
                    muted: m,
                    device_name: Some(ep.name.clone()),
                    switched_device: false,
                    show_hud: false,
                });
                post_hud_wake(hwnd_raw);
                (Some(ep), lvl, m)
            }
            None => {
                let _ = event_tx.send(HudWorkerEvent::AudioUnavailable);
                post_hud_wake(hwnd_raw);
                (None, 0.5_f32, false)
            }
        };

    while let Ok(first) = msg_rx.recv() {
        let mut pending_shutdown = false;
        let mut pending_reattach = false;
        let mut pending_refresh_devices = false;
        let mut pending_switch_device: Option<String> = None;
        let mut pending_power_refresh = false;
        let mut toggle_mute_count = 0_u32;
        let mut pending_set_vol: Option<f32> = None;
        let mut pending_step_sum: i32 = 0;
        let mut latest_notify: Option<(f32, bool, bool)> = None;

        let mut enqueue = |m: AudioThreadMsg| -> bool {
            match m {
                AudioThreadMsg::Command(AudioCommand::Shutdown) => return true,
                AudioThreadMsg::Command(AudioCommand::ToggleMute) => toggle_mute_count += 1,
                AudioThreadMsg::Command(AudioCommand::RefreshPower) => pending_power_refresh = true,
                AudioThreadMsg::Command(AudioCommand::SetDefaultDevice(id)) => {
                    pending_switch_device = Some(id);
                }
                AudioThreadMsg::Command(AudioCommand::SetVolume(v)) => {
                    pending_step_sum = 0;
                    pending_set_vol = Some(clamp_volume_ratio(v));
                }
                AudioThreadMsg::Command(AudioCommand::StepVolume(s)) => {
                    if let Some(v) = pending_set_vol {
                        pending_set_vol = Some(apply_volume_step(v, s));
                    } else {
                        pending_step_sum = pending_step_sum.saturating_add(s);
                    }
                }
                AudioThreadMsg::ReattachDefaultDevice => {
                    pending_reattach = true;
                    pending_refresh_devices = true;
                }
                AudioThreadMsg::RefreshDeviceList => {
                    pending_refresh_devices = true;
                }
                AudioThreadMsg::EndpointNotify {
                    level,
                    muted,
                    is_self_event,
                } => {
                    latest_notify = Some((level, muted, is_self_event));
                }
            }
            false
        };

        if enqueue(first) {
            pending_shutdown = true;
        } else {
            while let Ok(extra) = msg_rx.try_recv() {
                if enqueue(extra) {
                    pending_shutdown = true;
                    break;
                }
            }
        }
        if pending_shutdown {
            break;
        }

        if pending_power_refresh {
            let _ = event_tx.send(HudWorkerEvent::PowerSync {
                snap: read_system_power_status(),
                initial: false,
            });
            post_hud_wake(hwnd_raw);
        }

        let mut switched_manually = false;
        if let Some(target_id) = pending_switch_device {
            let already_active = attached
                .as_ref()
                .and_then(|ep| ep.id.as_deref())
                == Some(target_id.as_str());
            if !already_active && policy_com::set_default_audio_device(&target_id).is_ok() {
                switched_manually = true;
                pending_reattach = true;
                pending_refresh_devices = true;
            }
        }

        if pending_reattach {
            let prev_id = attached.as_ref().and_then(|ep| ep.id.clone());
            drop(attached.take());
            if let Some((ep, lvl, m)) = attach_default_endpoint(&enumerator, &worker_tx) {
                let endpoint_changed = ep.id != prev_id;
                let device_name = ep.name.clone();
                attached = Some(ep);
                current_level = lvl;
                current_muted = m;
                let switched = switched_manually || endpoint_changed;
                let _ = event_tx.send(HudWorkerEvent::VolumeSync {
                    level: lvl,
                    muted: m,
                    device_name: Some(device_name),
                    switched_device: switched,
                    show_hud: switched,
                });
                pending_refresh_devices = true;
            } else {
                attached = None;
                let _ = event_tx.send(HudWorkerEvent::AudioUnavailable);
            }
            post_hud_wake(hwnd_raw);
        }

        if pending_refresh_devices {
            let (devices, _) = enumerate_output_devices(&enumerator);
            let _ = event_tx.send(HudWorkerEvent::DevicesChanged { devices });
            post_hud_wake(hwnd_raw);
        }

        if toggle_mute_count % 2 == 1
            && let Some(ep) = &attached
        {
            let next_mute = !current_muted;
            if unsafe { ep.volume.SetMute(next_mute, &APP_VOLUME_EVENT_CONTEXT) }.is_ok() {
                current_muted = next_mute;
            }
        }

        if let Some(target) = pending_set_vol {
            if let Some(ep) = &attached {
                if target > 0.0 && current_muted {
                    let _ = unsafe { ep.volume.SetMute(false, &APP_VOLUME_EVENT_CONTEXT) };
                    current_muted = false;
                }
                let _ = unsafe {
                    ep.volume
                        .SetMasterVolumeLevelScalar(target, &APP_VOLUME_EVENT_CONTEXT)
                };
                current_level = target;
            }
        } else if pending_step_sum != 0
            && let Some(ep) = &attached
        {
            let latest = unsafe { ep.volume.GetMasterVolumeLevelScalar() }
                .map(clamp_volume_ratio)
                .unwrap_or(current_level);
            let next = apply_volume_step(latest, pending_step_sum);
            if pending_step_sum > 0 && current_muted {
                let _ = unsafe { ep.volume.SetMute(false, &APP_VOLUME_EVENT_CONTEXT) };
                current_muted = false;
            }
            let _ = unsafe {
                ep.volume
                    .SetMasterVolumeLevelScalar(next, &APP_VOLUME_EVENT_CONTEXT)
            };
            current_level = next;
        }

        if let Some((level, muted, is_self_event)) = latest_notify
            && !is_self_event
        {
            let changed = (level - current_level).abs() >= 0.002 || muted != current_muted;
            current_level = level;
            current_muted = muted;
            if changed {
                let _ = event_tx.send(HudWorkerEvent::VolumeSync {
                    level,
                    muted,
                    device_name: attached.as_ref().map(|ep| ep.name.clone()),
                    switched_device: false,
                    show_hud: true,
                });
                post_hud_wake(hwnd_raw);
            }
        }
    }

    drop(attached);
    drop(_dev_guard);
    drop(com);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_accumulator_handles_detents_touchpad_and_reversals() {
        let mut acc = WheelVolumeAccumulator::default();
        assert_eq!(acc.push_delta(120), 3);
        assert_eq!(acc.push_delta(-240), -6);

        // Touchpad cuộn mịn: nhiều gói nhỏ cộng dồn đủ 40 sinh 1 bước, rung tay -2 không xóa tích lũy.
        assert_eq!(acc.push_delta(15), 0);
        assert_eq!(acc.push_delta(15), 0);
        assert_eq!(acc.push_delta(-2), 0);
        assert_eq!(acc.push_delta(15), 1);
        assert_eq!(acc.push_delta(20), 0);

        // Đảo chiều rõ ràng: phần dư cũ (+23) bị xóa sạch.
        assert_eq!(acc.push_delta(-30), 0);
        assert_eq!(acc.push_delta(-15), -1);

        acc.push_delta(30);
        acc.reset();
        assert_eq!(acc.push_delta(15), 0);
    }

    #[test]
    fn volume_step_clamps_bounds_and_handles_bursts() {
        assert!((apply_volume_step(0.50, 3) - 0.53).abs() < 1e-5);
        assert!((apply_volume_step(0.99, 5) - 1.0).abs() < 1e-5);
        assert!((apply_volume_step(0.01, -5) - 0.0).abs() < 1e-5);
        assert!((apply_volume_step(0.50, 999) - 1.0).abs() < 1e-5);
        assert_eq!(clamp_volume_ratio(f32::NAN), 0.0);
    }

    #[test]
    fn power_tracker_alerts_only_on_ac_toggle_or_low_thresholds() {
        let mut tracker = PowerStateTracker::default();
        // Lần đọc đầu tiên không pop HUD.
        assert!(
            tracker
                .evaluate_update(PowerSnapshot {
                    has_battery: true,
                    on_ac: false,
                    percent: Some(80),
                })
                .is_none()
        );
        // Tụt từng % bình thường không spam HUD.
        assert!(
            tracker
                .evaluate_update(PowerSnapshot {
                    has_battery: true,
                    on_ac: false,
                    percent: Some(79),
                })
                .is_none()
        );
        // Cắm sạc -> pop HUD sạc.
        assert_eq!(
            tracker.evaluate_update(PowerSnapshot {
                has_battery: true,
                on_ac: true,
                percent: Some(79),
            }),
            Some((
                PowerSnapshot {
                    has_battery: true,
                    on_ac: true,
                    percent: Some(79),
                },
                false
            ))
        );
        // Đang cắm sạc tăng % không spam.
        assert!(
            tracker
                .evaluate_update(PowerSnapshot {
                    has_battery: true,
                    on_ac: true,
                    percent: Some(85),
                })
                .is_none()
        );
        // Rút sạc -> pop HUD dùng pin.
        assert_eq!(
            tracker.evaluate_update(PowerSnapshot {
                has_battery: true,
                on_ac: false,
                percent: Some(25),
            }),
            Some((
                PowerSnapshot {
                    has_battery: true,
                    on_ac: false,
                    percent: Some(25),
                },
                false
            ))
        );
        // Tụt xuống <= 15% lần đầu -> cảnh báo pin yếu.
        assert_eq!(
            tracker.evaluate_update(PowerSnapshot {
                has_battery: true,
                on_ac: false,
                percent: Some(15),
            }),
            Some((
                PowerSnapshot {
                    has_battery: true,
                    on_ac: false,
                    percent: Some(15),
                },
                true
            ))
        );
        // Tụt tiếp 14% -> không nhắc lại cho tới ngưỡng <= 7%.
        assert!(
            tracker
                .evaluate_update(PowerSnapshot {
                    has_battery: true,
                    on_ac: false,
                    percent: Some(14),
                })
                .is_none()
        );
        assert_eq!(
            tracker.evaluate_update(PowerSnapshot {
                has_battery: true,
                on_ac: false,
                percent: Some(7),
            }),
            Some((
                PowerSnapshot {
                    has_battery: true,
                    on_ac: false,
                    percent: Some(7),
                },
                true
            ))
        );

        // AC hint tu PBT_POWERSETTINGCHANGE bat HUD tuc thi va khong bi spam trung khi RefreshPower ve cung on_ac.
        assert_eq!(
            tracker.evaluate_ac_hint(true),
            Some((
                PowerSnapshot {
                    has_battery: true,
                    on_ac: true,
                    percent: Some(7),
                },
                false
            ))
        );
        assert!(
            tracker
                .evaluate_update(PowerSnapshot {
                    has_battery: true,
                    on_ac: true,
                    percent: Some(8),
                })
                .is_none()
        );
    }

    #[test]
    fn audio_devices_sort_and_policy_config_com_init() {
        let mut items = vec![
            AudioOutputDevice {
                id: String::from("id-2"),
                name: String::from("Speakers (Realtek)"),
                is_default: false,
            },
            AudioOutputDevice {
                id: String::from("id-1"),
                name: String::from("headphones (USB)"),
                is_default: true,
            },
        ];
        sort_audio_devices(&mut items);
        assert_eq!(items[0].id, "id-1");
        assert_eq!(items[1].id, "id-2");

        let com = ComMtaGuard(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok());
        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.expect("enumerator");
        let (real_devices, _def) = enumerate_output_devices(&enumerator);
        println!("Detected {} active playback devices", real_devices.len());
        for d in &real_devices {
            assert!(!d.id.is_empty());
            assert!(!d.name.is_empty());
        }
        let policy = policy_com::create_policy_config();
        assert!(policy.is_ok(), "IPolicyConfig CoCreateInstance failed: {policy:?}");
        if let Some(def) = real_devices.iter().find(|d| d.is_default) {
            let res = policy_com::set_default_audio_device(&def.id);
            assert!(res.is_ok(), "IPolicyConfig::SetDefaultEndpoint failed on current default: {res:?}");
        }
        drop(policy);
        drop(enumerator);
        drop(com);
    }
}
