use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

pub const WM_APP_VSYNC_TICK: u32 = WM_APP + 2;
const VISUALIZER_INTERVAL_MS: u64 = 50;

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
}

/// Luồng đồng bộ khung hình theo DWM Compositor (`DwmFlush`) khi chuyển động/kéo và nhịp thấp 50ms cho sóng nhạc, ngủ 0% CPU khi đứng yên.
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
            if mode == TickerMode::VSync {
                // Đảm bảo chuyển sang VSync mượt mà không bị nuốt nhịp bởi tick cũ từ VisualizerSlow.
                self.tick_pending.store(false, Ordering::Release);
            }
            guard.mode = mode;
            cvar.notify_all();
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
    let mut fast_flush_streak = 0_u8;
    loop {
        let mode = {
            let (lock, cvar) = &*shared;
            let Ok(mut guard) = lock.lock() else {
                return;
            };
            while guard.running && guard.mode == TickerMode::Off {
                fast_flush_streak = 0;
                let Ok(next_guard) = cvar.wait(guard) else {
                    return;
                };
                guard = next_guard;
            }
            if !guard.running {
                return;
            }
            guard.mode
        };

        match mode {
            TickerMode::Off => continue,
            TickerMode::VSync => {
                let t0 = Instant::now();
                let flushed = unsafe { DwmFlush() }.is_ok();
                if !flushed {
                    fast_flush_streak = 0;
                    thread::sleep(Duration::from_millis(7));
                } else if t0.elapsed() < Duration::from_micros(400) {
                    fast_flush_streak = fast_flush_streak.saturating_add(1);
                    if fast_flush_streak >= 3 {
                        thread::sleep(Duration::from_millis(7));
                    }
                } else {
                    fast_flush_streak = 0;
                }
            }
            TickerMode::VisualizerSlow => {
                let (lock, cvar) = &*shared;
                let Ok(guard) = lock.lock() else {
                    return;
                };
                if guard.running && guard.mode == TickerMode::VisualizerSlow {
                    let (next_guard, _) = match cvar.wait_timeout(
                        guard,
                        Duration::from_millis(VISUALIZER_INTERVAL_MS),
                    ) {
                        Ok(res) => res,
                        Err(_) => return,
                    };
                    if !next_guard.running || next_guard.mode == TickerMode::Off {
                        continue;
                    }
                    if next_guard.mode == TickerMode::VSync {
                        // Vừa chuyển sang animation khi đang ngủ VisualizerSlow: chạy luôn nhịp VSync không trễ.
                        drop(next_guard);
                        post_tick(hwnd_raw, &tick_pending);
                        continue;
                    }
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

        post_tick(hwnd_raw, &tick_pending);
    }
}

fn post_tick(hwnd_raw: isize, tick_pending: &AtomicBool) {
    if !tick_pending.swap(true, Ordering::AcqRel) {
        let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
        if unsafe { PostMessageW(Some(hwnd), WM_APP_VSYNC_TICK, WPARAM(0), LPARAM(0)) }.is_err() {
            tick_pending.store(false, Ordering::Release);
        }
    }
}
