use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::size_of;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_GETIDLIST_FILTER_PRESENT, CM_Get_DevNode_PropertyW, CM_Get_Device_ID_ListW,
    CM_Get_Device_ID_List_SizeW, CM_Get_Device_Interface_PropertyW, CM_LOCATE_DEVNODE_NORMAL,
    CM_LOCATE_DEVNODE_PHANTOM, CM_Locate_DevNodeW, CM_NOTIFY_ACTION,
    CM_NOTIFY_ACTION_DEVICEINSTANCEENUMERATED, CM_NOTIFY_ACTION_DEVICEINSTANCEREMOVED,
    CM_NOTIFY_ACTION_DEVICEINSTANCESTARTED, CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL,
    CM_NOTIFY_ACTION_DEVICEINTERFACEREMOVAL, CM_NOTIFY_EVENT_DATA, CM_NOTIFY_FILTER,
    CM_NOTIFY_FILTER_FLAG_ALL_DEVICE_INSTANCES, CM_NOTIFY_FILTER_FLAG_ALL_INTERFACE_CLASSES,
    CM_NOTIFY_FILTER_TYPE_DEVICEINSTANCE, CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE,
    CM_Register_Notification, CM_Unregister_Notification, CR_SUCCESS, HCMNOTIFICATION,
};
use windows::Win32::Devices::Properties::{
    DEVPROP_TYPE_BOOLEAN, DEVPROP_TYPE_BYTE, DEVPROP_TYPE_GUID, DEVPROP_TYPE_STRING,
    DEVPROP_TYPE_UINT32, DEVPROPTYPE, DEVPKEY_DeviceContainer_Category, DEVPKEY_Device_ContainerId,
    DEVPKEY_Device_DeviceDesc, DEVPKEY_Device_EnumeratorName, DEVPKEY_Device_FriendlyName,
    DEVPKEY_Device_InstanceId,
};
use windows::Win32::Foundation::DEVPROPKEY;
use windows::core::{GUID, PCWSTR};

use crate::state::DeviceHudCategory;
use crate::tray::to_wide_null;

use super::AudioThreadMsg;

const DEVPKEY_DEVICE_IS_CONNECTED: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0x83da6326_97a6_4088_9453_a1923f573b29),
    pid: 15,
};

const DEVPKEY_DEVICE_BATTERY_LEVEL: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0x104ea319_6ee2_4701_bd47_8ddbf425bbe5),
    pid: 2,
};

const DEBOUNCE_WINDOW: Duration = Duration::from_millis(950);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceHudEvent {
    pub container_key: String,
    pub name: String,
    pub category: DeviceHudCategory,
    pub connected: bool,
    pub battery_percent: Option<u8>,
    pub is_battery_refresh: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CachedDeviceMeta {
    name: String,
    category: DeviceHudCategory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BluetoothSnapshot {
    container_key: String,
    name: String,
    connected: bool,
    battery_percent: Option<u8>,
}

pub struct DeviceChangeTracker {
    known_meta: HashMap<String, CachedDeviceMeta>,
    bluetooth_states: HashMap<String, BluetoothSnapshot>,
    recent_emits: HashMap<String, (bool, Instant)>,
    pending_battery_checks: Vec<(String, Instant)>,
}

impl DeviceChangeTracker {
    pub fn new_seeded() -> Self {
        let mut tracker = Self {
            known_meta: HashMap::new(),
            bluetooth_states: HashMap::new(),
            recent_emits: HashMap::new(),
            pending_battery_checks: Vec::new(),
        };
        tracker.seed_initial_devices();
        tracker
    }

    #[cfg(test)]
    pub(crate) fn new_empty() -> Self {
        Self {
            known_meta: HashMap::new(),
            bluetooth_states: HashMap::new(),
            recent_emits: HashMap::new(),
            pending_battery_checks: Vec::new(),
        }
    }

    pub fn next_battery_check_timeout(&self, now: Instant) -> Option<Duration> {
        self.pending_battery_checks
            .iter()
            .map(|(_, at)| at.saturating_duration_since(now))
            .min()
    }

    pub fn poll_pending_battery_checks(&mut self, now: Instant) -> Vec<DeviceHudEvent> {
        let mut due_keys = Vec::new();
        self.pending_battery_checks.retain(|(key, at)| {
            if *at <= now {
                due_keys.push(key.clone());
                false
            } else {
                true
            }
        });
        if due_keys.is_empty() {
            return Vec::new();
        }
        let snaps = enumerate_bluetooth_snapshots();
        let mut events = Vec::new();
        for key in due_keys {
            if let Some(snap) = snaps.iter().find(|s| s.container_key == key)
                && snap.connected
                && snap.battery_percent.is_some()
            {
                let prev_bat = self
                    .bluetooth_states
                    .get(&key)
                    .and_then(|s| s.battery_percent);
                if prev_bat != snap.battery_percent {
                    self.bluetooth_states.insert(key.clone(), snap.clone());
                    events.push(DeviceHudEvent {
                        container_key: snap.container_key.clone(),
                        name: snap.name.clone(),
                        category: DeviceHudCategory::Bluetooth,
                        connected: true,
                        battery_percent: snap.battery_percent,
                        is_battery_refresh: true,
                    });
                }
            }
        }
        events
    }

    fn seed_initial_devices(&mut self) {
        for snap in enumerate_bluetooth_snapshots() {
            self.known_meta.insert(
                snap.container_key.clone(),
                CachedDeviceMeta {
                    name: snap.name.clone(),
                    category: DeviceHudCategory::Bluetooth,
                },
            );
            self.bluetooth_states.insert(snap.container_key.clone(), snap);
        }
        for id in enumerate_present_device_ids() {
            if let Some((key, meta)) = inspect_instance_meta(&id, false) {
                self.upsert_better_meta(&key, meta);
            }
        }
    }

    fn upsert_better_meta(&mut self, key: &str, candidate: CachedDeviceMeta) {
        let replace = match self.known_meta.get(key) {
            None => true,
            Some(old) => name_quality_score(&candidate.name) >= name_quality_score(&old.name),
        };
        if replace {
            self.known_meta.insert(key.to_string(), candidate);
        }
    }

    pub fn handle_pnp_event(
        &mut self,
        instance_id: &str,
        arrival: bool,
        now: Instant,
    ) -> Vec<DeviceHudEvent> {
        let upper = instance_id.trim().to_ascii_uppercase();
        if upper.is_empty() || is_ignored_device_instance(&upper) {
            return Vec::new();
        }

        if is_bluetooth_instance(&upper) {
            return self.sync_bluetooth_changes(now);
        }

        if arrival {
            let Some((container_key, meta)) = inspect_instance_meta(instance_id, false) else {
                return Vec::new();
            };
            if meta.category == DeviceHudCategory::Bluetooth {
                return self.sync_bluetooth_changes(now);
            }
            self.upsert_better_meta(&container_key, meta);
            let Some(final_meta) = self.known_meta.get(&container_key).cloned() else {
                return Vec::new();
            };
            return self
                .emit_debounced(container_key, final_meta, true, None, now)
                .into_iter()
                .collect();
        }

        // Khi rút thiết bị: đọc phantom devnode để lấy lại ContainerId, nếu mất devnode thì dùng instance_id.
        let inspected = inspect_instance_meta(instance_id, true);
        if let Some((_, ref meta)) = inspected
            && meta.category == DeviceHudCategory::Bluetooth
        {
            return self.sync_bluetooth_changes(now);
        }
        let container_key = inspected
            .as_ref()
            .map(|(k, _)| k.clone())
            .unwrap_or_else(|| upper.clone());
        let cached_meta = self
            .known_meta
            .get(&container_key)
            .cloned()
            .or_else(|| inspected.map(|(_, m)| m));
        let Some(meta) = cached_meta else {
            return Vec::new();
        };
        if meta.category == DeviceHudCategory::Bluetooth {
            return self.sync_bluetooth_changes(now);
        }
        self.emit_debounced(container_key, meta, false, None, now)
            .into_iter()
            .collect()
    }

    pub fn sync_bluetooth_changes(&mut self, now: Instant) -> Vec<DeviceHudEvent> {
        let snaps = enumerate_bluetooth_snapshots();
        let mut events = Vec::new();
        let mut seen_keys = Vec::with_capacity(snaps.len());

        for snap in snaps {
            seen_keys.push(snap.container_key.clone());
            self.upsert_better_meta(
                &snap.container_key,
                CachedDeviceMeta {
                    name: snap.name.clone(),
                    category: DeviceHudCategory::Bluetooth,
                },
            );
            let prev = self.bluetooth_states.get(&snap.container_key).cloned();
            let prev_connected = prev.as_ref().is_some_and(|p| p.connected);
            let prev_bat = prev.as_ref().and_then(|p| p.battery_percent);

            if snap.connected && !prev_connected {
                if snap.battery_percent.is_none() {
                    let check_at = now + Duration::from_millis(850);
                    if !self
                        .pending_battery_checks
                        .iter()
                        .any(|(k, _)| k == &snap.container_key)
                    {
                        self.pending_battery_checks
                            .push((snap.container_key.clone(), check_at));
                    }
                }
                if let Some(ev) = self.emit_debounced(
                    snap.container_key.clone(),
                    CachedDeviceMeta {
                        name: snap.name.clone(),
                        category: DeviceHudCategory::Bluetooth,
                    },
                    true,
                    snap.battery_percent,
                    now,
                ) {
                    events.push(ev);
                }
            } else if !snap.connected && prev_connected {
                self.pending_battery_checks
                    .retain(|(k, _)| k != &snap.container_key);
                if let Some(ev) = self.emit_debounced(
                    snap.container_key.clone(),
                    CachedDeviceMeta {
                        name: snap.name.clone(),
                        category: DeviceHudCategory::Bluetooth,
                    },
                    false,
                    None,
                    now,
                ) {
                    events.push(ev);
                }
            } else if snap.connected && prev_connected && snap.battery_percent != prev_bat && snap.battery_percent.is_some() {
                events.push(DeviceHudEvent {
                    container_key: snap.container_key.clone(),
                    name: snap.name.clone(),
                    category: DeviceHudCategory::Bluetooth,
                    connected: true,
                    battery_percent: snap.battery_percent,
                    is_battery_refresh: true,
                });
            }
            self.bluetooth_states.insert(snap.container_key.clone(), snap);
        }

        // Nếu thiết bị Bluetooth vừa bị gỡ khỏi hệ thống lúc đang kết nối
        let removed_keys: Vec<String> = self
            .bluetooth_states
            .iter()
            .filter(|(k, v)| v.connected && !seen_keys.contains(k))
            .map(|(k, _)| k.clone())
            .collect();
        for key in removed_keys {
            if let Some(mut prev) = self.bluetooth_states.get(&key).cloned() {
                prev.connected = false;
                prev.battery_percent = None;
                self.bluetooth_states.insert(key.clone(), prev.clone());
                if let Some(ev) = self.emit_debounced(
                    key,
                    CachedDeviceMeta {
                        name: prev.name,
                        category: DeviceHudCategory::Bluetooth,
                    },
                    false,
                    None,
                    now,
                ) {
                    events.push(ev);
                }
            }
        }
        events
    }

    pub fn handle_audio_endpoint_change(
        &mut self,
        name: String,
        connected: bool,
        is_bluetooth: bool,
        now: Instant,
    ) -> Option<DeviceHudEvent> {
        if is_bluetooth {
            let mut bth = self.sync_bluetooth_changes(now);
            return bth.pop();
        }
        let clean = name.trim();
        if clean.is_empty() {
            return None;
        }
        let key = format!("AUDIO:{}", clean.to_ascii_lowercase());
        // Nếu có một thiết bị vật lý USB/Headphones cùng tên vừa được PnP phát trong 1s qua -> bỏ qua để tránh đúp.
        let dup_pnp = self.recent_emits.iter().any(|(k, (c, at))| {
            *c == connected
                && now.saturating_duration_since(*at) < DEBOUNCE_WINDOW
                && (k == &key
                    || self
                        .known_meta
                        .get(k)
                        .is_some_and(|m| m.name.eq_ignore_ascii_case(clean) || clean.contains(&m.name)))
        });
        if dup_pnp {
            return None;
        }
        self.emit_debounced(
            key,
            CachedDeviceMeta {
                name: clean.to_string(),
                category: DeviceHudCategory::Headphones,
            },
            connected,
            None,
            now,
        )
    }

    fn emit_debounced(
        &mut self,
        container_key: String,
        meta: CachedDeviceMeta,
        connected: bool,
        battery_percent: Option<u8>,
        now: Instant,
    ) -> Option<DeviceHudEvent> {
        if let Some((prev_conn, prev_at)) = self.recent_emits.get(&container_key)
            && *prev_conn == connected
            && now.saturating_duration_since(*prev_at) < DEBOUNCE_WINDOW
        {
            return None;
        }
        self.recent_emits.retain(|_, (_, at)| now.saturating_duration_since(*at) < Duration::from_secs(10));
        self.recent_emits.insert(container_key.clone(), (connected, now));
        Some(DeviceHudEvent {
            container_key,
            name: meta.name,
            category: meta.category,
            connected,
            battery_percent,
            is_battery_refresh: false,
        })
    }
}

pub struct DeviceNotificationGuard {
    handles: Vec<HCMNOTIFICATION>,
    ctx_ptrs: Vec<*mut Sender<AudioThreadMsg>>,
}

impl DeviceNotificationGuard {
    pub fn register(tx: &Sender<AudioThreadMsg>) -> Self {
        let mut guard = Self {
            handles: Vec::with_capacity(2),
            ctx_ptrs: Vec::with_capacity(2),
        };
        let iface_filter = CM_NOTIFY_FILTER {
            cbSize: size_of::<CM_NOTIFY_FILTER>() as u32,
            Flags: CM_NOTIFY_FILTER_FLAG_ALL_INTERFACE_CLASSES,
            FilterType: CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE,
            Reserved: 0,
            u: Default::default(),
        };
        guard.register_filter(&iface_filter, tx);

        let inst_filter = CM_NOTIFY_FILTER {
            cbSize: size_of::<CM_NOTIFY_FILTER>() as u32,
            Flags: CM_NOTIFY_FILTER_FLAG_ALL_DEVICE_INSTANCES,
            FilterType: CM_NOTIFY_FILTER_TYPE_DEVICEINSTANCE,
            Reserved: 0,
            u: Default::default(),
        };
        guard.register_filter(&inst_filter, tx);
        guard
    }

    fn register_filter(&mut self, filter: &CM_NOTIFY_FILTER, tx: &Sender<AudioThreadMsg>) {
        let raw_ctx = Box::into_raw(Box::new(tx.clone()));
        let mut handle = HCMNOTIFICATION::default();
        let ret = unsafe {
            CM_Register_Notification(
                filter,
                Some(raw_ctx.cast()),
                Some(cm_notify_callback),
                &mut handle,
            )
        };
        if ret == CR_SUCCESS && !handle.is_invalid() {
            self.handles.push(handle);
            self.ctx_ptrs.push(raw_ctx);
        } else {
            unsafe {
                drop(Box::from_raw(raw_ctx));
            }
        }
    }
}

impl Drop for DeviceNotificationGuard {
    fn drop(&mut self) {
        for handle in self.handles.drain(..) {
            if !handle.is_invalid() {
                unsafe {
                    let _ = CM_Unregister_Notification(handle);
                }
            }
        }
        for ptr in self.ctx_ptrs.drain(..) {
            if !ptr.is_null() {
                unsafe {
                    drop(Box::from_raw(ptr));
                }
            }
        }
    }
}

unsafe extern "system" fn cm_notify_callback(
    _hnotify: HCMNOTIFICATION,
    context: *const c_void,
    action: CM_NOTIFY_ACTION,
    eventdata: *const CM_NOTIFY_EVENT_DATA,
    _eventdatasize: u32,
) -> u32 {
    if context.is_null() || eventdata.is_null() {
        return 0;
    }
    let tx = unsafe { &*(context as *const Sender<AudioThreadMsg>) };
    let data = unsafe { &*eventdata };
    if data.FilterType == CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE {
        let arrival = match action {
            a if a == CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL => true,
            a if a == CM_NOTIFY_ACTION_DEVICEINTERFACEREMOVAL => false,
            _ => return 0,
        };
        let sym_ptr = unsafe { data.u.DeviceInterface.SymbolicLink.as_ptr() };
        let sym = unsafe { read_wide_cstr(sym_ptr, 512) };
        if let Some(instance_id) = read_interface_instance_id(&sym) {
            let _ = tx.send(AudioThreadMsg::PnpDeviceNotify {
                instance_id,
                arrival,
            });
        }
    } else if data.FilterType == CM_NOTIFY_FILTER_TYPE_DEVICEINSTANCE {
        let arrival = match action {
            a if a == CM_NOTIFY_ACTION_DEVICEINSTANCESTARTED
                || a == CM_NOTIFY_ACTION_DEVICEINSTANCEENUMERATED =>
            {
                true
            }
            a if a == CM_NOTIFY_ACTION_DEVICEINSTANCEREMOVED => false,
            _ => return 0,
        };
        let id_ptr = unsafe { data.u.DeviceInstance.InstanceId.as_ptr() };
        let instance_id = unsafe { read_wide_cstr(id_ptr, 260) };
        if !instance_id.is_empty() {
            let _ = tx.send(AudioThreadMsg::PnpDeviceNotify {
                instance_id,
                arrival,
            });
        }
    }
    0
}

unsafe fn read_wide_cstr(ptr: *const u16, max_len: usize) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0_usize;
    while len < max_len {
        if unsafe { *ptr.add(len) } == 0 {
            break;
        }
        len += 1;
    }
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf16_lossy(slice)
}

fn read_interface_instance_id(symbolic_link: &str) -> Option<String> {
    if symbolic_link.is_empty() {
        return None;
    }
    let wide = to_wide_null(symbolic_link);
    let mut prop_type = DEVPROPTYPE(0);
    let mut buf = [0_u8; 520];
    let mut size = buf.len() as u32;
    let ret = unsafe {
        CM_Get_Device_Interface_PropertyW(
            PCWSTR(wide.as_ptr()),
            &DEVPKEY_Device_InstanceId,
            &mut prop_type,
            Some(buf.as_mut_ptr()),
            &mut size,
            0,
        )
    };
    if ret != CR_SUCCESS || prop_type != DEVPROP_TYPE_STRING {
        return None;
    }
    decode_utf16_bytes(&buf[..size as usize])
}

fn decode_utf16_bytes(bytes: &[u8]) -> Option<String> {
    let words: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .take_while(|&w| w != 0)
        .collect();
    let s = String::from_utf16_lossy(&words).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

fn read_devnode_string_prop(devinst: u32, key: &DEVPROPKEY) -> Option<String> {
    let mut prop_type = DEVPROPTYPE(0);
    let mut buf = [0_u8; 520];
    let mut size = buf.len() as u32;
    let ret = unsafe {
        CM_Get_DevNode_PropertyW(
            devinst,
            key,
            &mut prop_type,
            Some(buf.as_mut_ptr()),
            &mut size,
            0,
        )
    };
    if ret != CR_SUCCESS || prop_type != DEVPROP_TYPE_STRING {
        return None;
    }
    decode_utf16_bytes(&buf[..size as usize])
}

fn read_devnode_guid_prop(devinst: u32, key: &DEVPROPKEY) -> Option<String> {
    let mut prop_type = DEVPROPTYPE(0);
    let mut guid = GUID::zeroed();
    let mut size = size_of::<GUID>() as u32;
    let ret = unsafe {
        CM_Get_DevNode_PropertyW(
            devinst,
            key,
            &mut prop_type,
            Some((&mut guid as *mut GUID).cast()),
            &mut size,
            0,
        )
    };
    if ret != CR_SUCCESS || prop_type != DEVPROP_TYPE_GUID || guid == GUID::zeroed() {
        return None;
    }
    Some(format!("{guid:?}"))
}

fn read_devnode_bool_prop(devinst: u32, key: &DEVPROPKEY) -> Option<bool> {
    let mut prop_type = DEVPROPTYPE(0);
    let mut val = 0_u8;
    let mut size = 1_u32;
    let ret = unsafe {
        CM_Get_DevNode_PropertyW(
            devinst,
            key,
            &mut prop_type,
            Some(&mut val),
            &mut size,
            0,
        )
    };
    if ret != CR_SUCCESS || prop_type != DEVPROP_TYPE_BOOLEAN {
        return None;
    }
    Some(val != 0)
}

fn read_devnode_battery_prop(devinst: u32) -> Option<u8> {
    let mut prop_type = DEVPROPTYPE(0);
    let mut buf = [0_u8; 4];
    let mut size = buf.len() as u32;
    let ret = unsafe {
        CM_Get_DevNode_PropertyW(
            devinst,
            &DEVPKEY_DEVICE_BATTERY_LEVEL,
            &mut prop_type,
            Some(buf.as_mut_ptr()),
            &mut size,
            0,
        )
    };
    if ret != CR_SUCCESS {
        return None;
    }
    let val = if prop_type == DEVPROP_TYPE_BYTE && size >= 1 {
        buf[0] as u32
    } else if prop_type == DEVPROP_TYPE_UINT32 && size >= 4 {
        u32::from_le_bytes(buf)
    } else {
        return None;
    };
    if val <= 100 { Some(val as u8) } else { None }
}

fn enumerate_present_device_ids() -> Vec<String> {
    let mut len = 0_u32;
    let flags = CM_GETIDLIST_FILTER_PRESENT;
    if unsafe { CM_Get_Device_ID_List_SizeW(&mut len, PCWSTR::null(), flags) } != CR_SUCCESS
        || len == 0
    {
        return Vec::new();
    }
    let mut buf = vec![0_u16; len as usize];
    if unsafe { CM_Get_Device_ID_ListW(PCWSTR::null(), &mut buf, flags) } != CR_SUCCESS {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut start = 0_usize;
    for (idx, &ch) in buf.iter().enumerate() {
        if ch == 0 {
            if idx > start {
                out.push(String::from_utf16_lossy(&buf[start..idx]));
            }
            start = idx + 1;
        }
    }
    out
}

fn enumerate_bluetooth_snapshots() -> Vec<BluetoothSnapshot> {
    let all_ids = enumerate_present_device_ids_with_phantom();
    let mut container_battery: HashMap<String, u8> = HashMap::new();
    let mut container_connected: HashMap<String, bool> = HashMap::new();
    let mut primary_nodes: Vec<(String, String, bool)> = Vec::new();

    for id in &all_ids {
        let wide = to_wide_null(id);
        let mut devinst = 0_u32;
        if unsafe { CM_Locate_DevNodeW(&mut devinst, PCWSTR(wide.as_ptr()), CM_LOCATE_DEVNODE_PHANTOM) }
            != CR_SUCCESS
        {
            continue;
        }
        let Some(container) = read_devnode_guid_prop(devinst, &DEVPKEY_Device_ContainerId) else {
            continue;
        };
        if let Some(bat) = read_devnode_battery_prop(devinst) {
            container_battery.entry(container.clone()).or_insert(bat);
        }
        let is_conn = read_devnode_bool_prop(devinst, &DEVPKEY_DEVICE_IS_CONNECTED).unwrap_or(false);
        if is_conn {
            container_connected.insert(container.clone(), true);
        }
        let upper = id.to_ascii_uppercase();
        if upper.starts_with("BTHENUM\\DEV_") || upper.starts_with("BTHLE\\DEV_") {
            let name = read_devnode_string_prop(devinst, &DEVPKEY_Device_FriendlyName)
                .or_else(|| read_devnode_string_prop(devinst, &DEVPKEY_Device_DeviceDesc))
                .unwrap_or_else(|| String::from("Thiết bị Bluetooth"));
            primary_nodes.push((container, name, is_conn));
        }
    }

    let mut by_container: HashMap<String, BluetoothSnapshot> = HashMap::new();
    for (container, name, node_conn) in primary_nodes {
        let connected = node_conn || *container_connected.get(&container).unwrap_or(&false);
        let battery = container_battery.get(&container).copied();
        let replace = by_container.get(&container).is_none_or(|old| {
            connected && !old.connected || name_quality_score(&name) > name_quality_score(&old.name)
        });
        if replace {
            by_container.insert(
                container.clone(),
                BluetoothSnapshot {
                    container_key: container,
                    name,
                    connected,
                    battery_percent: battery,
                },
            );
        }
    }
    by_container.into_values().collect()
}

fn enumerate_present_device_ids_with_phantom() -> Vec<String> {
    let mut len = 0_u32;
    let flags = 0_u32;
    if unsafe { CM_Get_Device_ID_List_SizeW(&mut len, PCWSTR::null(), flags) } != CR_SUCCESS
        || len == 0
    {
        return Vec::new();
    }
    let mut buf = vec![0_u16; len as usize];
    if unsafe { CM_Get_Device_ID_ListW(PCWSTR::null(), &mut buf, flags) } != CR_SUCCESS {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut start = 0_usize;
    for (idx, &ch) in buf.iter().enumerate() {
        if ch == 0 {
            if idx > start {
                out.push(String::from_utf16_lossy(&buf[start..idx]));
            }
            start = idx + 1;
        }
    }
    out
}

fn inspect_instance_meta(instance_id: &str, allow_phantom: bool) -> Option<(String, CachedDeviceMeta)> {
    let upper = instance_id.trim().to_ascii_uppercase();
    if is_ignored_device_instance(&upper) {
        return None;
    }
    let wide = to_wide_null(instance_id);
    let mut devinst = 0_u32;
    let flags = if allow_phantom {
        CM_LOCATE_DEVNODE_PHANTOM
    } else {
        CM_LOCATE_DEVNODE_NORMAL
    };
    if unsafe { CM_Locate_DevNodeW(&mut devinst, PCWSTR(wide.as_ptr()), flags) } != CR_SUCCESS {
        return None;
    }

    let enumerator = read_devnode_string_prop(devinst, &DEVPKEY_Device_EnumeratorName).unwrap_or_default();
    let category_prop =
        read_devnode_string_prop(devinst, &DEVPKEY_DeviceContainer_Category).unwrap_or_default();
    let friendly = read_devnode_string_prop(devinst, &DEVPKEY_Device_FriendlyName);
    let desc = read_devnode_string_prop(devinst, &DEVPKEY_Device_DeviceDesc);
    let container_key = read_devnode_guid_prop(devinst, &DEVPKEY_Device_ContainerId)
        .unwrap_or_else(|| upper.clone());

    let raw_name = friendly.or(desc)?;
    let category = classify_pnp_category(&upper, &enumerator, &category_prop, &raw_name);
    let clean_name = clean_device_display_name(&raw_name, category);
    Some((
        container_key,
        CachedDeviceMeta {
            name: clean_name,
            category,
        },
    ))
}

pub fn is_bluetooth_instance(instance_upper: &str) -> bool {
    instance_upper.starts_with("BTHENUM\\")
        || instance_upper.starts_with("BTHLE\\")
        || instance_upper.starts_with("BTHLEDEVICE\\")
        || instance_upper.starts_with("BTHHFENUM\\")
}

pub fn is_ignored_device_instance(instance_upper: &str) -> bool {
    let ignored_prefixes = [
        "ROOT\\",
        "SWD\\",
        "ACPI\\",
        "PCI\\",
        "UMB\\",
        "HTREE\\",
        "DISPLAY\\",
        "USB\\ROOT_HUB",
        "STORAGE\\VOLUME",
        "HDAUDIO\\FUNC_",
        "INTELAUDIO\\",
    ];
    if ignored_prefixes.iter().any(|p| instance_upper.starts_with(p)) {
        return true;
    }
    // Bỏ qua Bluetooth protocol transport phụ; giữ BTHENUM\DEV_* và BTHLE\DEV_*.
    is_bluetooth_instance(instance_upper)
        && !instance_upper.starts_with("BTHENUM\\DEV_")
        && !instance_upper.starts_with("BTHLE\\DEV_")
        && !instance_upper.starts_with("BTHHFENUM\\")
        && !instance_upper.starts_with("BTHLEDEVICE\\")
}

pub fn classify_pnp_category(
    instance_upper: &str,
    enumerator: &str,
    container_category: &str,
    name: &str,
) -> DeviceHudCategory {
    let enum_l = enumerator.to_ascii_lowercase();
    let cat_l = container_category.to_ascii_lowercase();
    let name_l = name.to_ascii_lowercase();
    if is_bluetooth_instance(instance_upper)
        || enum_l.starts_with("bth")
        || cat_l.contains("bluetooth")
    {
        return DeviceHudCategory::Bluetooth;
    }
    if cat_l.contains("audio")
        || cat_l.contains("headphone")
        || cat_l.contains("headset")
        || name_l.contains("headphone")
        || name_l.contains("headset")
        || name_l.contains("tai nghe")
    {
        return DeviceHudCategory::Headphones;
    }
    if instance_upper.starts_with("USBSTOR\\")
        || instance_upper.starts_with("USB\\")
        || cat_l.contains("storage")
        || name_l.contains("usb")
        || name_l.contains("mass storage")
    {
        return DeviceHudCategory::Usb;
    }
    DeviceHudCategory::Peripheral
}

pub fn clean_device_display_name(raw: &str, category: DeviceHudCategory) -> String {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower == "usb composite device" || lower == "usb input device" {
        return match category {
            DeviceHudCategory::Usb => String::from("Thiết bị USB"),
            DeviceHudCategory::Headphones => String::from("Tai nghe USB"),
            _ => String::from("Thiết bị ngoại vi"),
        };
    }
    if lower == "usb mass storage device" {
        return String::from("Ổ lưu trữ USB");
    }
    if lower == "hid-compliant mouse" {
        return String::from("Chuột ngoại vi");
    }
    if lower == "hid keyboard device" {
        return String::from("Bàn phím ngoại vi");
    }
    trimmed.to_string()
}

fn name_quality_score(name: &str) -> u8 {
    let l = name.to_ascii_lowercase();
    if l == "thiết bị usb"
        || l == "thiết bị ngoại vi"
        || l == "usb composite device"
        || l == "usb input device"
    {
        1
    } else if l.starts_with("hid-") || l.starts_with("hid ") || l == "chuột ngoại vi" || l == "bàn phím ngoại vi" {
        2
    } else {
        4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_pnp_categories_and_filters_internal_nodes() {
        assert!(is_ignored_device_instance(r"ROOT\SYSTEM\0000"));
        assert!(is_ignored_device_instance(r"USB\ROOT_HUB30\4&123"));
        assert!(!is_ignored_device_instance(r"USB\VID_1234&PID_5678\ABC"));
        assert!(!is_ignored_device_instance(
            r"BTHENUM\DEV_E4282D85C378\9&2E5A28BA&0&BLUETOOTHDEVICE_E4282D85C378"
        ));

        assert_eq!(
            classify_pnp_category(
                r"BTHENUM\DEV_123\4",
                "BTHENUM",
                "Communication.Headset.Bluetooth",
                "Diamond"
            ),
            DeviceHudCategory::Bluetooth
        );
        assert_eq!(
            classify_pnp_category(
                r"USB\VID_046D&PID_C52B\1",
                "USB",
                "Input.Mouse",
                "Logitech G Pro"
            ),
            DeviceHudCategory::Usb
        );
        assert_eq!(
            classify_pnp_category(
                r"HID\VID_046D\1",
                "HID",
                "Audio.Headset",
                "HyperX Cloud II"
            ),
            DeviceHudCategory::Headphones
        );
        assert_eq!(
            clean_device_display_name("USB Mass Storage Device", DeviceHudCategory::Usb),
            "Ổ lưu trữ USB"
        );
    }

    #[test]
    fn debounces_duplicate_container_and_audio_events() {
        let mut tracker = DeviceChangeTracker::new_empty();
        let t0 = Instant::now();
        let meta = CachedDeviceMeta {
            name: String::from("Chuột Logitech"),
            category: DeviceHudCategory::Usb,
        };
        tracker.known_meta.insert(String::from("{C1}"), meta.clone());

        let first = tracker.emit_debounced(String::from("{C1}"), meta.clone(), true, None, t0);
        assert!(first.is_some());
        let dup = tracker.emit_debounced(
            String::from("{C1}"),
            meta.clone(),
            true,
            None,
            t0 + Duration::from_millis(200),
        );
        assert!(dup.is_none());

        // Bỏ qua sự kiện Audio trùng tên ngay sau khi PnP đã phát.
        let audio_dup = tracker.handle_audio_endpoint_change(
            String::from("Chuột Logitech"),
            true,
            false,
            t0 + Duration::from_millis(300),
        );
        assert!(audio_dup.is_none());

        // Đủ quá cửa sổ debounce hoặc khác chiều rút ra -> phát bình thường.
        let unplug = tracker.emit_debounced(
            String::from("{C1}"),
            meta,
            false,
            None,
            t0 + Duration::from_millis(250),
        );
        assert!(unplug.is_some_and(|e| !e.connected));
    }
}
