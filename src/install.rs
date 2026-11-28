#![allow(dead_code)]

use std::fs;
use std::mem::size_of;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, MAX_PATH, WPARAM};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize, IPersistFile,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_DWORD, REG_OPTION_NON_VOLATILE,
    REG_SZ, REG_VALUE_TYPE, RegCloseKey, RegCreateKeyExW, RegDeleteKeyW, RegDeleteValueW,
    RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::Win32::System::Threading::{
    CREATE_NO_WINDOW, GetCurrentProcessId, OpenMutexW, OpenProcess, PROCESS_SYNCHRONIZE,
    PROCESS_TERMINATE, SYNCHRONIZATION_ACCESS_RIGHTS, TerminateProcess, WaitForSingleObject,
};
use windows::Win32::UI::Shell::{
    CSIDL_DESKTOPDIRECTORY, CSIDL_PROGRAMS, IShellLinkW, SHGetFolderPathW, ShellLink,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowW, GetClassNameW, GetWindowThreadProcessId, IDYES, MB_DEFBUTTON2,
    MB_ICONERROR, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK, MB_YESNO, MessageBoxW, PostMessageW,
    WM_COMMAND,
};
use windows::core::{BOOL, Interface, PCWSTR};

pub const APP_NAME: &str = "Win Dynamic Island";
pub const APP_PUBLISHER: &str = "kha0305";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_ABOUT_URL: &str = "https://github.com/kha0305/dynamic-island-win";
pub const WINDOW_CLASS_NAME: &str = "WinDynamicIslandWindowClass";
pub const INSTANCE_MUTEX_NAME: &str = "Local\\WinDynamicIslandSingleInstanceMutex";
pub const EXIT_COMMAND_ID: usize = 1099;
pub const UNINSTALL_FLAG: &str = "--uninstall";
pub const QUIET_FLAG: &str = "--quiet";
pub const SILENT_FLAG: &str = "--silent";
pub const SHORTCUT_FILE_NAME: &str = "Win Dynamic Island.lnk";
pub const EXE_FILE_NAME: &str = "win-dynamic-island.exe";

const RUN_KEY_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const STARTUP_APPROVED_RUN_KEY_PATH: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run";
const UNINSTALL_KEY_PATH: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\WinDynamicIsland";
const RUN_VALUE_NAME: &str = "WinDynamicIsland";

struct RegGuard(HKEY);

impl Drop for RegGuard {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = RegCloseKey(self.0);
            }
        }
    }
}

pub struct ComApartment(bool);

impl ComApartment {
    pub fn init_sta() -> Self {
        let ok = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
        Self(ok)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

pub fn to_wide_null(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn known_shell_folder(csidl: i32) -> Option<PathBuf> {
    let mut buf = [0u16; MAX_PATH as usize];
    let hr = unsafe { SHGetFolderPathW(None, csidl, None, 0, &mut buf) };
    if hr.is_ok() {
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        if len > 0 {
            return Some(PathBuf::from(String::from_utf16_lossy(&buf[..len])));
        }
    }
    None
}

pub fn install_dir() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    if base.is_empty() {
        return None;
    }
    Some(
        PathBuf::from(base)
            .join("Programs")
            .join("WinDynamicIsland"),
    )
}

pub fn installed_exe_path() -> Option<PathBuf> {
    Some(install_dir()?.join(EXE_FILE_NAME))
}

pub fn start_menu_shortcut_path() -> Option<PathBuf> {
    if let Some(programs) = known_shell_folder(CSIDL_PROGRAMS as i32) {
        return Some(programs.join(SHORTCUT_FILE_NAME));
    }
    let appdata = std::env::var_os("APPDATA")?;
    Some(
        PathBuf::from(appdata)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join(SHORTCUT_FILE_NAME),
    )
}

pub fn desktop_shortcut_path() -> Option<PathBuf> {
    if let Some(desktop) = known_shell_folder(CSIDL_DESKTOPDIRECTORY as i32) {
        return Some(desktop.join(SHORTCUT_FILE_NAME));
    }
    let profile = std::env::var_os("USERPROFILE")?;
    Some(PathBuf::from(profile).join("Desktop").join(SHORTCUT_FILE_NAME))
}

pub fn format_autostart_command(exe_path: &Path) -> String {
    format!("\"{}\"", exe_path.display())
}

fn set_reg_sz(hkey: HKEY, name: &str, value: &str) -> Result<(), String> {
    let name_w = to_wide_null(name);
    let val_w = to_wide_null(value);
    let bytes: Vec<u8> = val_w.iter().flat_map(|c| c.to_le_bytes()).collect();
    let res = unsafe {
        RegSetValueExW(
            hkey,
            PCWSTR(name_w.as_ptr()),
            None,
            REG_SZ,
            Some(&bytes),
        )
    };
    if res.is_ok() {
        Ok(())
    } else {
        Err(format!("Không thể ghi giá trị Registry {name}."))
    }
}

fn set_reg_dword(hkey: HKEY, name: &str, value: u32) -> Result<(), String> {
    let name_w = to_wide_null(name);
    let bytes = value.to_le_bytes();
    let res = unsafe {
        RegSetValueExW(
            hkey,
            PCWSTR(name_w.as_ptr()),
            None,
            REG_DWORD,
            Some(&bytes),
        )
    };
    if res.is_ok() {
        Ok(())
    } else {
        Err(format!("Không thể ghi khóa DWORD Registry {name}."))
    }
}

pub fn is_existing_autostart_enabled() -> bool {
    let subkey = to_wide_null(RUN_KEY_PATH);
    let val_w = to_wide_null(RUN_VALUE_NAME);
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
    let _guard = RegGuard(hkey);
    let mut reg_type = REG_VALUE_TYPE::default();
    let mut byte_len = 0u32;
    let err = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(val_w.as_ptr()),
            None,
            Some(&mut reg_type),
            None,
            Some(&mut byte_len),
        )
    };
    err.is_ok() && byte_len > 2
}

pub fn set_autostart_for_exe(exe_path: &Path, enabled: bool) -> bool {
    let subkey = to_wide_null(RUN_KEY_PATH);
    let value_name = to_wide_null(RUN_VALUE_NAME);
    clear_startup_approved_override();

    if enabled {
        let cmd = format_autostart_command(exe_path);
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
        let _guard = RegGuard(hkey);
        set_reg_sz(hkey, RUN_VALUE_NAME, &cmd).is_ok()
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
        let _guard = RegGuard(hkey);
        let _ = unsafe { RegDeleteValueW(hkey, PCWSTR(value_name.as_ptr())) };
        true
    }
}

pub fn clear_startup_approved_override() {
    let subkey = to_wide_null(STARTUP_APPROVED_RUN_KEY_PATH);
    let val_w = to_wide_null(RUN_VALUE_NAME);
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
    if open_err.is_ok() {
        let _guard = RegGuard(hkey);
        unsafe {
            let _ = RegDeleteValueW(hkey, PCWSTR(val_w.as_ptr()));
        }
    }
}

pub fn create_shortcut(
    shortcut_path: &Path,
    target_exe: &Path,
    working_dir: &Path,
    description: &str,
) -> Result<(), String> {
    if let Some(parent) = shortcut_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let link: IShellLinkW =
        unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }
            .map_err(|e| format!("Không tạo được đối tượng ShellLink: {e}"))?;

    let target_w = to_wide_null(&target_exe.to_string_lossy());
    let dir_w = to_wide_null(&working_dir.to_string_lossy());
    let desc_w = to_wide_null(description);
    let lnk_w = to_wide_null(&shortcut_path.to_string_lossy());

    unsafe {
        link.SetPath(PCWSTR(target_w.as_ptr()))
            .map_err(|e| format!("Lỗi đặt đường dẫn shortcut: {e}"))?;
        link.SetWorkingDirectory(PCWSTR(dir_w.as_ptr()))
            .map_err(|e| format!("Lỗi đặt thư mục làm việc shortcut: {e}"))?;
        link.SetDescription(PCWSTR(desc_w.as_ptr()))
            .map_err(|e| format!("Lỗi đặt mô tả shortcut: {e}"))?;
        link.SetIconLocation(PCWSTR(target_w.as_ptr()), 0)
            .map_err(|e| format!("Lỗi gắn biểu tượng shortcut: {e}"))?;
        let persist: IPersistFile = link
            .cast()
            .map_err(|e| format!("Lỗi lấy giao diện IPersistFile: {e}"))?;
        persist
            .Save(PCWSTR(lnk_w.as_ptr()), true)
            .map_err(|e| format!("Không thể lưu tệp shortcut: {e}"))?;
    }
    Ok(())
}

pub fn register_uninstall_entry(
    exe_path: &Path,
    dir_path: &Path,
    exe_bytes_len: u64,
) -> Result<(), String> {
    let subkey = to_wide_null(UNINSTALL_KEY_PATH);
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
        return Err(String::from(
            "Không thể tạo khóa gỡ cài đặt trong Registry.",
        ));
    }
    let _guard = RegGuard(hkey);

    let exe_str = exe_path.to_string_lossy();
    let dir_str = dir_path.to_string_lossy();
    set_reg_sz(hkey, "DisplayName", APP_NAME)?;
    set_reg_sz(hkey, "DisplayVersion", APP_VERSION)?;
    set_reg_sz(hkey, "Publisher", APP_PUBLISHER)?;
    set_reg_sz(hkey, "DisplayIcon", &format!("{exe_str},0"))?;
    set_reg_sz(hkey, "InstallLocation", &dir_str)?;
    set_reg_sz(
        hkey,
        "UninstallString",
        &format!("\"{exe_str}\" {UNINSTALL_FLAG}"),
    )?;
    set_reg_sz(
        hkey,
        "QuietUninstallString",
        &format!("\"{exe_str}\" {UNINSTALL_FLAG} {QUIET_FLAG}"),
    )?;
    set_reg_sz(hkey, "URLInfoAbout", APP_ABOUT_URL)?;
    set_reg_dword(hkey, "NoModify", 1)?;
    set_reg_dword(hkey, "NoRepair", 1)?;
    let size_kb = ((exe_bytes_len / 1024).max(1)).min(u32::MAX as u64) as u32;
    set_reg_dword(hkey, "EstimatedSize", size_kb)?;
    Ok(())
}

pub fn remove_uninstall_entry() {
    let subkey = to_wide_null(UNINSTALL_KEY_PATH);
    unsafe {
        let _ = RegDeleteKeyW(HKEY_CURRENT_USER, PCWSTR(subkey.as_ptr()));
    }
}

fn is_instance_mutex_alive() -> bool {
    const MUTEX_SYNCHRONIZE: SYNCHRONIZATION_ACCESS_RIGHTS = SYNCHRONIZATION_ACCESS_RIGHTS(0x0010_0000);
    let mutex_w = to_wide_null(INSTANCE_MUTEX_NAME);
    if let Ok(handle) = unsafe { OpenMutexW(MUTEX_SYNCHRONIZE, false, PCWSTR(mutex_w.as_ptr())) }
        && !handle.is_invalid()
    {
        unsafe {
            let _ = CloseHandle(handle);
        }
        return true;
    }
    false
}

fn find_other_running_app_pids() -> Vec<u32> {
    let self_pid = unsafe { GetCurrentProcessId() };
    let Ok(snap) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return Vec::new();
    };
    if snap.is_invalid() {
        return Vec::new();
    }
    struct SnapGuard(HANDLE);
    impl Drop for SnapGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    let _guard = SnapGuard(snap);

    let mut pids = Vec::new();
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    if unsafe { Process32FirstW(snap, &mut entry) }.is_ok() {
        loop {
            let end = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
            if entry.th32ProcessID != self_pid && name.eq_ignore_ascii_case(EXE_FILE_NAME) {
                pids.push(entry.th32ProcessID);
            }
            if unsafe { Process32NextW(snap, &mut entry) }.is_err() {
                break;
            }
        }
    }
    pids
}

unsafe extern "system" fn close_island_window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let mut pid = 0_u32;
    let _ = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    let target_pids = unsafe { &*(lparam.0 as *const Vec<u32>) };
    if target_pids.contains(&pid) {
        let mut buf = [0_u16; 64];
        let len = unsafe { GetClassNameW(hwnd, &mut buf) };
        if len > 0 {
            let cls = String::from_utf16_lossy(&buf[..len as usize]);
            if cls == WINDOW_CLASS_NAME {
                unsafe {
                    let _ = PostMessageW(
                        Some(hwnd),
                        WM_COMMAND,
                        WPARAM(EXIT_COMMAND_ID),
                        LPARAM(0),
                    );
                }
            }
        }
    }
    BOOL(1)
}

pub fn stop_running_instance_and_wait() {
    let initial_pids = find_other_running_app_pids();
    let class_w = to_wide_null(WINDOW_CLASS_NAME);
    if !initial_pids.is_empty() {
        unsafe {
            let _ = EnumWindows(
                Some(close_island_window_proc),
                LPARAM((&raw const initial_pids) as isize),
            );
        }
    }
    if let Ok(existing) = unsafe { FindWindowW(PCWSTR(class_w.as_ptr()), PCWSTR::null()) }
        && !existing.is_invalid()
    {
        unsafe {
            let _ = PostMessageW(
                Some(existing),
                WM_COMMAND,
                WPARAM(EXIT_COMMAND_ID),
                LPARAM(0),
            );
        }
    }

    for _ in 0..15 {
        let still_running = !find_other_running_app_pids().is_empty();
        if !still_running && !is_instance_mutex_alive() {
            thread::sleep(Duration::from_millis(80));
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }

    // Nếu tiến trình cũ treo hoặc mất cửa sổ mà vẫn giữ khóa tệp/mutex -> kết thúc cưỡng bức để cài đè không lỗi.
    for pid in find_other_running_app_pids() {
        if let Ok(proc_handle) =
            unsafe { OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, false, pid) }
        {
            unsafe {
                let _ = TerminateProcess(proc_handle, 0);
                let _ = WaitForSingleObject(proc_handle, 2_500);
                let _ = CloseHandle(proc_handle);
            }
        }
    }
    thread::sleep(Duration::from_millis(120));
}

pub fn install_to_dir(
    target_dir: &Path,
    payload: &[u8],
    enable_autostart: bool,
    desktop_shortcut: Option<&Path>,
    start_menu_shortcut: Option<&Path>,
    register_arp: bool,
) -> Result<PathBuf, String> {
    if payload.len() < 64 * 1024 || !payload.starts_with(b"MZ") {
        return Err(String::from(
            "Dữ liệu cài đặt ứng dụng không hợp lệ hoặc bị thiếu.",
        ));
    }

    fs::create_dir_all(target_dir)
        .map_err(|e| format!("Không thể tạo thư mục cài đặt: {e}"))?;

    let target_exe = target_dir.join(EXE_FILE_NAME);
    let staging = target_dir.join("win-dynamic-island.exe.staging");
    let _ = fs::remove_file(&staging);
    fs::write(&staging, payload).map_err(|e| format!("Không thể ghi tệp cài đặt tạm: {e}"))?;

    if target_exe.exists() {
        let mut replaced = false;
        for _ in 0..10 {
            if fs::rename(&staging, &target_exe).is_ok() {
                replaced = true;
                break;
            }
            if fs::remove_file(&target_exe).is_ok() && fs::rename(&staging, &target_exe).is_ok() {
                replaced = true;
                break;
            }
            thread::sleep(Duration::from_millis(150));
        }
        if !replaced {
            let _ = fs::remove_file(&staging);
            return Err(String::from(
                "Không thể thay thế tệp đang chạy. Vui lòng đóng Win Dynamic Island rồi thử lại.",
            ));
        }
    } else if let Err(e) = fs::rename(&staging, &target_exe) {
        let _ = fs::remove_file(&staging);
        return Err(format!("Không thể hoàn tất chép tệp cài đặt: {e}"));
    }

    let _com = ComApartment::init_sta();
    if let Some(sm) = start_menu_shortcut {
        create_shortcut(
            sm,
            &target_exe,
            target_dir,
            "Thanh viên thuốc thông minh cho Windows 11",
        )?;
    }
    if let Some(dk) = desktop_shortcut {
        create_shortcut(
            dk,
            &target_exe,
            target_dir,
            "Thanh viên thuốc thông minh cho Windows 11",
        )?;
    }
    if register_arp {
        register_uninstall_entry(&target_exe, target_dir, payload.len() as u64)?;
        let _ = set_autostart_for_exe(&target_exe, enable_autostart);
    }

    Ok(target_exe)
}

pub fn launch_installed_app(exe_path: &Path) -> Result<(), String> {
    // Khởi chạy qua explorer.exe để luôn giữ mức Medium IL (0x2000), bảo đảm OLE Drag-Drop hoạt động.
    if Command::new("explorer.exe").arg(exe_path).spawn().is_ok() {
        return Ok(());
    }
    Command::new(exe_path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Không thể khởi chạy Win Dynamic Island: {e}"))
}

pub fn show_message_box(title: &str, body: &str, flags: u32) -> i32 {
    let t = to_wide_null(title);
    let b = to_wide_null(body);
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(b.as_ptr()),
            PCWSTR(t.as_ptr()),
            windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_STYLE(flags),
        )
        .0
    }
}

fn query_uninstall_install_location() -> Option<PathBuf> {
    let subkey = to_wide_null(UNINSTALL_KEY_PATH);
    let val_w = to_wide_null("InstallLocation");
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
        return None;
    }
    let _guard = RegGuard(hkey);
    let mut reg_type = REG_VALUE_TYPE::default();
    let mut byte_len = 0u32;
    let probe = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(val_w.as_ptr()),
            None,
            Some(&mut reg_type),
            None,
            Some(&mut byte_len),
        )
    };
    if probe.is_err() || !(2..=8192).contains(&byte_len) {
        return None;
    }
    let mut buf = vec![0u8; byte_len as usize];
    let read = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(val_w.as_ptr()),
            None,
            Some(&mut reg_type),
            Some(buf.as_mut_ptr()),
            Some(&mut byte_len),
        )
    };
    if read.is_err() || reg_type != REG_SZ {
        return None;
    }
    let u16_count = (byte_len as usize) / 2;
    let mut units = Vec::with_capacity(u16_count);
    for &[lo, hi] in buf[..u16_count * 2].as_chunks::<2>().0 {
        let u = u16::from_le_bytes([lo, hi]);
        if u == 0 {
            break;
        }
        units.push(u);
    }
    let s = String::from_utf16_lossy(&units);
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

pub fn run_uninstall_flow(quiet: bool) -> i32 {
    if !quiet {
        let answer = show_message_box(
            "Gỡ cài đặt Win Dynamic Island",
            "Bạn có chắc muốn gỡ cài đặt Win Dynamic Island khỏi máy tính không?\n\nCác tệp cá nhân trên ổ đĩa của bạn vẫn được giữ nguyên an toàn.",
            MB_YESNO.0 | MB_ICONQUESTION.0 | MB_DEFBUTTON2.0,
        );
        if answer != IDYES.0 {
            return 0;
        }
    }

    stop_running_instance_and_wait();

    let target_dir = query_uninstall_install_location().or_else(install_dir);

    if let Some(sm) = start_menu_shortcut_path() {
        let _ = fs::remove_file(sm);
    }
    if let Some(dk) = desktop_shortcut_path() {
        let _ = fs::remove_file(dk);
    }
    let _ = set_autostart_for_exe(Path::new(""), false);
    remove_uninstall_entry();

    if let Some(dir) = target_dir {
        let exe_path = dir.join(EXE_FILE_NAME);
        let current = std::env::current_exe().ok();
        let running_from_install = current.as_ref().is_some_and(|cur| {
            cur.to_string_lossy()
                .eq_ignore_ascii_case(&exe_path.to_string_lossy())
        });
        if running_from_install {
            let exe_s = exe_path.to_string_lossy().replace('"', "");
            let dir_s = dir.to_string_lossy().replace('"', "");
            let raw = format!(
                "/S /C \"ping -n 2 127.0.0.1 >nul & del /f /q \"{exe_s}\" & rmdir \"{dir_s}\"\"",
            );
            let _ = Command::new("cmd.exe")
                .raw_arg(raw)
                .creation_flags(CREATE_NO_WINDOW.0)
                .spawn();
        } else {
            let _ = fs::remove_file(&exe_path);
            let _ = fs::remove_dir(&dir);
        }
    }

    if !quiet {
        show_message_box(
            "Gỡ cài đặt hoàn tất",
            "Đã gỡ cài đặt Win Dynamic Island khỏi Windows.",
            MB_OK.0 | MB_ICONINFORMATION.0,
        );
    }
    0
}

pub fn report_setup_error(title: &str, msg: &str, silent: bool) {
    if !silent {
        let _ = show_message_box(title, msg, MB_OK.0 | MB_ICONERROR.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_to_dir_writes_binary_and_rejects_invalid_payload() {
        let tmp = std::env::temp_dir().join("wdi_install_unit_test");
        let _ = fs::remove_dir_all(&tmp);

        let bad = install_to_dir(&tmp, b"not-an-exe", false, None, None, false);
        assert!(bad.is_err());

        let mut valid_pe = vec![0u8; 70 * 1024];
        valid_pe[0] = b'M';
        valid_pe[1] = b'Z';
        let installed = install_to_dir(&tmp, &valid_pe, false, None, None, false)
            .expect("Phai ghi duoc tep cai dat hop le");
        assert!(installed.is_file());
        assert_eq!(
            fs::metadata(&installed).expect("Co metadata").len(),
            valid_pe.len() as u64
        );

        let _ = fs::remove_dir_all(&tmp);
    }
}
