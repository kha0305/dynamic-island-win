use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use windows::Win32::Foundation::HWND;
use windows::Win32::Media::Audio::Endpoints::{
    IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl,
};
use windows::Win32::Media::Audio::{
    AUDIO_VOLUME_NOTIFICATION_DATA, DEVICE_STATE, EDataFlow, ERole, IMMDevice,
    IMMDeviceEnumerator, IMMNotificationClient, IMMNotificationClient_Impl, MMDeviceEnumerator,
    eMultimedia, eRender,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::{BOOL, GUID, PCWSTR, implement};

mod policy_config;
mod power;
#[cfg(test)]
mod tests;

use policy_config::{
    enumerate_output_devices, policy_com, read_device_friendly_name, read_device_id,
};
pub use power::{GUID_ACDC_POWER_SOURCE, PowerSnapshot, PowerStateTracker, read_system_power_status};

pub const WM_APP_HUD_EVENT: u32 = WM_APP + 7;
pub const HUD_EXPIRE_DELAY_MS: u32 = 2_000;
pub const WHEEL_STEP_DELTA: i32 = 40;
pub const WHEEL_VOLUME_STEP: f32 = 0.01;

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
