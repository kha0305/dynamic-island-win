#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[path = "../install.rs"]
mod install;

use std::mem::size_of;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, CreateSolidBrush, DEFAULT_CHARSET,
    DEFAULT_PITCH, DeleteObject, FF_DONTCARE, FW_NORMAL, FW_SEMIBOLD, HBRUSH, HDC, HFONT,
    HGDIOBJ, OUT_DEFAULT_PRECIS, SetBkColor, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    ICC_PROGRESS_CLASS, ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx,
    PBM_SETPOS, PBM_SETRANGE32, PROGRESS_CLASSW,
};
use windows::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForSystem, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::Shell::{
    FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FileOpenDialog, IFileOpenDialog,
    SHCreateItemFromParsingName, SIGDN_FILESYSPATH,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, BS_PUSHBUTTON, CS_HREDRAW,
    CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA, GetMessageW,
    GetSystemMetrics, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, HICON, IDC_ARROW,
    IMAGE_ICON, LR_DEFAULTCOLOR, LR_SHARED, LoadCursorW, LoadImageW, MB_ICONERROR,
    MB_ICONINFORMATION, MB_OK, MSG, PostMessageW, PostQuitMessage, RegisterClassExW, SM_CXICON,
    SM_CXSCREEN, SM_CXSMICON, SM_CYICON, SM_CYSCREEN, SM_CYSMICON, STM_SETICON, SW_SHOWNORMAL,
    SetWindowLongPtrW, SetWindowTextW, ShowWindow, TranslateMessage, WM_APP, WM_CLOSE,
    WM_COMMAND, WM_CTLCOLORBTN, WM_CTLCOLORSTATIC, WM_DESTROY, WM_SETFONT, WNDCLASSEXW,
    WS_BORDER, WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU,
    WS_TABSTOP, WS_VISIBLE,
};
use windows::core::PCWSTR;

use install::{
    APP_VERSION, ComApartment, EXE_FILE_NAME, QUIET_FLAG, SILENT_FLAG, desktop_shortcut_path,
    install_dir, install_to_dir, launch_installed_app, report_setup_error, show_message_box,
    start_menu_shortcut_path, stop_running_instance_and_wait, to_wide_null,
};

const SETUP_CLASS_NAME: &str = "WinDynamicIslandSetupClass";
const IDI_APP_ICON: u16 = 1;

const IDC_EDIT_DIR: isize = 100;
const IDC_CHK_AUTOSTART: isize = 101;
const IDC_CHK_DESKTOP: isize = 102;
const IDC_CHK_LAUNCH: isize = 103;
const IDC_BTN_INSTALL: isize = 104;
const IDC_BTN_CANCEL: isize = 105;
const IDC_BTN_BROWSE: isize = 106;

const WM_APP_INSTALL_STATUS: u32 = WM_APP + 10;
const WM_APP_INSTALL_DONE: u32 = WM_APP + 11;

const BST_CHECKED_VAL: usize = 1;
const SS_LEFT_STYLE: u32 = 0x0000_0000;
const SS_ICON_STYLE: u32 = 0x0000_0003;
const ES_AUTOHSCROLL_STYLE: u32 = 0x0000_0080;

#[cfg(has_setup_payload)]
const EMBEDDED_APP_BYTES: Option<&[u8]> = Some(include_bytes!(env!("WDI_SETUP_PAYLOAD_ABS")));

#[cfg(not(has_setup_payload))]
const EMBEDDED_APP_BYTES: Option<&[u8]> = None;

fn resolve_payload_bytes() -> Result<Vec<u8>, String> {
    if let Some(bytes) = EMBEDDED_APP_BYTES {
        return Ok(bytes.to_vec());
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .ok_or_else(|| String::from("Không xác định được thư mục trình cài đặt."))?;
    let sibling = exe_dir.join(EXE_FILE_NAME);
    if sibling.is_file() {
        return std::fs::read(&sibling)
            .map_err(|e| format!("Không thể đọc tệp {EXE_FILE_NAME}: {e}"));
    }
    Err(String::from(
        "Gói cài đặt chưa chứa tệp win-dynamic-island.exe. Hãy tải lại bản Setup chính thức.",
    ))
}

struct SetupState {
    hwnd: HWND,
    bg_brush: HBRUSH,
    font_title: HFONT,
    font_body: HFONT,
    font_small: HFONT,
    edit_dir: HWND,
    btn_browse: HWND,
    chk_autostart: HWND,
    chk_desktop: HWND,
    chk_launch: HWND,
    lbl_status: HWND,
    progress: HWND,
    btn_install: HWND,
    btn_cancel: HWND,
    installing: bool,
    finished: bool,
    result_rx: Option<Receiver<Result<PathBuf, String>>>,
    status_rx: Option<Receiver<(&'static str, i32)>>,
}

impl Drop for SetupState {
    fn drop(&mut self) {
        unsafe {
            for h in [
                HGDIOBJ(self.bg_brush.0),
                HGDIOBJ(self.font_title.0),
                HGDIOBJ(self.font_body.0),
                HGDIOBJ(self.font_small.0),
            ] {
                if !h.is_invalid() {
                    let _ = DeleteObject(h);
                }
            }
        }
    }
}

fn scale_px(base: i32, dpi: u32) -> i32 {
    (((base as i64 * dpi as i64) + 48) / 96) as i32
}

fn make_font(dpi: u32, pt: i32, bold: bool) -> HFONT {
    let height = -((pt * dpi as i32) / 72);
    let face = to_wide_null("Segoe UI");
    unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            if bold { FW_SEMIBOLD.0 as i32 } else { FW_NORMAL.0 as i32 },
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            DEFAULT_PITCH.0 as u32 | FF_DONTCARE.0 as u32,
            PCWSTR(face.as_ptr()),
        )
    }
}

fn load_icon(hinstance: HINSTANCE, large: bool) -> Option<HICON> {
    let (cx_m, cy_m) = if large {
        (SM_CXICON, SM_CYICON)
    } else {
        (SM_CXSMICON, SM_CYSMICON)
    };
    let cx = unsafe { GetSystemMetrics(cx_m) }.max(16);
    let cy = unsafe { GetSystemMetrics(cy_m) }.max(16);
    let h = unsafe {
        LoadImageW(
            Some(hinstance),
            PCWSTR(IDI_APP_ICON as usize as *const u16),
            IMAGE_ICON,
            cx,
            cy,
            LR_DEFAULTCOLOR | LR_SHARED,
        )
    }
    .ok()?;
    let icon = HICON(h.0);
    if icon.is_invalid() { None } else { Some(icon) }
}

fn set_control_font(hwnd: HWND, font: HFONT) {
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::SendMessageW(
            hwnd,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(1)),
        );
    }
}

struct ChildSpec<'a> {
    parent: HWND,
    hinstance: HINSTANCE,
    class: &'a str,
    text: &'a str,
    ex_style: u32,
    style: u32,
    bounds: (i32, i32, i32, i32),
    id: isize,
    font: HFONT,
}

fn create_child(spec: ChildSpec<'_>) -> windows::core::Result<HWND> {
    let cls_w = to_wide_null(spec.class);
    let txt_w = to_wide_null(spec.text);
    let (x, y, w, h) = spec.bounds;
    let hwnd = unsafe {
        CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(spec.ex_style),
            PCWSTR(cls_w.as_ptr()),
            PCWSTR(txt_w.as_ptr()),
            windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(spec.style),
            x,
            y,
            w,
            h,
            Some(spec.parent),
            Some(windows::Win32::UI::WindowsAndMessaging::HMENU(
                spec.id as *mut _,
            )),
            Some(spec.hinstance),
            None,
        )
    }?;
    set_control_font(hwnd, spec.font);
    Ok(hwnd)
}

fn get_window_text_string(hwnd: HWND) -> String {
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; len as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buf) };
    if copied <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..copied as usize])
}

fn normalize_picked_install_dir(picked: PathBuf) -> PathBuf {
    let already_named = picked
        .file_name()
        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("WinDynamicIsland"));
    if already_named {
        return picked;
    }
    let is_empty_dir = std::fs::read_dir(&picked)
        .ok()
        .is_some_and(|mut it| it.next().is_none());
    if is_empty_dir {
        picked
    } else {
        picked.join("WinDynamicIsland")
    }
}

fn browse_for_install_folder(owner: HWND, current: &str) -> Option<PathBuf> {
    let dialog: IFileOpenDialog =
        unsafe { CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER) }.ok()?;
    unsafe {
        let mut opts = dialog.GetOptions().ok()?;
        opts |= FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST;
        let _ = dialog.SetOptions(opts);
        let title_w = to_wide_null("Chọn thư mục cài đặt Win Dynamic Island");
        let _ = dialog.SetTitle(PCWSTR(title_w.as_ptr()));
        let trimmed = current.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed);
            let seed = if p.is_dir() {
                Some(p)
            } else {
                p.parent().filter(|par| par.is_dir()).map(PathBuf::from)
            };
            if let Some(seed_dir) = seed {
                let seed_w = to_wide_null(&seed_dir.to_string_lossy());
                if let Ok(item) = SHCreateItemFromParsingName::<_, _, windows::Win32::UI::Shell::IShellItem>(
                    PCWSTR(seed_w.as_ptr()),
                    None,
                ) {
                    let _ = dialog.SetFolder(&item);
                }
            }
        }
        if dialog.Show(Some(owner)).is_err() {
            return None;
        }
        let result = dialog.GetResult().ok()?;
        let pwstr = result.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let chosen = pwstr.to_string().ok();
        CoTaskMemFree(Some(pwstr.0.cast()));
        chosen.map(|s| normalize_picked_install_dir(PathBuf::from(s)))
    }
}

fn is_checked(hwnd: HWND) -> bool {
    let res = unsafe {
        windows::Win32::UI::WindowsAndMessaging::SendMessageW(hwnd, BM_GETCHECK, None, None)
    };
    res.0 as usize == BST_CHECKED_VAL
}

fn set_checked(hwnd: HWND, checked: bool) {
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::SendMessageW(
            hwnd,
            BM_SETCHECK,
            Some(WPARAM(if checked { BST_CHECKED_VAL } else { 0 })),
            None,
        );
    }
}

fn get_state_mut<'a>(hwnd: HWND) -> Option<&'a mut SetupState> {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut SetupState;
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { &mut *ptr })
    }
}

impl SetupState {
    fn browse_install_dir(&self) {
        if self.installing || self.finished {
            return;
        }
        let cur = get_window_text_string(self.edit_dir);
        if let Some(chosen) = browse_for_install_folder(self.hwnd, &cur) {
            let txt_w = to_wide_null(&chosen.to_string_lossy());
            unsafe {
                let _ = SetWindowTextW(self.edit_dir, PCWSTR(txt_w.as_ptr()));
            }
        }
    }

    fn start_install(&mut self) {
        if self.installing {
            return;
        }
        if self.finished {
            unsafe {
                let _ = DestroyWindow(self.hwnd);
            }
            return;
        }

        let dir_raw = get_window_text_string(self.edit_dir);
        let dir_trimmed = dir_raw.trim();
        if dir_trimmed.is_empty() {
            let _ = show_message_box(
                "Thiếu thư mục cài đặt",
                "Vui lòng nhập hoặc chọn thư mục cài đặt hợp lệ.",
                MB_OK.0 | MB_ICONERROR.0,
            );
            return;
        }
        let target_dir = PathBuf::from(dir_trimmed);

        let autostart = is_checked(self.chk_autostart);
        let create_desktop = is_checked(self.chk_desktop);
        let launch_after = is_checked(self.chk_launch);

        self.installing = true;
        unsafe {
            let _ = EnableWindow(self.edit_dir, false);
            let _ = EnableWindow(self.btn_browse, false);
            let _ = EnableWindow(self.chk_autostart, false);
            let _ = EnableWindow(self.chk_desktop, false);
            let _ = EnableWindow(self.chk_launch, false);
            let _ = EnableWindow(self.btn_install, false);
            let _ = EnableWindow(self.btn_cancel, false);
        }

        let (st_tx, st_rx) = mpsc::channel();
        let (res_tx, res_rx) = mpsc::channel();
        self.status_rx = Some(st_rx);
        self.result_rx = Some(res_rx);

        let hwnd_raw = self.hwnd.0 as isize;
        thread::spawn(move || {
            let push_status = |msg: &'static str, pct: i32| {
                let _ = st_tx.send((msg, pct));
                let target = HWND(hwnd_raw as *mut _);
                unsafe {
                    let _ = PostMessageW(Some(target), WM_APP_INSTALL_STATUS, WPARAM(0), LPARAM(0));
                }
            };

            push_status("Đang kiểm tra gói cài đặt...", 15);
            let res = (|| -> Result<PathBuf, String> {
                let payload = resolve_payload_bytes()?;
                push_status("Đang đóng phiên bản đang chạy (nếu có)...", 35);
                stop_running_instance_and_wait();

                let sm = start_menu_shortcut_path();
                let dk = if create_desktop {
                    desktop_shortcut_path()
                } else {
                    None
                };
                push_status("Đang cài đặt tệp và tạo lối tắt...", 75);
                let installed = install_to_dir(
                    &target_dir,
                    &payload,
                    autostart,
                    dk.as_deref(),
                    sm.as_deref(),
                    true,
                )?;
                if launch_after {
                    push_status("Đang khởi chạy Win Dynamic Island...", 95);
                    launch_installed_app(&installed)?;
                }
                Ok(installed)
            })();

            let _ = res_tx.send(res);
            let target = HWND(hwnd_raw as *mut _);
            unsafe {
                let _ = PostMessageW(Some(target), WM_APP_INSTALL_DONE, WPARAM(0), LPARAM(0));
            }
        });
    }

    fn drain_status(&mut self) {
        let Some(rx) = &self.status_rx else {
            return;
        };
        while let Ok((msg, pct)) = rx.try_recv() {
            let msg_w = to_wide_null(msg);
            unsafe {
                let _ = SetWindowTextW(self.lbl_status, PCWSTR(msg_w.as_ptr()));
                let _ = windows::Win32::UI::WindowsAndMessaging::SendMessageW(
                    self.progress,
                    PBM_SETPOS,
                    Some(WPARAM(pct as usize)),
                    None,
                );
            }
        }
    }

    fn handle_install_done(&mut self) {
        self.drain_status();
        self.installing = false;
        let Some(rx) = self.result_rx.take() else {
            return;
        };
        let Ok(res) = rx.try_recv() else {
            return;
        };

        match res {
            Ok(_installed) => {
                self.finished = true;
                let done_w = to_wide_null("Cài đặt Win Dynamic Island hoàn tất.");
                let btn_w = to_wide_null("Hoàn tất");
                unsafe {
                    let _ = SetWindowTextW(self.lbl_status, PCWSTR(done_w.as_ptr()));
                    let _ = windows::Win32::UI::WindowsAndMessaging::SendMessageW(
                        self.progress,
                        PBM_SETPOS,
                        Some(WPARAM(100)),
                        None,
                    );
                    let _ = SetWindowTextW(self.btn_install, PCWSTR(btn_w.as_ptr()));
                    let _ = EnableWindow(self.btn_install, true);
                }
                if is_checked(self.chk_launch) {
                    unsafe {
                        let _ = DestroyWindow(self.hwnd);
                    }
                }
            }
            Err(err) => {
                let err_w = to_wide_null("Cài đặt chưa hoàn tất. Vui lòng thử lại.");
                unsafe {
                    let _ = SetWindowTextW(self.lbl_status, PCWSTR(err_w.as_ptr()));
                    let _ = windows::Win32::UI::WindowsAndMessaging::SendMessageW(
                        self.progress,
                        PBM_SETPOS,
                        Some(WPARAM(0)),
                        None,
                    );
                    let _ = EnableWindow(self.edit_dir, true);
                    let _ = EnableWindow(self.btn_browse, true);
                    let _ = EnableWindow(self.chk_autostart, true);
                    let _ = EnableWindow(self.chk_desktop, true);
                    let _ = EnableWindow(self.chk_launch, true);
                    let _ = EnableWindow(self.btn_install, true);
                    let _ = EnableWindow(self.btn_cancel, true);
                }
                let _ = show_message_box(
                    "Lỗi cài đặt Win Dynamic Island",
                    &err,
                    MB_OK.0 | MB_ICONERROR.0,
                );
            }
        }
    }
}

unsafe extern "system" fn setup_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
            if let Some(st) = get_state_mut(hwnd) {
                let hdc = HDC(wparam.0 as *mut _);
                unsafe {
                    SetBkMode(hdc, TRANSPARENT);
                    SetBkColor(hdc, COLORREF(0x00FF_FFFF));
                    SetTextColor(hdc, COLORREF(0x001E_1E1E));
                }
                return LRESULT(st.bg_brush.0 as isize);
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_APP_INSTALL_STATUS => {
            if let Some(st) = get_state_mut(hwnd) {
                st.drain_status();
            }
            LRESULT(0)
        }
        WM_APP_INSTALL_DONE => {
            if let Some(st) = get_state_mut(hwnd) {
                st.handle_install_done();
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as isize;
            if let Some(st) = get_state_mut(hwnd) {
                if id == IDC_BTN_BROWSE {
                    st.browse_install_dir();
                    return LRESULT(0);
                }
                if id == IDC_BTN_INSTALL {
                    st.start_install();
                    return LRESULT(0);
                }
                if id == IDC_BTN_CANCEL && !st.installing {
                    unsafe {
                        let _ = DestroyWindow(hwnd);
                    }
                    return LRESULT(0);
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            if let Some(st) = get_state_mut(hwnd)
                && st.installing
            {
                return LRESULT(0);
            }
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            let ptr = unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) } as *mut SetupState;
            if !ptr.is_null() {
                unsafe {
                    drop(Box::from_raw(ptr));
                }
            }
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn run_silent_install(quiet: bool) -> i32 {
    let res = (|| -> Result<(), String> {
        let payload = resolve_payload_bytes()?;
        stop_running_instance_and_wait();
        let dir = install_dir()
            .ok_or_else(|| String::from("Không xác định được thư mục %LOCALAPPDATA%."))?;
        let sm = start_menu_shortcut_path();
        let dk = desktop_shortcut_path();
        let exe = install_to_dir(&dir, &payload, true, dk.as_deref(), sm.as_deref(), true)?;
        launch_installed_app(&exe)?;
        Ok(())
    })();

    match res {
        Ok(()) => {
            if !quiet {
                let _ = show_message_box(
                    "Cài đặt hoàn tất",
                    "Đã cài đặt và khởi chạy Win Dynamic Island thành công.",
                    MB_OK.0 | MB_ICONINFORMATION.0,
                );
            }
            0
        }
        Err(err) => {
            report_setup_error("Lỗi cài đặt Win Dynamic Island", &err, quiet);
            1
        }
    }
}

fn run_gui_setup() -> i32 {
    let _com = ComApartment::init_sta();
    unsafe {
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_err() {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE);
        }
        let icc = INITCOMMONCONTROLSEX {
            dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_STANDARD_CLASSES | ICC_PROGRESS_CLASS,
        };
        let _ = InitCommonControlsEx(&icc);
    }

    let Ok(hmod) = (unsafe { GetModuleHandleW(None) }) else {
        return 1;
    };
    let hinstance: HINSTANCE = hmod.into();
    let dpi = unsafe { GetDpiForSystem() }.max(96);
    let s = |v: i32| scale_px(v, dpi);

    let icon_large = load_icon(hinstance, true).unwrap_or_default();
    let icon_small = load_icon(hinstance, false).unwrap_or_default();
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default();
    let bg_brush = unsafe { CreateSolidBrush(COLORREF(0x00FF_FFFF)) };

    let cls_w = to_wide_null(SETUP_CLASS_NAME);
    let title_w = to_wide_null(&format!("Cài đặt Win Dynamic Island v{APP_VERSION}"));
    let wc = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(setup_wnd_proc),
        hInstance: hinstance,
        hIcon: icon_large,
        hIconSm: icon_small,
        hCursor: cursor,
        hbrBackground: bg_brush,
        lpszClassName: PCWSTR(cls_w.as_ptr()),
        ..Default::default()
    };
    if unsafe { RegisterClassExW(&wc) } == 0 {
        return 1;
    }

    let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
    let mut client_rect = RECT {
        left: 0,
        top: 0,
        right: s(520),
        bottom: s(358),
    };
    unsafe {
        let _ = AdjustWindowRectExForDpi(&mut client_rect, style, false, Default::default(), dpi);
    }
    let win_w = client_rect.right - client_rect.left;
    let win_h = client_rect.bottom - client_rect.top;
    let scr_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let scr_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    let win_x = ((scr_w - win_w) / 2).max(0);
    let win_y = ((scr_h - win_h) / 2).max(0);

    let Ok(hwnd) = (unsafe {
        CreateWindowExW(
            Default::default(),
            PCWSTR(cls_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            style,
            win_x,
            win_y,
            win_w,
            win_h,
            None,
            None,
            Some(hinstance),
            None,
        )
    }) else {
        return 1;
    };

    let font_title = make_font(dpi, 14, true);
    let font_body = make_font(dpi, 10, false);
    let font_small = make_font(dpi, 9, false);

    let icon_ctrl = create_child(ChildSpec {
        parent: hwnd,
        hinstance,
        class: "STATIC",
        text: "",
        ex_style: 0,
        style: (WS_CHILD | WS_VISIBLE).0 | SS_ICON_STYLE,
        bounds: (s(24), s(20), s(48), s(48)),
        id: 0,
        font: font_body,
    });
    if let Ok(ic) = icon_ctrl
        && !icon_large.is_invalid()
    {
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::SendMessageW(
                ic,
                STM_SETICON,
                Some(WPARAM(icon_large.0 as usize)),
                None,
            );
        }
    }

    let _ = create_child(ChildSpec {
        parent: hwnd,
        hinstance,
        class: "STATIC",
        text: &format!("Win Dynamic Island v{APP_VERSION}"),
        ex_style: 0,
        style: (WS_CHILD | WS_VISIBLE).0 | SS_LEFT_STYLE,
        bounds: (s(86), s(18), s(410), s(28)),
        id: 0,
        font: font_title,
    });
    let _ = create_child(ChildSpec {
        parent: hwnd,
        hinstance,
        class: "STATIC",
        text: "Thanh viên thuốc thông minh cho Windows 11 (nhạc, khay tệp, HUD, Anti)",
        ex_style: 0,
        style: (WS_CHILD | WS_VISIBLE).0 | SS_LEFT_STYLE,
        bounds: (s(86), s(48), s(410), s(36)),
        id: 0,
        font: font_small,
    });

    let _ = create_child(ChildSpec {
        parent: hwnd,
        hinstance,
        class: "STATIC",
        text: "Thư mục cài đặt:",
        ex_style: 0,
        style: (WS_CHILD | WS_VISIBLE).0 | SS_LEFT_STYLE,
        bounds: (s(24), s(92), s(472), s(20)),
        id: 0,
        font: font_small,
    });

    let dir_display = install_dir().map_or_else(
        || String::from("%LOCALAPPDATA%\\Programs\\WinDynamicIsland"),
        |p| p.to_string_lossy().into_owned(),
    );
    let edit_dir = create_child(ChildSpec {
        parent: hwnd,
        hinstance,
        class: "EDIT",
        text: &dir_display,
        ex_style: WS_EX_CLIENTEDGE.0,
        style: (WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER).0 | ES_AUTOHSCROLL_STYLE,
        bounds: (s(24), s(114), s(382), s(26)),
        id: IDC_EDIT_DIR,
        font: font_small,
    })
    .expect("Tao o nhap thu muc cai dat");
    let btn_browse = create_child(ChildSpec {
        parent: hwnd,
        hinstance,
        class: "BUTTON",
        text: "Chọn...",
        ex_style: 0,
        style: (WS_CHILD | WS_VISIBLE | WS_TABSTOP).0 | BS_PUSHBUTTON as u32,
        bounds: (s(414), s(113), s(82), s(28)),
        id: IDC_BTN_BROWSE,
        font: font_small,
    })
    .expect("Tao nut chon thu muc");

    let chk_autostart = create_child(ChildSpec {
        parent: hwnd,
        hinstance,
        class: "BUTTON",
        text: "Tự động khởi động cùng Windows (khuyên dùng)",
        ex_style: 0,
        style: (WS_CHILD | WS_VISIBLE | WS_TABSTOP).0 | BS_AUTOCHECKBOX as u32,
        bounds: (s(24), s(152), s(472), s(24)),
        id: IDC_CHK_AUTOSTART,
        font: font_body,
    })
    .expect("Tao checkbox autostart");
    let chk_desktop = create_child(ChildSpec {
        parent: hwnd,
        hinstance,
        class: "BUTTON",
        text: "Tạo lối tắt trên màn hình nền (Desktop)",
        ex_style: 0,
        style: (WS_CHILD | WS_VISIBLE | WS_TABSTOP).0 | BS_AUTOCHECKBOX as u32,
        bounds: (s(24), s(182), s(472), s(24)),
        id: IDC_CHK_DESKTOP,
        font: font_body,
    })
    .expect("Tao checkbox desktop");
    let chk_launch = create_child(ChildSpec {
        parent: hwnd,
        hinstance,
        class: "BUTTON",
        text: "Mở Win Dynamic Island ngay sau khi cài đặt xong",
        ex_style: 0,
        style: (WS_CHILD | WS_VISIBLE | WS_TABSTOP).0 | BS_AUTOCHECKBOX as u32,
        bounds: (s(24), s(212), s(472), s(24)),
        id: IDC_CHK_LAUNCH,
        font: font_body,
    })
    .expect("Tao checkbox launch");

    set_checked(chk_autostart, true);
    set_checked(chk_desktop, true);
    set_checked(chk_launch, true);

    let lbl_status = create_child(ChildSpec {
        parent: hwnd,
        hinstance,
        class: "STATIC",
        text: "Sẵn sàng cài đặt ở quyền chuẩn Windows (giữ nguyên kéo thả Khay tệp).",
        ex_style: 0,
        style: (WS_CHILD | WS_VISIBLE).0 | SS_LEFT_STYLE,
        bounds: (s(24), s(250), s(472), s(22)),
        id: 0,
        font: font_small,
    })
    .expect("Tao nhan trang thai");

    let prog_cls = to_wide_null(&String::from_utf16_lossy(unsafe { PROGRESS_CLASSW.as_wide() }));
    let progress = unsafe {
        CreateWindowExW(
            Default::default(),
            PCWSTR(prog_cls.as_ptr()),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE,
            s(24),
            s(276),
            s(472),
            s(12),
            Some(hwnd),
            None,
            Some(hinstance),
            None,
        )
    }
    .expect("Tao thanh tien do");
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::SendMessageW(
            progress,
            PBM_SETRANGE32,
            Some(WPARAM(0)),
            Some(LPARAM(100)),
        );
    }

    let btn_install = create_child(ChildSpec {
        parent: hwnd,
        hinstance,
        class: "BUTTON",
        text: "Cài đặt ngay",
        ex_style: 0,
        style: (WS_CHILD | WS_VISIBLE | WS_TABSTOP).0 | BS_DEFPUSHBUTTON as u32,
        bounds: (s(274), s(306), s(124), s(32)),
        id: IDC_BTN_INSTALL,
        font: font_body,
    })
    .expect("Tao nut cai dat");
    let btn_cancel = create_child(ChildSpec {
        parent: hwnd,
        hinstance,
        class: "BUTTON",
        text: "Đóng",
        ex_style: 0,
        style: (WS_CHILD | WS_VISIBLE | WS_TABSTOP).0 | BS_PUSHBUTTON as u32,
        bounds: (s(406), s(306), s(90), s(32)),
        id: IDC_BTN_CANCEL,
        font: font_body,
    })
    .expect("Tao nut dong");

    let state = Box::new(SetupState {
        hwnd,
        bg_brush,
        font_title,
        font_body,
        font_small,
        edit_dir,
        btn_browse,
        chk_autostart,
        chk_desktop,
        chk_launch,
        lbl_status,
        progress,
        btn_install,
        btn_cancel,
        installing: false,
        finished: false,
        result_rx: None,
        status_rx: None,
    });
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
        let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
    }

    let mut msg = MSG::default();
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
        unsafe {
            let _ = TranslateMessage(&msg);
            windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
        }
    }
    let _ = RECT::default();
    0
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let silent = args
        .iter()
        .any(|a| a.eq_ignore_ascii_case("/S") || a == SILENT_FLAG || a == QUIET_FLAG);
    if silent {
        let quiet = args
            .iter()
            .any(|a| a == QUIET_FLAG || a.eq_ignore_ascii_case("/S"));
        let code = run_silent_install(quiet);
        std::process::exit(code);
    }
    let code = run_gui_setup();
    std::process::exit(code);
}
