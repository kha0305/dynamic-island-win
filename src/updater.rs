use std::ffi::c_void;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Networking::WinHttp::{
    WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_FLAG_SECURE,
    WINHTTP_OPTION_REDIRECT_POLICY, WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP,
    WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE, WinHttpCloseHandle, WinHttpConnect,
    WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable, WinHttpQueryHeaders,
    WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetOption,
    WinHttpSetTimeouts,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::PCWSTR;

use crate::json_mini::{
    extract_json_array_section, extract_json_string_field_top_level, iter_json_objects_in_array,
    parse_sha256_file, sha256_hex,
};
use crate::tray::to_wide_null;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GITHUB_LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/kha0305/dynamic-island-win/releases/latest";
pub const WM_APP_UPDATE_EVENT: u32 = WM_APP + 6;
pub const POST_UPDATE_ARG: &str = "--post-update";

const MAX_METADATA_BYTES: usize = 1024 * 1024;
const MAX_BINARY_BYTES: usize = 32 * 1024 * 1024;
const MIN_VALID_EXE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateReleaseInfo {
    pub version_tag: String,
    pub exe_url: String,
    pub sha256_url: Option<String>,
}

#[derive(Debug, Clone)]
pub enum UpdaterEvent {
    CheckUpToDate { silent: bool },
    CheckAvailable { release: UpdateReleaseInfo, silent: bool },
    CheckError { message: String, silent: bool },
    DownloadReadyToRestart,
    DownloadError { message: String },
}

struct WinHttpHandle(*mut c_void);

impl Drop for WinHttpHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
}

pub fn spawn_cleanup_old_binary() {
    let Ok(exe_path) = std::env::current_exe() else {
        return;
    };
    let old_path = backup_path_for(&exe_path);
    let new_path = staging_path_for(&exe_path);
    if !old_path.exists() && !new_path.exists() {
        return;
    }

    let _ = thread::Builder::new()
        .name(String::from("updater-cleanup"))
        .spawn(move || {
            let _ = fs::remove_file(&new_path);
            for _ in 0..20 {
                if !old_path.exists() || fs::remove_file(&old_path).is_ok() {
                    break;
                }
                thread::sleep(Duration::from_millis(150));
            }
        });
}

pub fn check_for_updates_async(hwnd: HWND, silent: bool) -> Receiver<UpdaterEvent> {
    let hwnd_raw = hwnd.0 as isize;
    let (tx, rx) = mpsc::channel();
    let _ = thread::Builder::new()
        .name(String::from("updater-check"))
        .spawn(move || {
            let event = match fetch_latest_release() {
                Ok(Some(release)) => UpdaterEvent::CheckAvailable { release, silent },
                Ok(None) => UpdaterEvent::CheckUpToDate { silent },
                Err(message) => UpdaterEvent::CheckError { message, silent },
            };
            let _ = tx.send(event);
            let target = HWND(hwnd_raw as *mut c_void);
            unsafe {
                let _ = PostMessageW(Some(target), WM_APP_UPDATE_EVENT, WPARAM(0), LPARAM(0));
            }
        });
    rx
}

pub fn download_and_stage_update_async(
    hwnd: HWND,
    release: UpdateReleaseInfo,
) -> Receiver<UpdaterEvent> {
    let hwnd_raw = hwnd.0 as isize;
    let (tx, rx) = mpsc::channel();
    let _ = thread::Builder::new()
        .name(String::from("updater-download"))
        .spawn(move || {
            let event = match download_and_stage_sync(&release) {
                Ok(()) => UpdaterEvent::DownloadReadyToRestart,
                Err(message) => UpdaterEvent::DownloadError { message },
            };
            let _ = tx.send(event);
            let target = HWND(hwnd_raw as *mut c_void);
            unsafe {
                let _ = PostMessageW(Some(target), WM_APP_UPDATE_EVENT, WPARAM(0), LPARAM(0));
            }
        });
    rx
}

pub fn spawn_updated_process() -> Result<(), String> {
    let exe_path = std::env::current_exe().map_err(|e| format!("Không xác định được tệp chạy: {e}"))?;
    Command::new(exe_path)
        .arg(POST_UPDATE_ARG)
        .spawn()
        .map_err(|e| format!("Không thể khởi chạy phiên bản mới: {e}"))?;
    Ok(())
}

fn fetch_latest_release() -> Result<Option<UpdateReleaseInfo>, String> {
    let body_bytes = winhttp_get_bytes(GITHUB_LATEST_RELEASE_URL, MAX_METADATA_BYTES, true)?;
    let json = String::from_utf8(body_bytes)
        .map_err(|_| String::from("Dữ liệu phiên bản từ GitHub không hợp lệ (UTF-8)."))?;
    let Some(release) = parse_github_latest_release_json(&json) else {
        return Err(String::from(
            "Chưa tìm thấy bản phát hành phù hợp trên GitHub Releases.",
        ));
    };

    if is_newer_version(&release.version_tag, APP_VERSION) {
        Ok(Some(release))
    } else {
        Ok(None)
    }
}

fn download_and_stage_sync(release: &UpdateReleaseInfo) -> Result<(), String> {
    if !is_trusted_github_https_url(&release.exe_url) {
        return Err(String::from("Liên kết tải xuống không thuộc máy chủ GitHub hợp lệ."));
    }

    let expected_sha256 = if let Some(sha_url) = &release.sha256_url {
        if !is_trusted_github_https_url(sha_url) {
            return Err(String::from("Liên kết tệp SHA-256 không hợp lệ."));
        }
        let sha_bytes = winhttp_get_bytes(sha_url, 16 * 1024, false)?;
        let sha_text = String::from_utf8(sha_bytes)
            .map_err(|_| String::from("Tệp SHA-256 không đúng định dạng văn bản."))?;
        let parsed = parse_sha256_file(&sha_text)
            .ok_or_else(|| String::from("Không đọc được mã kiểm tra SHA-256."))?;
        Some(parsed)
    } else {
        None
    };

    let exe_bytes = winhttp_get_bytes(&release.exe_url, MAX_BINARY_BYTES, false)?;
    if exe_bytes.len() < MIN_VALID_EXE_BYTES || !exe_bytes.starts_with(b"MZ") {
        return Err(String::from("Tệp tải về không phải tệp thực thi Windows hợp lệ."));
    }

    if let Some(expected) = expected_sha256 {
        let actual = sha256_hex(&exe_bytes);
        if actual != expected {
            return Err(String::from(
                "Mã kiểm tra SHA-256 không khớp. Hủy cập nhật để bảo vệ an toàn hệ thống.",
            ));
        }
    }

    let current_exe =
        std::env::current_exe().map_err(|e| format!("Không xác định được vị trí ứng dụng: {e}"))?;
    let staging_path = staging_path_for(&current_exe);
    let backup_path = backup_path_for(&current_exe);

    let _ = fs::remove_file(&staging_path);
    fs::write(&staging_path, &exe_bytes)
        .map_err(|e| format!("Không thể ghi tệp cập nhật tạm thời: {e}"))?;

    if backup_path.exists() {
        let _ = fs::remove_file(&backup_path);
    }

    if let Err(err) = fs::rename(&current_exe, &backup_path) {
        let _ = fs::remove_file(&staging_path);
        return Err(format!("Không thể sao lưu tệp cũ trước khi thay thế: {err}"));
    }

    if let Err(err) = fs::rename(&staging_path, &current_exe) {
        let _ = fs::rename(&backup_path, &current_exe);
        let _ = fs::remove_file(&staging_path);
        return Err(format!("Không thể thay thế tệp cập nhật mới: {err}"));
    }

    Ok(())
}

fn backup_path_for(exe: &Path) -> PathBuf {
    let mut os = exe.as_os_str().to_os_string();
    os.push(".old");
    PathBuf::from(os)
}

fn staging_path_for(exe: &Path) -> PathBuf {
    let mut os = exe.as_os_str().to_os_string();
    os.push(".new");
    PathBuf::from(os)
}

fn parse_https_url(url: &str) -> Option<(String, u16, String)> {
    let rest = url.strip_prefix("https://")?;
    let (authority, path_and_query) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => {
            let parsed_port = p.parse::<u16>().ok()?;
            (h, parsed_port)
        }
        None => (authority, 443),
    };
    if host.is_empty() {
        return None;
    }
    Some((
        host.to_string(),
        port,
        path_and_query.to_string(),
    ))
}

pub fn is_trusted_github_https_url(url: &str) -> bool {
    let Some((host, _port, _path)) = parse_https_url(url) else {
        return false;
    };
    let lower = host.to_ascii_lowercase();
    lower == "api.github.com"
        || lower == "github.com"
        || lower == "objects.githubusercontent.com"
        || lower == "release-assets.githubusercontent.com"
        || lower.ends_with(".githubusercontent.com")
}

fn winhttp_get_bytes(url: &str, max_bytes: usize, json_headers: bool) -> Result<Vec<u8>, String> {
    let (host, port, path) =
        parse_https_url(url).ok_or_else(|| String::from("URL cập nhật không hợp lệ."))?;
    let agent_w = to_wide_null(&format!("WinDynamicIsland/{APP_VERSION}"));
    let session_raw = unsafe {
        let h = WinHttpOpen(
            PCWSTR(agent_w.as_ptr()),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        );
        if h.is_null() {
            WinHttpOpen(
                PCWSTR(agent_w.as_ptr()),
                WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            )
        } else {
            h
        }
    };
    if session_raw.is_null() {
        return Err(String::from("Không thể khởi tạo phiên kết nối WinHTTP."));
    }
    let session = WinHttpHandle(session_raw);
    unsafe {
        let _ = WinHttpSetTimeouts(session.0, 5_000, 8_000, 10_000, 20_000);
    }

    let host_w = to_wide_null(&host);
    let connect_raw = unsafe { WinHttpConnect(session.0, PCWSTR(host_w.as_ptr()), port, 0) };
    if connect_raw.is_null() {
        return Err(String::from("Không thể kết nối tới máy chủ GitHub."));
    }
    let connect = WinHttpHandle(connect_raw);

    let verb_w = to_wide_null("GET");
    let path_w = to_wide_null(&path);
    let req_raw = unsafe {
        WinHttpOpenRequest(
            connect.0,
            PCWSTR(verb_w.as_ptr()),
            PCWSTR(path_w.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        )
    };
    if req_raw.is_null() {
        return Err(String::from("Không thể tạo yêu cầu HTTPS."));
    }
    let req = WinHttpHandle(req_raw);

    let redirect_policy = WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP.to_le_bytes();
    unsafe {
        WinHttpSetOption(
            Some(req.0 as *const c_void),
            WINHTTP_OPTION_REDIRECT_POLICY,
            Some(&redirect_policy),
        )
        .map_err(|_| String::from("Không cấu hình được chính sách chuyển hướng HTTPS."))?;
    }

    let extra_headers = if json_headers {
        Some(to_wide_null(
            "Accept: application/vnd.github+json\r\nX-GitHub-Api-Version: 2022-11-28\r\n",
        ))
    } else {
        None
    };
    let send_res = unsafe {
        WinHttpSendRequest(
            req.0,
            extra_headers.as_deref().map(|v| &v[..v.len().saturating_sub(1)]),
            None,
            0,
            0,
            0,
        )
    };
    send_res.map_err(|_| String::from("Không thể gửi yêu cầu kiểm tra cập nhật (kiểm tra mạng)."))?;

    unsafe {
        WinHttpReceiveResponse(req.0, std::ptr::null_mut())
            .map_err(|_| String::from("Máy chủ GitHub không phản hồi kịp thời."))?;
    }

    let mut status_code: u32 = 0;
    let mut status_len = std::mem::size_of::<u32>() as u32;
    let mut header_idx: u32 = 0;
    unsafe {
        WinHttpQueryHeaders(
            req.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some((&raw mut status_code).cast()),
            &mut status_len,
            &mut header_idx,
        )
        .map_err(|_| String::from("Không đọc được mã trạng thái HTTP."))?;
    }
    if status_code != 200 {
        if status_code == 404 {
            return Err(String::from(
                "Kho lưu trữ GitHub chưa có bản phát hành (Release) nào.",
            ));
        }
        return Err(format!("Máy chủ GitHub trả về mã lỗi HTTP {status_code}."));
    }

    let mut out = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let mut available: u32 = 0;
        unsafe {
            WinHttpQueryDataAvailable(req.0, &mut available)
                .map_err(|_| String::from("Lỗi khi đọc kích thước dữ liệu từ mạng."))?;
        }
        if available == 0 {
            break;
        }
        let mut remaining = available as usize;
        while remaining > 0 {
            let want = remaining.min(chunk.len());
            if out.len().saturating_add(want) > max_bytes {
                return Err(String::from("Dữ liệu tải về vượt quá giới hạn cho phép."));
            }
            let mut read: u32 = 0;
            unsafe {
                WinHttpReadData(req.0, chunk.as_mut_ptr().cast(), want as u32, &mut read)
                    .map_err(|_| String::from("Kết nối tải về bị ngắt giữa chừng."))?;
            }
            if read == 0 {
                break;
            }
            out.extend_from_slice(&chunk[..read as usize]);
            remaining = remaining.saturating_sub(read as usize);
        }
    }
    Ok(out)
}

pub fn parse_semver(tag: &str) -> Option<(u32, u32, u32)> {
    let clean = tag.trim().trim_start_matches(['v', 'V']);
    if clean.is_empty() || clean.contains('-') {
        return None;
    }
    let mut parts = clean.split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next()?.parse::<u32>().ok()?;
    let patch = parts.next().unwrap_or("0").parse::<u32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

pub fn is_newer_version(remote_tag: &str, current_version: &str) -> bool {
    match (parse_semver(remote_tag), parse_semver(current_version)) {
        (Some(remote), Some(current)) => remote > current,
        _ => false,
    }
}

pub fn parse_github_latest_release_json(json: &str) -> Option<UpdateReleaseInfo> {
    let tag_name = extract_json_string_field_top_level(json, "tag_name")?;
    let assets_slice = extract_json_array_section(json, "assets")?;

    let mut exe_url: Option<String> = None;
    let mut sha256_url: Option<String> = None;

    for obj in iter_json_objects_in_array(assets_slice) {
        let Some(name) = extract_json_string_field_top_level(obj, "name") else {
            continue;
        };
        let Some(url) = extract_json_string_field_top_level(obj, "browser_download_url") else {
            continue;
        };
        if !is_trusted_github_https_url(&url) {
            continue;
        }
        if name.eq_ignore_ascii_case("win-dynamic-island.exe") {
            exe_url = Some(url);
        } else if name.eq_ignore_ascii_case("win-dynamic-island.exe.sha256") {
            sha256_url = Some(url);
        }
    }

    Some(UpdateReleaseInfo {
        version_tag: tag_name,
        exe_url: exe_url?,
        sha256_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_comparison_handles_prefixes_and_prereleases() {
        assert!(is_newer_version("v0.2.0", "0.1.0"));
        assert!(is_newer_version("0.1.1", "0.1.0"));
        assert!(!is_newer_version("v0.1.0", "0.1.0"));
        assert!(!is_newer_version("v0.0.9", "0.1.0"));
        assert!(!is_newer_version("v0.2.0-beta1", "0.1.0"));
    }

    #[test]
    fn github_release_json_parser_extracts_exe_and_sha256_assets() {
        let sample = r#"{
            "url": "https://api.github.com/repos/kha0305/dynamic-island-win/releases/latest",
            "author": { "tag_name": "nested-ignored" },
            "tag_name": "v0.2.1",
            "assets": [
                {
                    "name": "WinDynamicIsland-win64.zip",
                    "browser_download_url": "https://github.com/kha0305/dynamic-island-win/releases/download/v0.2.1/WinDynamicIsland-win64.zip"
                },
                {
                    "name": "win-dynamic-island.exe",
                    "browser_download_url": "https://github.com/kha0305/dynamic-island-win/releases/download/v0.2.1/win-dynamic-island.exe"
                },
                {
                    "name": "win-dynamic-island.exe.sha256",
                    "browser_download_url": "https://github.com/kha0305/dynamic-island-win/releases/download/v0.2.1/win-dynamic-island.exe.sha256"
                }
            ]
        }"#;

        let parsed = parse_github_latest_release_json(sample).expect("Phai parse duoc JSON release");
        assert_eq!(parsed.version_tag, "v0.2.1");
        assert_eq!(
            parsed.exe_url,
            "https://github.com/kha0305/dynamic-island-win/releases/download/v0.2.1/win-dynamic-island.exe"
        );
        assert_eq!(
            parsed.sha256_url.as_deref(),
            Some("https://github.com/kha0305/dynamic-island-win/releases/download/v0.2.1/win-dynamic-island.exe.sha256")
        );
    }

    #[test]
    fn trusted_url_check_blocks_non_https_and_foreign_hosts() {
        assert!(is_trusted_github_https_url(
            "https://github.com/kha0305/dynamic-island-win/releases/download/v0.1.0/win-dynamic-island.exe"
        ));
        assert!(is_trusted_github_https_url(
            "https://objects.githubusercontent.com/github-production-release-asset/123"
        ));
        assert!(!is_trusted_github_https_url(
            "http://github.com/kha0305/dynamic-island-win/releases/download/v0.1.0/win-dynamic-island.exe"
        ));
        assert!(!is_trusted_github_https_url("https://evil.example.com/win-dynamic-island.exe"));
    }
}
