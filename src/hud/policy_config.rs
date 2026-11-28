use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::{
    DEVICE_STATE_ACTIVE, IMMDevice, IMMDeviceEnumerator, eMultimedia, eRender,
};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Com::STGM_READ;
use windows::Win32::System::Com::StructuredStorage::{
    PROPVARIANT, PropVariantClear, PropVariantToStringAlloc,
};
use windows::core::PWSTR;

use super::AudioOutputDevice;

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
) -> (Vec<AudioOutputDevice>, Option<String>) {
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
