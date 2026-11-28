use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Devices::Properties::DEVPKEY_Device_EnumeratorName;
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::{
    DEVICE_STATE_ACTIVE, EndpointFormFactor, Headphones, Headset, IMMDevice, IMMDeviceEnumerator,
    PKEY_AudioEndpoint_FormFactor, eMultimedia, eRender,
};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Com::STGM_READ;
use windows::Win32::System::Com::StructuredStorage::{
    PROPVARIANT, PropVariantClear, PropVariantToStringAlloc, PropVariantToUInt32,
};
use windows::core::PWSTR;

use super::AudioOutputDevice;
use crate::state::AudioOutputKind;

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
pub(super) mod policy_com {
    use core::ffi::c_void;
    use windows::Win32::Media::Audio::{ERole, eCommunications, eConsole, eMultimedia};
    use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
    use windows::core::{GUID, HRESULT, HSTRING, IUnknown, IUnknown_Vtbl, PCWSTR, interface};

    /// CLSID `CPolicyConfigClient` của Windows Core Audio để đổi thiết bị phát mặc định.
    const CLSID_CPOLICY_CONFIG_CLIENT: GUID =
        GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);

    #[interface("f8679f50-850a-41cf-9c72-430f290290c8")]
    pub(in crate::hud) unsafe trait IPolicyConfig: IUnknown {
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

    pub(in crate::hud) fn create_policy_config() -> windows::core::Result<IPolicyConfig> {
        unsafe { CoCreateInstance(&CLSID_CPOLICY_CONFIG_CLIENT, None, CLSCTX_ALL) }
    }

    pub(in crate::hud) fn set_default_audio_device(device_id: &str) -> windows::core::Result<()> {
        let policy = create_policy_config()?;
        let wide_id = HSTRING::from(device_id);
        for role in [eConsole, eMultimedia, eCommunications] {
            unsafe { policy.SetDefaultEndpoint(PCWSTR(wide_id.as_ptr()), role) }.ok()?;
        }
        Ok(())
    }
}

pub(super) fn read_device_id(device: &IMMDevice) -> Option<String> {
    let raw = unsafe { device.GetId() }.ok()?;
    CoTaskMemPwstr(raw).to_non_empty_string()
}

pub(super) fn read_device_friendly_name(device: &IMMDevice) -> String {
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

pub fn classify_audio_output_kind(enumerator: &str, form_factor: Option<i32>, name: &str) -> AudioOutputKind {
    let enum_lower = enumerator.trim().to_ascii_lowercase();
    let name_lower = name.trim().to_ascii_lowercase();
    if enum_lower.starts_with("bth") || name_lower.contains("bluetooth") {
        return AudioOutputKind::Bluetooth;
    }
    if let Some(ff) = form_factor
        && (ff == Headphones.0 || ff == Headset.0)
    {
        return AudioOutputKind::Headphones;
    }
    if name_lower.contains("headphone")
        || name_lower.contains("headset")
        || name_lower.contains("earphone")
        || name_lower.contains("tai nghe")
    {
        return AudioOutputKind::Headphones;
    }
    AudioOutputKind::Speaker
}

pub(super) fn read_audio_output_kind(device: &IMMDevice, friendly_name: &str) -> AudioOutputKind {
    let Ok(store) = (unsafe { device.OpenPropertyStore(STGM_READ) }) else {
        return classify_audio_output_kind("", None, friendly_name);
    };
    let enum_key = PROPERTYKEY {
        fmtid: DEVPKEY_Device_EnumeratorName.fmtid,
        pid: DEVPKEY_Device_EnumeratorName.pid,
    };
    let enumerator = if let Ok(pv) = unsafe { store.GetValue(&enum_key) } {
        let guard = PropVarGuard(pv);
        unsafe { PropVariantToStringAlloc(&guard.0) }
            .ok()
            .and_then(|pw| CoTaskMemPwstr(pw).to_non_empty_string())
            .unwrap_or_default()
    } else {
        String::new()
    };
    let form_factor = if let Ok(pv) = unsafe { store.GetValue(&PKEY_AudioEndpoint_FormFactor) } {
        let guard = PropVarGuard(pv);
        unsafe { PropVariantToUInt32(&guard.0) }
            .ok()
            .map(|v| EndpointFormFactor(v as i32).0)
    } else {
        None
    };
    classify_audio_output_kind(&enumerator, form_factor, friendly_name)
}

pub fn sort_audio_devices(devices: &mut [AudioOutputDevice]) {
    devices.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
}

pub(super) fn enumerate_output_devices(
    enumerator: &IMMDeviceEnumerator,
) -> (Vec<AudioOutputDevice>, Option<String>, AudioOutputKind) {
    let default_id = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) }
        .ok()
        .and_then(|dev| read_device_id(&dev));

    let Ok(collection) = (unsafe { enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) })
    else {
        return (Vec::new(), None, AudioOutputKind::Speaker);
    };
    let count = unsafe { collection.GetCount() }.unwrap_or(0);
    let mut devices = Vec::with_capacity(count as usize);
    let mut default_name = None;
    let mut default_kind = AudioOutputKind::Speaker;

    for idx in 0..count {
        let Ok(dev) = (unsafe { collection.Item(idx) }) else {
            continue;
        };
        let Some(id) = read_device_id(&dev) else {
            continue;
        };
        let name = read_device_friendly_name(&dev);
        let kind = read_audio_output_kind(&dev, &name);
        let is_default = default_id.as_deref() == Some(id.as_str());
        if is_default {
            default_name = Some(name.clone());
            default_kind = kind;
        }
        devices.push(AudioOutputDevice {
            id,
            name,
            kind,
            is_default,
        });
    }
    sort_audio_devices(&mut devices);
    (devices, default_name, default_kind)
}
