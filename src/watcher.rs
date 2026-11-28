use std::mem::size_of;
use std::sync::atomic::{AtomicIsize, Ordering};

use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentThreadId, OpenProcess, PROCESS_NAME_FORMAT,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Shell::{
    QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ASFW_ANY, AllowSetForegroundWindow, BringWindowToTop, EVENT_SYSTEM_FOREGROUND, EnumWindows,
    GetClassNameW, GetForegroundWindow, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
    IsIconic, IsWindowVisible, PostMessageW, SW_RESTORE, SetForegroundWindow, ShowWindow,
    SwitchToThisWindow, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_APP,
};
use windows::core::{BOOL, PWSTR};

use crate::anti::{AntiSource, classify_exe_name_to_source, collect_target_pids_for_source};

pub const WM_APP_FOREGROUND_CHANGED: u32 = WM_APP + 4;
const MONITORINFOF_PRIMARY: u32 = 1;

static HOOK_TARGET_HWND: AtomicIsize = AtomicIsize::new(0);

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayMonitor {
    pub index: usize,
    pub label: String,
    pub short_label: String,
    pub monitor_rect: (i32, i32, i32, i32),
    pub work_top: i32,
    pub dpi_scale: f32,
    pub is_primary: bool,
}

pub struct ForegroundHookGuard {
    hook: HWINEVENTHOOK,
}

impl ForegroundHookGuard {
    pub fn install(hwnd: HWND) -> Self {
        HOOK_TARGET_HWND.store(hwnd.0 as isize, Ordering::Release);
        let hook = unsafe {
            SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(foreground_win_event_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            )
        };
        Self { hook }
    }
}

impl Drop for ForegroundHookGuard {
    fn drop(&mut self) {
        HOOK_TARGET_HWND.store(0, Ordering::Release);
        if !self.hook.is_invalid() {
            unsafe {
                let _ = UnhookWinEvent(self.hook);
            }
        }
    }
}

unsafe extern "system" fn foreground_win_event_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    let raw = HOOK_TARGET_HWND.load(Ordering::Acquire);
    if raw != 0 {
        let target = HWND(raw as *mut core::ffi::c_void);
        unsafe {
            let _ = PostMessageW(Some(target), WM_APP_FOREGROUND_CHANGED, WPARAM(0), LPARAM(0));
        }
    }
}

struct RawMonitorEntry {
    rect: (i32, i32, i32, i32),
    work_top: i32,
    dpi_scale: f32,
    is_primary: bool,
}

pub fn enumerate_monitors() -> Vec<DisplayMonitor> {
    let mut raw_list: Vec<RawMonitorEntry> = Vec::new();
    let ptr = &mut raw_list as *mut Vec<RawMonitorEntry>;
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(collect_monitor_proc), LPARAM(ptr as isize));
    }

    raw_list.sort_by(|a, b| {
        b.is_primary
            .cmp(&a.is_primary)
            .then_with(|| a.rect.0.cmp(&b.rect.0))
            .then_with(|| a.rect.1.cmp(&b.rect.1))
    });

    let mut monitors = Vec::with_capacity(raw_list.len());
    for (idx, entry) in raw_list.into_iter().enumerate() {
        let width = (entry.rect.2 - entry.rect.0).max(0);
        let height = (entry.rect.3 - entry.rect.1).max(0);
        let num = idx + 1;
        let label = if entry.is_primary {
            format!("Màn hình {num} ({width}x{height}) - Chính")
        } else {
            format!("Màn hình {num} ({width}x{height})")
        };
        let short_label = if entry.is_primary {
            format!("{num} - Chính ({width}x{height})")
        } else {
            format!("{num} ({width}x{height})")
        };
        monitors.push(DisplayMonitor {
            index: idx,
            label,
            short_label,
            monitor_rect: entry.rect,
            work_top: entry.work_top,
            dpi_scale: entry.dpi_scale,
            is_primary: entry.is_primary,
        });
    }

    if monitors.is_empty() {
        monitors.push(DisplayMonitor {
            index: 0,
            label: String::from("Màn hình 1 (Mặc định) - Chính"),
            short_label: String::from("1 - Chính"),
            monitor_rect: (0, 0, 1920, 1080),
            work_top: 0,
            dpi_scale: 1.0,
            is_primary: true,
        });
    }
    monitors
}

unsafe extern "system" fn collect_monitor_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let list_ptr = lparam.0 as *mut Vec<RawMonitorEntry>;
    if list_ptr.is_null() {
        return BOOL(0);
    }

    let mut info_ex = MONITORINFOEXW {
        monitorInfo: MONITORINFO {
            cbSize: size_of::<MONITORINFOEXW>() as u32,
            ..Default::default()
        },
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(hmonitor, &mut info_ex.monitorInfo) }.as_bool() {
        let rc = info_ex.monitorInfo.rcMonitor;
        let work_top = info_ex.monitorInfo.rcWork.top;
        let is_primary = (info_ex.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0;
        let mut dpi_x = 96u32;
        let mut dpi_y = 96u32;
        let dpi_scale =
            if unsafe { GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }
                .is_ok()
                && dpi_x > 0
            {
                (dpi_x as f32 / 96.0).clamp(0.5, 4.0)
            } else {
                1.0
            };

        unsafe {
            (*list_ptr).push(RawMonitorEntry {
                rect: (rc.left, rc.top, rc.right, rc.bottom),
                work_top,
                dpi_scale,
                is_primary,
            });
        }
    }
    BOOL(1)
}

pub fn select_monitor_or_default(monitors: &[DisplayMonitor], index: usize) -> DisplayMonitor {
    if let Some(found) = monitors.get(index % monitors.len().max(1)) {
        found.clone()
    } else {
        DisplayMonitor {
            index: 0,
            label: String::from("Màn hình 1 (Mặc định) - Chính"),
            short_label: String::from("1 - Chính"),
            monitor_rect: (0, 0, 1920, 1080),
            work_top: 0,
            dpi_scale: 1.0,
            is_primary: true,
        }
    }
}

pub fn is_foreground_fullscreen_on_monitor(
    island_hwnd: HWND,
    monitor_rect: (i32, i32, i32, i32),
) -> bool {
    if let Ok(quns) = unsafe { SHQueryUserNotificationState() }
        && (quns == QUNS_RUNNING_D3D_FULL_SCREEN || quns == QUNS_PRESENTATION_MODE)
    {
        return true;
    }

    let fg = unsafe { GetForegroundWindow() };
    is_hwnd_fullscreen_on_monitor(fg, island_hwnd, monitor_rect)
}

pub fn is_hwnd_fullscreen_on_monitor(
    target_hwnd: HWND,
    island_hwnd: HWND,
    monitor_rect: (i32, i32, i32, i32),
) -> bool {
    if target_hwnd.is_invalid() || target_hwnd == island_hwnd {
        return false;
    }
    if !unsafe { IsWindowVisible(target_hwnd) }.as_bool()
        || unsafe { IsIconic(target_hwnd) }.as_bool()
    {
        return false;
    }

    let mut cloaked = 0u32;
    if unsafe {
        DwmGetWindowAttribute(
            target_hwnd,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut u32).cast(),
            size_of::<u32>() as u32,
        )
    }
    .is_ok()
        && cloaked != 0
    {
        return false;
    }

    let mut class_buf = [0u16; 128];
    let len = unsafe { GetClassNameW(target_hwnd, &mut class_buf) };
    if len > 0 {
        let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);
        if is_ignored_shell_class(&class_name) {
            return false;
        }
    }

    let mut win_rc = RECT::default();
    if unsafe { GetWindowRect(target_hwnd, &mut win_rc) }.is_err() {
        return false;
    }

    is_rect_fullscreen(
        (win_rc.left, win_rc.top, win_rc.right, win_rc.bottom),
        monitor_rect,
    )
}

pub fn is_ignored_shell_class(class_name: &str) -> bool {
    matches!(
        class_name,
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
    )
}

pub fn is_rect_fullscreen(
    win_rect: (i32, i32, i32, i32),
    monitor_rect: (i32, i32, i32, i32),
) -> bool {
    let (wl, wt, wr, wb) = win_rect;
    let (ml, mt, mr, mb) = monitor_rect;
    if wr <= wl || wb <= wt || mr <= ml || mb <= mt {
        return false;
    }
    wl <= ml && wt <= mt && wr >= mr && wb >= mb
}

pub fn is_anti_process_or_title(exe_name: &str, window_title: &str) -> bool {
    let exe = exe_name.trim().to_ascii_lowercase();
    if exe.contains("agy") || exe.contains("antigravity") {
        return true;
    }
    let is_terminal = matches!(
        exe.as_str(),
        "windowsterminal.exe"
            | "cmd.exe"
            | "powershell.exe"
            | "pwsh.exe"
            | "conhost.exe"
            | "alacritty.exe"
            | "wezterm-gui.exe"
            | "mintty.exe"
            | "code.exe"
    );
    if is_terminal {
        let title = window_title.trim().to_ascii_lowercase();
        return title.contains("agy") || title.contains("antigravity");
    }
    false
}

pub fn window_matches_anti_source(source: AntiSource, exe_name: &str, window_title: &str) -> bool {
    let exe = exe_name.trim().to_ascii_lowercase();
    let title = window_title.trim().to_ascii_lowercase();
    match source {
        AntiSource::AntiIde => {
            (exe.contains("antigravity") && exe.contains("ide"))
                || (exe == "code.exe" && title.contains("antigravity"))
        }
        AntiSource::AgyCli => {
            exe.starts_with("agy")
                || (is_terminal_exe(&exe) && (title.contains("agy") || title.contains("antigravity")))
        }
        AntiSource::AntiApp => exe.contains("antigravity") && !exe.contains("ide"),
        AntiSource::Antigravity => is_anti_process_or_title(&exe, &title),
    }
}

fn is_terminal_exe(exe_lower: &str) -> bool {
    matches!(
        exe_lower,
        "windowsterminal.exe"
            | "cmd.exe"
            | "powershell.exe"
            | "pwsh.exe"
            | "conhost.exe"
            | "alacritty.exe"
            | "wezterm-gui.exe"
            | "mintty.exe"
            | "code.exe"
    )
}

fn read_hwnd_exe_name(hwnd: HWND) -> String {
    let mut pid = 0_u32;
    let _ = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return String::new();
    }
    let Ok(proc_handle) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) })
    else {
        return String::new();
    };
    let mut buf = [0_u16; 520];
    let mut len = buf.len() as u32;
    let ok = unsafe {
        QueryFullProcessImageNameW(
            proc_handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
    };
    unsafe {
        let _ = CloseHandle(proc_handle);
    }
    if ok.is_ok() && len > 0 {
        let full = String::from_utf16_lossy(&buf[..len as usize]);
        std::path::Path::new(&full)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    } else {
        String::new()
    }
}

fn read_hwnd_title(hwnd: HWND) -> String {
    let mut title_buf = [0_u16; 256];
    let len = unsafe { GetWindowTextW(hwnd, &mut title_buf) };
    if len > 0 {
        String::from_utf16_lossy(&title_buf[..len as usize])
    } else {
        String::new()
    }
}

struct AntiFocusSearch {
    island_hwnd: HWND,
    source: AntiSource,
    target_pids: Vec<u32>,
    exact_match: Option<HWND>,
    fallback_match: Option<HWND>,
}

unsafe extern "system" fn find_anti_window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let search = unsafe { &mut *(lparam.0 as *mut AntiFocusSearch) };
    if hwnd.is_invalid() || hwnd == search.island_hwnd || !unsafe { IsWindowVisible(hwnd) }.as_bool() {
        return BOOL(1);
    }
    let mut cloaked = 0_u32;
    if unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut u32).cast(),
            size_of::<u32>() as u32,
        )
    }
    .is_ok()
        && cloaked != 0
    {
        return BOOL(1);
    }
    if !unsafe { IsIconic(hwnd) }.as_bool() {
        let mut rc = RECT::default();
        if unsafe { GetWindowRect(hwnd, &mut rc) }.is_err()
            || (rc.right - rc.left) < 80
            || (rc.bottom - rc.top) < 80
        {
            return BOOL(1);
        }
    }
    let title = read_hwnd_title(hwnd);
    if title.trim().is_empty() {
        return BOOL(1);
    }
    let exe = read_hwnd_exe_name(hwnd);
    if matches!(
        (search.source, classify_exe_name_to_source(&exe)),
        (
            AntiSource::AgyCli,
            Some(AntiSource::AntiIde | AntiSource::AntiApp)
        )
    ) {
        return BOOL(1);
    }
    let mut pid = 0_u32;
    let _ = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid != 0 && search.target_pids.contains(&pid) {
        search.exact_match = Some(hwnd);
        return BOOL(0);
    }
    if search.fallback_match.is_none() && window_matches_anti_source(search.source, &exe, &title) {
        search.fallback_match = Some(hwnd);
    }
    BOOL(1)
}

pub fn focus_anti_window(island_hwnd: HWND, source: AntiSource) -> bool {
    let target_pids = collect_target_pids_for_source(source);
    let mut search = AntiFocusSearch {
        island_hwnd,
        source,
        target_pids,
        exact_match: None,
        fallback_match: None,
    };
    let ptr = &mut search as *mut AntiFocusSearch;
    unsafe {
        let _ = EnumWindows(Some(find_anti_window_proc), LPARAM(ptr as isize));
    }
    let Some(target) = search.exact_match.or(search.fallback_match) else {
        return false;
    };
    unsafe {
        let _ = AllowSetForegroundWindow(ASFW_ANY);
        if IsIconic(target).as_bool() {
            let _ = ShowWindow(target, SW_RESTORE);
        }
        if SetForegroundWindow(target).as_bool() && GetForegroundWindow() == target {
            return true;
        }
        let fg = GetForegroundWindow();
        let fg_tid = if fg.is_invalid() {
            0
        } else {
            GetWindowThreadProcessId(fg, None)
        };
        let cur_tid = GetCurrentThreadId();
        let attached = fg_tid != 0
            && fg_tid != cur_tid
            && AttachThreadInput(cur_tid, fg_tid, true).as_bool();
        let _ = BringWindowToTop(target);
        let ok = SetForegroundWindow(target).as_bool();
        if attached {
            let _ = AttachThreadInput(cur_tid, fg_tid, false);
        }
        if !ok || GetForegroundWindow() != target {
            SwitchToThisWindow(target, true);
        }
        GetForegroundWindow() == target || ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anti_foreground_matcher_recognizes_cli_ide_and_app() {
        assert!(is_anti_process_or_title("agy.exe", ""));
        assert!(is_anti_process_or_title("Antigravity.exe", "Workspace"));
        assert!(is_anti_process_or_title(
            "WindowsTerminal.exe",
            "Administrator: agy"
        ));
        assert!(!is_anti_process_or_title("chrome.exe", "Antigravity docs"));

        assert!(window_matches_anti_source(
            AntiSource::AntiIde,
            "Antigravity IDE.exe",
            "win-dynamic-island"
        ));
        assert!(!window_matches_anti_source(
            AntiSource::AntiIde,
            "Antigravity.exe",
            "Antigravity"
        ));
        assert!(window_matches_anti_source(
            AntiSource::AntiApp,
            "Antigravity.exe",
            "Antigravity"
        ));
        assert!(window_matches_anti_source(
            AntiSource::AgyCli,
            "WindowsTerminal.exe",
            "agy - session"
        ));
    }

    #[test]
    fn fullscreen_rect_detection_handles_maximized_and_multi_monitor() {
        let primary = (0, 0, 1920, 1080);
        // Cửa sổ Maximize bình thường chừa thanh Taskbar 48px bên dưới -> KHÔNG phải fullscreen.
        assert!(!is_rect_fullscreen((-8, -8, 1928, 1040), primary));
        // Cửa sổ Fullscreen borderless hoặc video fullscreen -> LÀ fullscreen.
        assert!(is_rect_fullscreen((0, 0, 1920, 1080), primary));
        assert!(is_rect_fullscreen((-1, -1, 1921, 1081), primary));
        // Cửa sổ Fullscreen trên màn hình phụ bên trái -> KHÔNG ẩn màn hình chính.
        assert!(!is_rect_fullscreen((-1920, 0, 0, 1080), primary));
        // Cửa sổ có kích thước 0 -> KHÔNG phải fullscreen.
        assert!(!is_rect_fullscreen((0, 0, 0, 0), primary));
    }

    #[test]
    fn shell_desktop_and_taskbar_classes_are_ignored() {
        assert!(is_ignored_shell_class("Progman"));
        assert!(is_ignored_shell_class("WorkerW"));
        assert!(is_ignored_shell_class("Shell_TrayWnd"));
        assert!(is_ignored_shell_class("Shell_SecondaryTrayWnd"));
        assert!(!is_ignored_shell_class("Chrome_WidgetWin_1"));
    }

    #[test]
    fn enumerate_monitors_returns_valid_primary_monitor() {
        let monitors = enumerate_monitors();
        assert!(!monitors.is_empty());
        assert!(monitors[0].is_primary);
        assert!(monitors[0].dpi_scale >= 0.5);
    }

    #[test]
    fn win32_hwnd_fullscreen_integration_check() {
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, SW_HIDE, SW_MINIMIZE, SW_SHOWNOACTIVATE,
            SetWindowPos, ShowWindow, SWP_NOACTIVATE, SWP_NOZORDER, WS_EX_TOOLWINDOW, WS_POPUP,
        };
        use windows::core::PCWSTR;

        let class_name: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let monitors = enumerate_monitors();
        let primary = select_monitor_or_default(&monitors, 0);
        let (ml, mt, mr, mb) = primary.monitor_rect;
        let mw = mr - ml;
        let mh = mb - mt;

        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                PCWSTR(class_name.as_ptr()),
                PCWSTR::null(),
                WS_POPUP,
                ml,
                mt,
                mw,
                mh,
                None,
                None,
                None,
                None,
            )
        }
        .expect("CreateWindowExW must succeed");

        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        assert!(is_hwnd_fullscreen_on_monitor(
            hwnd,
            HWND::default(),
            primary.monitor_rect
        ));
        assert!(!is_hwnd_fullscreen_on_monitor(
            hwnd,
            hwnd,
            primary.monitor_rect
        ));

        unsafe {
            let _ = SetWindowPos(
                hwnd,
                None,
                ml,
                mt,
                mw,
                (mh - 48).max(100),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        assert!(!is_hwnd_fullscreen_on_monitor(
            hwnd,
            HWND::default(),
            primary.monitor_rect
        ));

        unsafe {
            let _ = SetWindowPos(hwnd, None, ml, mt, mw, mh, SWP_NOZORDER | SWP_NOACTIVATE);
            let _ = ShowWindow(hwnd, SW_MINIMIZE);
        }
        assert!(!is_hwnd_fullscreen_on_monitor(
            hwnd,
            HWND::default(),
            primary.monitor_rect
        ));

        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
            let _ = DestroyWindow(hwnd);
        }
    }
}
