use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory2, DXGI_CREATE_FACTORY_FLAGS, DXGI_GPU_PREFERENCE_MINIMUM_POWER,
    IDXGIAdapter1, IDXGIFactory6, IDXGIOutput,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

pub const WM_APP_VSYNC_TICK: u32 = WM_APP + 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickerMode {
    Off,
    VSync,
    VisualizerSlow,
}

#[derive(Debug, Clone, Copy)]
struct TickerState {
    running: bool,
    mode: TickerMode,
    target_monitor_rect: Option<(i32, i32, i32, i32)>,
    output_dirty: bool,
}

/// Luồng đồng bộ khung hình chỉ hoạt động khi có animation (VSync gốc) hoặc sóng nhạc (nhịp thấp 42ms), ngủ 0% CPU khi đứng yên.
pub struct VSyncTicker {
    shared: Arc<(Mutex<TickerState>, Condvar)>,
    tick_pending: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl VSyncTicker {
    pub fn start(hwnd: HWND) -> Self {
        let hwnd_raw = hwnd.0 as isize;
        let shared = Arc::new((
            Mutex::new(TickerState {
                running: true,
                mode: TickerMode::Off,
                target_monitor_rect: None,
                output_dirty: true,
            }),
            Condvar::new(),
        ));
        let tick_pending = Arc::new(AtomicBool::new(false));

        let shared_bg = Arc::clone(&shared);
        let pending_bg = Arc::clone(&tick_pending);

        let handle = thread::Builder::new()
            .name(String::from("vsync-ticker"))
            .spawn(move || {
                vsync_loop(hwnd_raw, shared_bg, pending_bg);
            })
            .ok();

        Self {
            shared,
            tick_pending,
            handle,
        }
    }

    pub fn set_mode(&self, mode: TickerMode) {
        let (lock, cvar) = &*self.shared;
        if let Ok(mut guard) = lock.lock()
            && guard.mode != mode
        {
            guard.mode = mode;
            cvar.notify_all();
        }
    }

    pub fn set_monitor_rect(&self, rect: (i32, i32, i32, i32)) {
        let (lock, _cvar) = &*self.shared;
        if let Ok(mut guard) = lock.lock()
            && guard.target_monitor_rect != Some(rect)
        {
            guard.target_monitor_rect = Some(rect);
            guard.output_dirty = true;
        }
    }

    pub fn clear_pending(&self) {
        self.tick_pending.store(false, Ordering::Release);
    }
}

impl Drop for VSyncTicker {
    fn drop(&mut self) {
        let (lock, cvar) = &*self.shared;
        if let Ok(mut guard) = lock.lock() {
            guard.running = false;
            guard.mode = TickerMode::Off;
            cvar.notify_all();
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn vsync_loop(
    hwnd_raw: isize,
    shared: Arc<(Mutex<TickerState>, Condvar)>,
    tick_pending: Arc<AtomicBool>,
) {
    let factory: Option<IDXGIFactory6> =
        unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)) }.ok();
    let mut cached_output: Option<IDXGIOutput> = None;

    loop {
        let mode = {
            let (lock, cvar) = &*shared;
            let Ok(mut guard) = lock.lock() else {
                return;
            };
            while guard.running && guard.mode == TickerMode::Off {
                let Ok(next_guard) = cvar.wait(guard) else {
                    return;
                };
                guard = next_guard;
            }
            if !guard.running {
                return;
            }
            if guard.output_dirty {
                guard.output_dirty = false;
                cached_output = factory
                    .as_ref()
                    .and_then(|f| find_output_for_monitor(f, guard.target_monitor_rect));
            }
            guard.mode
        };

        match mode {
            TickerMode::Off => continue,
            TickerMode::VSync => {
                let waited_vblank = if let Some(out) = &cached_output {
                    unsafe { out.WaitForVBlank() }.is_ok()
                } else {
                    false
                };
                if !waited_vblank {
                    thread::sleep(Duration::from_millis(8));
                }
            }
            TickerMode::VisualizerSlow => {
                let (lock, cvar) = &*shared;
                let Ok(guard) = lock.lock() else {
                    return;
                };
                if guard.running && guard.mode == TickerMode::VisualizerSlow {
                    let _ = cvar.wait_timeout(guard, Duration::from_millis(42));
                }
            }
        }

        let still_active = {
            let (lock, _) = &*shared;
            lock.lock()
                .is_ok_and(|g| g.running && g.mode != TickerMode::Off)
        };
        if !still_active {
            continue;
        }

        if !tick_pending.swap(true, Ordering::AcqRel) {
            let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
            if unsafe { PostMessageW(Some(hwnd), WM_APP_VSYNC_TICK, WPARAM(0), LPARAM(0)) }.is_err()
            {
                tick_pending.store(false, Ordering::Release);
            }
        }
    }
}

fn find_output_for_monitor(
    factory: &IDXGIFactory6,
    target_rect: Option<(i32, i32, i32, i32)>,
) -> Option<IDXGIOutput> {
    let adapter = unsafe {
        factory.EnumAdapterByGpuPreference::<IDXGIAdapter1>(0, DXGI_GPU_PREFERENCE_MINIMUM_POWER)
    }
    .ok()?;

    let mut fallback: Option<IDXGIOutput> = None;
    let mut index = 0u32;
    while let Ok(output) = unsafe { adapter.EnumOutputs(index) } {
        if fallback.is_none() {
            fallback = Some(output.clone());
        }
        if let Some((l, t, r, b)) = target_rect
            && let Ok(desc) = unsafe { output.GetDesc() }
        {
            let d: RECT = desc.DesktopCoordinates;
            if d.left == l && d.top == t && d.right == r && d.bottom == b {
                return Some(output);
            }
        }
        index += 1;
    }
    fallback
}
