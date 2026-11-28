#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod anti;
mod config;
mod hud;
mod json_mini;
mod media;
mod renderer;
mod shelf;
mod state;
mod tray;
mod updater;
mod vsync;
mod watcher;
mod window;

use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, LPARAM, WPARAM};
use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, FindWindowW, GetMessageW, MSG, PostMessageW, TranslateMessage,
};
use windows::core::PCWSTR;

use crate::tray::to_wide_null;
use crate::window::{IslandWindow, WINDOW_CLASS_NAME, WM_APP_WAKE_INSTANCE};

const INSTANCE_MUTEX_NAME: &str = "Local\\WinDynamicIslandSingleInstanceMutex";

struct SingleInstanceGuard(HANDLE);

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

fn acquire_single_instance_named(name: &str) -> windows::core::Result<Option<SingleInstanceGuard>> {
    let wide = to_wide_null(name);
    let handle = unsafe { CreateMutexW(None, false, PCWSTR(wide.as_ptr())) }?;
    let last_err = unsafe { GetLastError() };
    if last_err == ERROR_ALREADY_EXISTS {
        unsafe {
            let _ = CloseHandle(handle);
        }
        return Ok(None);
    }
    Ok(Some(SingleInstanceGuard(handle)))
}

fn wake_existing_instance() {
    let class_w = to_wide_null(WINDOW_CLASS_NAME);
    if let Ok(existing) = unsafe { FindWindowW(PCWSTR(class_w.as_ptr()), PCWSTR::null()) }
        && !existing.is_invalid()
    {
        unsafe {
            let _ = PostMessageW(Some(existing), WM_APP_WAKE_INSTANCE, WPARAM(0), LPARAM(0));
        }
    }
}

struct ComApartmentGuard(bool);

impl ComApartmentGuard {
    fn init() -> Self {
        let initialized = unsafe { OleInitialize(None) }.is_ok();
        Self(initialized)
    }
}

impl Drop for ComApartmentGuard {
    fn drop(&mut self) {
        if self.0 {
            unsafe { OleUninitialize() };
        }
    }
}

fn enable_per_monitor_dpi_v2() {
    unsafe {
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_err() {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE);
        }
    }
}

fn run() -> windows::core::Result<()> {
    let post_update = std::env::args().any(|arg| arg == updater::POST_UPDATE_ARG);
    let _instance_guard = if post_update {
        let mut acquired = None;
        for _ in 0..40 {
            if let Some(guard) = acquire_single_instance_named(INSTANCE_MUTEX_NAME)? {
                acquired = Some(guard);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let Some(guard) = acquired else {
            wake_existing_instance();
            return Ok(());
        };
        guard
    } else {
        let Some(guard) = acquire_single_instance_named(INSTANCE_MUTEX_NAME)? else {
            wake_existing_instance();
            return Ok(());
        };
        guard
    };

    updater::spawn_cleanup_old_binary();
    enable_per_monitor_dpi_v2();
    let _com = ComApartmentGuard::init();
    let _window = IslandWindow::create()?;

    let mut msg = MSG::default();
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

fn parse_anti_hook_arg() -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == anti::ANTI_HOOK_FLAG {
            return Some(args.next().unwrap_or_else(|| String::from("PostInvocation")));
        }
    }
    None
}

fn main() {
    if let Some(event_name) = parse_anti_hook_arg() {
        let code = anti::run_anti_hook_cli(&event_name);
        std::process::exit(code);
    }
    if let Err(err) = run() {
        eprintln!("Loi khoi tao Win Dynamic Island: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_instance_mutex_blocks_duplicate_process() {
        let test_name = "Local\\WinDynamicIslandUnitTestMutex_97f0bbbf";
        let first = acquire_single_instance_named(test_name)
            .expect("Lan dau tao mutex phai thanh cong")
            .expect("Phai giu duoc lock");
        let second = acquire_single_instance_named(test_name)
            .expect("Kiem tra mutex trung khong duoc loi");
        assert!(second.is_none(), "Lan thu hai phai bi chan mo trung");
        drop(first);

        let third = acquire_single_instance_named(test_name)
            .expect("Sau khi giai phong mutex phai tao lai duoc");
        assert!(third.is_some());
    }
}
