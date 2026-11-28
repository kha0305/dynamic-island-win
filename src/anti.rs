use std::ffi::c_void;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use windows::Win32::Foundation::{
    CloseHandle, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM, WAIT_OBJECT_0, WPARAM,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ACTION_ADDED, FILE_ACTION_MODIFIED, FILE_ACTION_RENAMED_NEW_NAME,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_LIST_DIRECTORY, FILE_NOTIFY_CHANGE_FILE_NAME,
    FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_NOTIFY_INFORMATION, FILE_SHARE_DELETE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, OPEN_EXISTING, ReadDirectoryChangesW,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    CreateEventW, GetCurrentProcessId, ResetEvent, SetEvent, WaitForMultipleObjects,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::PCWSTR;

use crate::json_mini::{
    escape_json_string, extract_json_bool_field_top_level, extract_json_object_section,
    extract_json_string_field_top_level, extract_json_u64_field_top_level,
};
use crate::tray::to_wide_null;

pub const WM_APP_ANTI_UPDATE: u32 = WM_APP + 8;
pub const ANTI_HOOK_FLAG: &str = "--anti-hook";
const ANTI_HOOK_KEY: &str = "win-dynamic-island-status";
const ANTI_PROGRESS_FILE_NAME: &str = "anti-progress.json";
const ANTI_SESSIONS_DIR_NAME: &str = "anti-sessions";
const IDE_TRANSCRIPT_FILE_NAME: &str = "transcript.jsonl";
/// Nếu trạng thái đang chạy không cập nhật quá 6 phút thì coi như phiên đã kết thúc/treo.
const STALE_RUNNING_MS: u64 = 360_000;
const IDE_STARTUP_RECENT_MS: u64 = 15_000;
const SESSION_RETENTION_MS: u64 = 1_800_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AntiAgentState {
    ReadingProject,
    Analyzing,
    Coding,
    Testing,
    Building,
    WaitingApproval,
    FixingError,
    Error,
    Canceled,
    Done,
}

impl AntiAgentState {
    pub fn as_key(self) -> &'static str {
        match self {
            Self::ReadingProject => "reading_project",
            Self::Analyzing => "analyzing",
            Self::Coding => "coding",
            Self::Testing => "testing",
            Self::Building => "building",
            Self::WaitingApproval => "waiting_approval",
            Self::FixingError => "fixing_error",
            Self::Error => "error",
            Self::Canceled => "canceled",
            Self::Done => "done",
        }
    }

    pub fn from_key(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "reading_project" | "reading" => Some(Self::ReadingProject),
            "analyzing" | "thinking" => Some(Self::Analyzing),
            "coding" | "editing" => Some(Self::Coding),
            "testing" => Some(Self::Testing),
            "building" => Some(Self::Building),
            "waiting_approval" | "waiting" => Some(Self::WaitingApproval),
            "fixing_error" | "fixing" => Some(Self::FixingError),
            "error" | "failed" => Some(Self::Error),
            "canceled" | "cancelled" => Some(Self::Canceled),
            "done" | "completed" => Some(Self::Done),
            _ => None,
        }
    }

    pub fn label_vi(self) -> &'static str {
        match self {
            Self::ReadingProject => "Đang đọc dự án",
            Self::Analyzing => "Đang phân tích",
            Self::Coding => "Đang code",
            Self::Testing => "Đang test",
            Self::Building => "Đang build",
            Self::WaitingApproval => "Đang chờ duyệt",
            Self::FixingError => "Đang sửa lỗi",
            Self::Error => "Bị lỗi",
            Self::Canceled => "Đã hủy",
            Self::Done => "Đã xong",
        }
    }

    pub fn is_running(self) -> bool {
        !matches!(self, Self::Error | Self::Canceled | Self::Done)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AntiSource {
    AgyCli,
    AntiIde,
    AntiApp,
    Antigravity,
}

impl AntiSource {
    pub fn as_key(self) -> &'static str {
        match self {
            Self::AgyCli => "agy-cli",
            Self::AntiIde => "anti-ide",
            Self::AntiApp => "anti-app",
            Self::Antigravity => "antigravity",
        }
    }

    pub fn from_key(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "agy-cli" | "agy" | "cli" => Self::AgyCli,
            "anti-ide" | "ide" => Self::AntiIde,
            "anti-app" | "app" => Self::AntiApp,
            _ => Self::Antigravity,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::AgyCli => "agy CLI",
            Self::AntiIde => "Anti IDE",
            Self::AntiApp => "Anti App",
            Self::Antigravity => "Antigravity",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AntiProgressSnapshot {
    pub source: AntiSource,
    pub conversation_id: String,
    pub state: AntiAgentState,
    pub detail: String,
    pub recent_steps: Vec<String>,
    pub had_error: bool,
    pub updated_unix_ms: u64,
}

impl AntiProgressSnapshot {
    pub fn slot_key(&self) -> String {
        if self.conversation_id.trim().is_empty() {
            self.source.as_key().to_string()
        } else {
            format!("{}:{}", self.source.as_key(), self.conversation_id)
        }
    }

    pub fn is_stale(&self, now_ms: u64) -> bool {
        if self.state == AntiAgentState::WaitingApproval {
            return now_ms.saturating_sub(self.updated_unix_ms) > STALE_RUNNING_MS * 3;
        }
        self.state.is_running() && now_ms.saturating_sub(self.updated_unix_ms) > STALE_RUNNING_MS
    }

    pub fn to_json(&self) -> String {
        let joined_steps = self.recent_steps.join("\n");
        format!(
            "{{\n  \"source\": \"{}\",\n  \"conversation_id\": \"{}\",\n  \"state\": \"{}\",\n  \"label\": \"{}\",\n  \"detail\": \"{}\",\n  \"recent_steps\": \"{}\",\n  \"active\": {},\n  \"had_error\": {},\n  \"updated_unix_ms\": {}\n}}\n",
            self.source.as_key(),
            escape_json_string(&self.conversation_id),
            self.state.as_key(),
            escape_json_string(self.state.label_vi()),
            escape_json_string(&self.detail),
            escape_json_string(&joined_steps),
            self.state.is_running(),
            self.had_error,
            self.updated_unix_ms,
        )
    }

    pub fn from_json(raw: &str) -> Option<Self> {
        let state_str = extract_json_string_field_top_level(raw, "state")?;
        let state = AntiAgentState::from_key(&state_str)?;
        let source = extract_json_string_field_top_level(raw, "source")
            .map(|s| AntiSource::from_key(&s))
            .unwrap_or(AntiSource::Antigravity);
        let conversation_id =
            extract_json_string_field_top_level(raw, "conversation_id").unwrap_or_default();
        let detail = extract_json_string_field_top_level(raw, "detail").unwrap_or_default();
        let recent_steps = extract_json_string_field_top_level(raw, "recent_steps")
            .map(|s| {
                s.lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .take(3)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let had_error = extract_json_bool_field_top_level(raw, "had_error").unwrap_or(false);
        let updated_unix_ms =
            extract_json_u64_field_top_level(raw, "updated_unix_ms").unwrap_or(0);
        Some(Self {
            source,
            conversation_id,
            state,
            detail,
            recent_steps,
            had_error,
            updated_unix_ms,
        })
    }
}

pub fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

pub fn anti_progress_dir() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    if base.is_empty() {
        return None;
    }
    Some(PathBuf::from(base).join("WinDynamicIsland"))
}

pub fn anti_progress_path() -> Option<PathBuf> {
    Some(anti_progress_dir()?.join(ANTI_PROGRESS_FILE_NAME))
}

fn anti_sessions_dir() -> Option<PathBuf> {
    Some(anti_progress_dir()?.join(ANTI_SESSIONS_DIR_NAME))
}

fn ide_brain_dir() -> Option<PathBuf> {
    let profile = std::env::var_os("USERPROFILE")?;
    if profile.is_empty() {
        return None;
    }
    Some(
        PathBuf::from(profile)
            .join(".gemini")
            .join("antigravity")
            .join("brain"),
    )
}

pub fn write_anti_progress_atomic(path: &Path, snap: &AntiProgressSnapshot) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Tao thu muc anti-progress: {e}"))?;
    }
    let tmp = path.with_extension(format!("json.tmp.{}", std::process::id()));
    fs::write(&tmp, snap.to_json()).map_err(|e| format!("Ghi tam anti-progress: {e}"))?;
    if let Err(err) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("Doi ten atomic anti-progress: {err}"));
    }
    Ok(())
}

pub fn read_anti_progress(path: &Path) -> Option<AntiProgressSnapshot> {
    let text = fs::read_to_string(path).ok()?;
    AntiProgressSnapshot::from_json(&text)
}

pub fn detect_source_from_paths(transcript_path: Option<&str>, artifact_dir: Option<&str>) -> AntiSource {
    for raw in [transcript_path, artifact_dir].into_iter().flatten() {
        let norm = raw.replace('\\', "/").to_ascii_lowercase();
        if norm.contains("/antigravity-cli/") || norm.contains("antigravity-cli/") {
            return AntiSource::AgyCli;
        }
        if norm.contains("/antigravity-ide/") || norm.contains("antigravity-ide/") {
            return AntiSource::AntiIde;
        }
        if norm.contains("/antigravity/") || norm.contains("antigravity/") {
            return AntiSource::AntiApp;
        }
    }
    AntiSource::Antigravity
}

pub fn classify_exe_name_to_source(exe_name: &str) -> Option<AntiSource> {
    let file_name = Path::new(exe_name)
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_else(|| exe_name.trim().to_ascii_lowercase());
    if file_name == "agy.exe"
        || file_name.starts_with("agy-")
        || file_name == "antigravity-cli.exe"
    {
        return Some(AntiSource::AgyCli);
    }
    if file_name.contains("antigravity") && file_name.contains("ide") {
        return Some(AntiSource::AntiIde);
    }
    if file_name == "antigravity.exe"
        || (file_name.contains("antigravity") && !file_name.contains("cli"))
    {
        return Some(AntiSource::AntiApp);
    }
    None
}

pub struct AntiEnvHints<'a> {
    pub ls_version: Option<&'a str>,
    pub jetski_app_data_dir: Option<&'a str>,
    pub agentapi_exe: Option<&'a str>,
    pub vscode_pid_exe: Option<&'a str>,
}

pub fn classify_anti_source_inputs(
    env: &AntiEnvHints<'_>,
    ancestor_exes: &[String],
    running_exes: &[String],
    transcript_path: Option<&str>,
    artifact_dir: Option<&str>,
) -> AntiSource {
    if let Some(ver) = env.ls_version
        && ver.trim().to_ascii_lowercase().starts_with("cli-")
    {
        return AntiSource::AgyCli;
    }
    if let Some(app_dir) = env.jetski_app_data_dir {
        match app_dir.trim().to_ascii_lowercase().as_str() {
            "antigravity-cli" => return AntiSource::AgyCli,
            "antigravity-ide" => return AntiSource::AntiIde,
            _ => {}
        }
    }
    if let Some(exe) = env.agentapi_exe
        && let Some(src) = classify_exe_name_to_source(exe)
    {
        return src;
    }
    if let Some(vsc_exe) = env.vscode_pid_exe
        && let Some(src) = classify_exe_name_to_source(vsc_exe)
    {
        return src;
    }

    for exe in ancestor_exes {
        if let Some(src) = classify_exe_name_to_source(exe) {
            return src;
        }
    }

    let path_guess = detect_source_from_paths(transcript_path, artifact_dir);
    if path_guess == AntiSource::AntiApp {
        let has_ide = running_exes
            .iter()
            .any(|e| classify_exe_name_to_source(e) == Some(AntiSource::AntiIde));
        let has_app = running_exes
            .iter()
            .any(|e| classify_exe_name_to_source(e) == Some(AntiSource::AntiApp));
        if has_ide && !has_app {
            return AntiSource::AntiIde;
        }
        if has_app {
            return AntiSource::AntiApp;
        }
        return AntiSource::Antigravity;
    }
    path_guess
}

fn collect_process_ancestors_and_running(vscode_pid: Option<u32>) -> (Vec<String>, Vec<String>, Option<String>) {
    let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return (Vec::new(), Vec::new(), None);
    };
    let _guard = HandleGuard(snapshot);

    let mut entries: Vec<(u32, u32, String)> = Vec::new();
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    if unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok() {
        loop {
            let end = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
            entries.push((entry.th32ProcessID, entry.th32ParentProcessID, name));
            if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
                break;
            }
        }
    }

    let mut ancestors = Vec::new();
    let mut visited = Vec::with_capacity(10);
    let mut cur_pid = unsafe { GetCurrentProcessId() };
    for _ in 0..10 {
        if visited.contains(&cur_pid) {
            break;
        }
        visited.push(cur_pid);
        let Some((_, ppid, _)) = entries.iter().find(|(pid, _, _)| *pid == cur_pid) else {
            break;
        };
        if *ppid == 0 || *ppid == cur_pid {
            break;
        }
        if let Some((_, _, p_name)) = entries.iter().find(|(pid, _, _)| *pid == *ppid) {
            ancestors.push(p_name.clone());
        }
        cur_pid = *ppid;
    }

    let vscode_exe = vscode_pid.and_then(|target| {
        entries
            .iter()
            .find(|(pid, _, _)| *pid == target)
            .map(|(_, _, name)| name.clone())
    });

    let running = entries
        .into_iter()
        .map(|(_, _, name)| name)
        .filter(|name| classify_exe_name_to_source(name).is_some())
        .collect();
    (ancestors, running, vscode_exe)
}

pub fn detect_source_from_runtime(transcript_path: Option<&str>, artifact_dir: Option<&str>) -> AntiSource {
    let ls_ver = std::env::var("ANTIGRAVITY_LS_VERSION").ok();
    let jetski_dir = std::env::var("JETSKI_APP_DATA_DIR").ok();
    let agentapi_exe = std::env::var("ANTIGRAVITY_AGENTAPI_EXE").ok();
    let vscode_pid = std::env::var("VSCODE_PID")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok());
    let (ancestors, running, vscode_exe) = collect_process_ancestors_and_running(vscode_pid);
    let env = AntiEnvHints {
        ls_version: ls_ver.as_deref(),
        jetski_app_data_dir: jetski_dir.as_deref(),
        agentapi_exe: agentapi_exe.as_deref(),
        vscode_pid_exe: vscode_exe.as_deref(),
    };
    classify_anti_source_inputs(&env, &ancestors, &running, transcript_path, artifact_dir)
}

fn running_anti_exes() -> Vec<String> {
    collect_process_ancestors_and_running(None).1
}

fn is_shell_boundary_exe(exe_name: &str) -> bool {
    let lower = Path::new(exe_name)
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_else(|| exe_name.trim().to_ascii_lowercase());
    matches!(
        lower.as_str(),
        "explorer.exe"
            | "svchost.exe"
            | "services.exe"
            | "wininit.exe"
            | "csrss.exe"
            | "smss.exe"
            | "lsass.exe"
            | "sihost.exe"
            | "taskmgr.exe"
            | "codex-windows-sandbox-service.exe"
    )
}

pub fn collect_ancestor_pids_from_entries(
    entries: &[(u32, u32, String)],
    source: AntiSource,
) -> Vec<u32> {
    let mut out = Vec::new();
    for (pid, ppid, exe) in entries {
        if classify_exe_name_to_source(exe) != Some(source) {
            continue;
        }
        let mut chain = vec![*pid];
        let mut cur_ppid = *ppid;
        let mut hops = 0_u8;
        let mut owned_by_other_anti = false;
        while cur_ppid != 0 && hops < 6 {
            hops += 1;
            let Some((_, next_ppid, p_exe)) = entries.iter().find(|(p, _, _)| *p == cur_ppid)
            else {
                break;
            };
            if is_shell_boundary_exe(p_exe) {
                break;
            }
            if let Some(parent_src) = classify_exe_name_to_source(p_exe)
                && parent_src != source
            {
                // Bỏ qua tiến trình con dùng chung (như `agy.exe --hub` do `Antigravity IDE.exe` sinh ra).
                owned_by_other_anti = true;
                break;
            }
            chain.push(cur_ppid);
            if *next_ppid == cur_ppid {
                break;
            }
            cur_ppid = *next_ppid;
        }
        if !owned_by_other_anti {
            for id in chain {
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
    }
    out
}

pub fn collect_target_pids_for_source(source: AntiSource) -> Vec<u32> {
    let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return Vec::new();
    };
    let _guard = HandleGuard(snapshot);
    let mut entries: Vec<(u32, u32, String)> = Vec::new();
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    if unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok() {
        loop {
            let end = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
            entries.push((entry.th32ProcessID, entry.th32ParentProcessID, name));
            if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
                break;
            }
        }
    }
    collect_ancestor_pids_from_entries(&entries, source)
}

fn sanitize_detail(raw: &str) -> String {
    let compact: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut out = String::new();
    for ch in compact.chars().take(96) {
        out.push(ch);
    }
    out
}

pub fn summarize_command_hint(raw_cmd: &str) -> String {
    let trimmed = raw_cmd.trim();
    if trimmed.is_empty() {
        return String::from("Chạy lệnh hệ thống");
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("cargo test") {
        return String::from("Chạy kiểm thử Cargo");
    }
    if lower.contains("cargo clippy") {
        return String::from("Kiểm tra Clippy");
    }
    if lower.contains("cargo build") {
        return String::from("Biên dịch ứng dụng");
    }
    if lower.contains("cargo check") {
        return String::from("Kiểm tra biên dịch Cargo");
    }
    if lower.starts_with("git ") || lower.contains(" git ") {
        return String::from("Thao tác Git");
    }
    if lower.contains("pytest") || lower.contains("vitest") || lower.contains("npm test") {
        return String::from("Chạy bộ kiểm thử");
    }
    if trimmed.starts_with('$') || trimmed.contains("| ") || trimmed.contains("Get-ChildItem") {
        return String::from("Truy vấn PowerShell");
    }
    sanitize_detail(trimmed)
}

fn classify_tool_call(
    tool_name: &str,
    tool_args_json: &str,
    had_prev_error: bool,
) -> (AntiAgentState, String) {
    let name = tool_name.trim().to_ascii_lowercase();
    if name.contains("ask_question") || name.contains("request_feedback") {
        return (AntiAgentState::WaitingApproval, String::from("Chờ bạn xác nhận"));
    }
    if matches!(
        name.as_str(),
        "write_file" | "edit_file" | "multi_edit" | "replace_file_content" | "multi_replace_file_content"
    ) {
        let file_hint = extract_json_string_field_top_level(tool_args_json, "Path")
            .or_else(|| extract_json_string_field_top_level(tool_args_json, "TargetFile"))
            .and_then(|p| {
                Path::new(&p)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
            })
            .unwrap_or_default();
        let st = if had_prev_error {
            AntiAgentState::FixingError
        } else {
            AntiAgentState::Coding
        };
        let msg = if file_hint.is_empty() {
            String::from("Chỉnh sửa mã nguồn")
        } else {
            format!("Sửa {file_hint}")
        };
        return (st, sanitize_detail(&msg));
    }
    if matches!(
        name.as_str(),
        "read_file"
            | "list_resources"
            | "read_resource"
            | "view_file"
            | "view_file_outline"
            | "view_code_item"
            | "grep_search"
            | "find_by_name"
            | "list_dir"
    ) {
        let hint = extract_json_string_field_top_level(tool_args_json, "Path")
            .and_then(|p| {
                Path::new(&p)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
            })
            .unwrap_or_default();
        let msg = if hint.is_empty() {
            String::from("Đọc tệp dự án")
        } else {
            format!("Đọc {hint}")
        };
        return (AntiAgentState::ReadingProject, sanitize_detail(&msg));
    }
    if name == "run_command" || name == "command_status" {
        let summary = extract_json_string_field_top_level(tool_args_json, "toolSummary")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let cmd = extract_json_string_field_top_level(tool_args_json, "CommandLine")
            .unwrap_or_default();
        let lower = format!(
            "{} {}",
            summary.as_deref().unwrap_or(""),
            cmd
        )
        .to_ascii_lowercase();
        let st = if lower.contains("test")
            || lower.contains("pytest")
            || lower.contains("vitest")
            || lower.contains("jest")
        {
            AntiAgentState::Testing
        } else if lower.contains("build")
            || lower.contains("clippy")
            || lower.contains("check")
            || lower.contains("rustc")
            || lower.contains("tsc")
            || lower.contains("package")
        {
            AntiAgentState::Building
        } else if had_prev_error {
            AntiAgentState::FixingError
        } else {
            AntiAgentState::Analyzing
        };
        let display = summary.unwrap_or_else(|| summarize_command_hint(&cmd));
        return (st, sanitize_detail(&display));
    }
    if had_prev_error {
        return (AntiAgentState::FixingError, sanitize_detail(tool_name));
    }
    (AntiAgentState::Analyzing, sanitize_detail(tool_name))
}

const TRANSCRIPT_TAIL_MAX_BYTES: u64 = 128 * 1024;

fn read_transcript_tail_text(transcript_path: &Path) -> Option<(String, u64)> {
    let mut file = fs::File::open(transcript_path).ok()?;
    let meta = file.metadata().ok()?;
    let file_len = meta.len();
    if file_len == 0 {
        return None;
    }
    let modified_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64);
    let start_offset = file_len.saturating_sub(TRANSCRIPT_TAIL_MAX_BYTES);
    if start_offset > 0 {
        file.seek(SeekFrom::Start(start_offset)).ok()?;
    }

    let mut buf = Vec::with_capacity(
        (file_len - start_offset).min(TRANSCRIPT_TAIL_MAX_BYTES) as usize,
    );
    file.take(TRANSCRIPT_TAIL_MAX_BYTES)
        .read_to_end(&mut buf)
        .ok()?;
    let tail_bytes = if start_offset > 0 {
        let first_nl = buf.iter().position(|&b| b == b'\n')?;
        &buf[first_nl + 1..]
    } else {
        &buf[..]
    };
    Some((String::from_utf8_lossy(tail_bytes).into_owned(), modified_ms))
}

/// Đọc tối đa phần đuôi `transcript.jsonl` để nắm ngay tool đang chuẩn bị chạy mà không nạp cả tệp lớn.
fn infer_state_from_transcript_tail(
    transcript_path: &Path,
    had_prev_error: bool,
) -> Option<(AntiAgentState, String)> {
    let (text, _) = read_transcript_tail_text(transcript_path)?;
    for line in text.lines().rev().take(8) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let kind = extract_json_string_field_top_level(trimmed, "type").unwrap_or_default();
        if kind == "PLANNER_RESPONSE" {
            if let Some(calls) = crate::json_mini::extract_json_array_section(trimmed, "tool_calls") {
                for obj in crate::json_mini::iter_json_objects_in_array(calls).into_iter().rev() {
                    if let Some(name) = extract_json_string_field_top_level(obj, "name") {
                        let args = extract_json_object_section(obj, "args").unwrap_or("");
                        return Some(classify_tool_call(&name, args, had_prev_error));
                    }
                }
            }
            break;
        }
    }
    None
}

fn generic_step_has_error(content: &str, status: &str) -> bool {
    if status.eq_ignore_ascii_case("ERROR") || status.eq_ignore_ascii_case("FAILED") {
        return true;
    }
    let lower = content.to_ascii_lowercase();
    if let Some(idx) = lower.find("command exited with code ") {
        let rest = lower[idx + "command exited with code ".len()..].trim_start();
        let code_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !code_str.is_empty() && code_str != "0" {
            return true;
        }
    }
    lower.contains("encountered error in tool execution")
}

pub fn extract_conv_id_from_brain_path(brain_root: &Path, file_path: &Path) -> String {
    let Ok(rel) = file_path.strip_prefix(brain_root) else {
        return String::new();
    };
    rel.components()
        .next()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .filter(|s| !s.is_empty() && !s.starts_with('.'))
        .unwrap_or_default()
}

pub fn detect_ide_source_from_exes(running_exes: &[String]) -> AntiSource {
    let has_ide = running_exes
        .iter()
        .any(|e| classify_exe_name_to_source(e) == Some(AntiSource::AntiIde));
    let has_app = running_exes
        .iter()
        .any(|e| classify_exe_name_to_source(e) == Some(AntiSource::AntiApp));
    if has_ide && !has_app {
        AntiSource::AntiIde
    } else if has_app && !has_ide {
        AntiSource::AntiApp
    } else if has_ide {
        AntiSource::AntiIde
    } else {
        AntiSource::Antigravity
    }
}

pub fn parse_ide_transcript_tail(
    transcript_path: &Path,
    conversation_id: &str,
    source: AntiSource,
    fallback_now_ms: u64,
) -> Option<AntiProgressSnapshot> {
    let (text, modified_ms) = read_transcript_tail_text(transcript_path)?;
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return None;
    }
    let window = &lines[lines.len().saturating_sub(12)..];
    let last_line = *window.last()?;

    let mut had_error = false;
    let mut recent_steps: Vec<String> = Vec::with_capacity(3);
    let mut last_tool_pair: Option<(AntiAgentState, String)> = None;

    for line in window {
        let kind = extract_json_string_field_top_level(line, "type").unwrap_or_default();
        let status = extract_json_string_field_top_level(line, "status").unwrap_or_default();
        if kind == "GENERIC" {
            let content = extract_json_string_field_top_level(line, "content").unwrap_or_default();
            if generic_step_has_error(&content, &status) {
                had_error = true;
            }
        } else if kind == "PLANNER_RESPONSE"
            && let Some(calls) = crate::json_mini::extract_json_array_section(line, "tool_calls")
        {
            for obj in crate::json_mini::iter_json_objects_in_array(calls) {
                if let Some(name) = extract_json_string_field_top_level(obj, "name") {
                    let args = extract_json_object_section(obj, "args").unwrap_or("");
                    let pair = classify_tool_call(&name, args, had_error);
                    recent_steps = append_recent_step(&recent_steps, pair.0, &pair.1);
                    last_tool_pair = Some(pair);
                    had_error = false;
                }
            }
        }
    }

    let last_kind = extract_json_string_field_top_level(last_line, "type").unwrap_or_default();
    let last_status = extract_json_string_field_top_level(last_line, "status").unwrap_or_default();
    let (state, detail, final_err) = match last_kind.as_str() {
        "USER_INPUT" => (
            AntiAgentState::Analyzing,
            String::from("Tiếp nhận yêu cầu"),
            false,
        ),
        "PLANNER_RESPONSE" => {
            if let Some(calls) =
                crate::json_mini::extract_json_array_section(last_line, "tool_calls")
                && let Some(obj) = crate::json_mini::iter_json_objects_in_array(calls)
                    .into_iter()
                    .last()
                && let Some(name) = extract_json_string_field_top_level(obj, "name")
            {
                let args = extract_json_object_section(obj, "args").unwrap_or("");
                let (st, hint) = classify_tool_call(&name, args, had_error);
                (st, hint, had_error)
            } else if last_status.eq_ignore_ascii_case("CANCELED")
                || last_status.eq_ignore_ascii_case("CANCELLED")
            {
                (
                    AntiAgentState::Canceled,
                    String::from("Người dùng hủy"),
                    false,
                )
            } else if last_status.eq_ignore_ascii_case("DONE")
                && extract_json_string_field_top_level(last_line, "content")
                    .is_some_and(|c| !c.trim().is_empty())
            {
                (AntiAgentState::Done, String::from("Hoàn tất tác vụ"), false)
            } else {
                (
                    AntiAgentState::Analyzing,
                    String::from("Đang phân tích"),
                    had_error,
                )
            }
        }
        "GENERIC" => {
            let content =
                extract_json_string_field_top_level(last_line, "content").unwrap_or_default();
            if last_status.eq_ignore_ascii_case("CANCELED")
                || last_status.eq_ignore_ascii_case("CANCELLED")
            {
                (
                    AntiAgentState::Canceled,
                    String::from("Người dùng hủy"),
                    false,
                )
            } else if last_status.eq_ignore_ascii_case("RUNNING") {
                if let Some((st, hint)) = last_tool_pair {
                    (st, hint, had_error)
                } else {
                    (
                        AntiAgentState::Analyzing,
                        String::from("Đang chạy tác vụ"),
                        had_error,
                    )
                }
            } else if generic_step_has_error(&content, &last_status) {
                (
                    AntiAgentState::FixingError,
                    String::from("Đang xử lý lỗi"),
                    true,
                )
            } else if let Some((_, hint)) = last_tool_pair {
                (AntiAgentState::Analyzing, hint, had_error)
            } else {
                (
                    AntiAgentState::Analyzing,
                    String::from("Đang xử lý kết quả"),
                    had_error,
                )
            }
        }
        _ => (AntiAgentState::Analyzing, String::new(), had_error),
    };

    let updated_unix_ms = if modified_ms > 0 {
        modified_ms
    } else {
        fallback_now_ms
    };
    let steps = if recent_steps.is_empty() {
        append_recent_step(&[], state, &detail)
    } else {
        recent_steps
    };
    Some(AntiProgressSnapshot {
        source,
        conversation_id: conversation_id.to_string(),
        state,
        detail,
        recent_steps: steps,
        had_error: final_err,
        updated_unix_ms,
    })
}

fn append_recent_step(prev_steps: &[String], state: AntiAgentState, detail: &str) -> Vec<String> {
    let trimmed = detail.trim();
    let entry = if trimmed.is_empty() {
        state.label_vi().to_string()
    } else {
        format!("{}: {}", state.label_vi(), sanitize_detail(trimmed))
    };
    let mut out = Vec::with_capacity(3);
    out.push(entry);
    for old in prev_steps {
        if out.len() >= 3 {
            break;
        }
        if old != &out[0] {
            out.push(old.clone());
        }
    }
    out
}

#[cfg(test)]
pub fn derive_anti_progress_from_hook(
    event_name: &str,
    stdin_json: &str,
    prev: Option<&AntiProgressSnapshot>,
    now_ms: u64,
) -> AntiProgressSnapshot {
    let transcript = extract_json_string_field_top_level(stdin_json, "transcriptPath");
    let artifact_dir = extract_json_string_field_top_level(stdin_json, "artifactDirectoryPath");
    let source = detect_source_from_paths(transcript.as_deref(), artifact_dir.as_deref());
    derive_anti_progress_from_hook_with_source(event_name, stdin_json, source, prev, now_ms)
}

pub fn derive_anti_progress_from_hook_with_source(
    event_name: &str,
    stdin_json: &str,
    source: AntiSource,
    prev: Option<&AntiProgressSnapshot>,
    now_ms: u64,
) -> AntiProgressSnapshot {
    let conv_id = extract_json_string_field_top_level(stdin_json, "conversationId")
        .unwrap_or_default();
    let transcript = extract_json_string_field_top_level(stdin_json, "transcriptPath");
    let hook_evt = extract_json_string_field_top_level(stdin_json, "hookEventName")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| event_name.to_string());

    let same_conv_prev = prev.filter(|p| {
        conv_id.is_empty() || p.conversation_id.is_empty() || p.conversation_id == conv_id
    });
    let prev_had_error = same_conv_prev.is_some_and(|p| p.had_error);

    let (state, detail, had_error) = match hook_evt.as_str() {
        "SessionStart" => (AntiAgentState::ReadingProject, String::from("Bắt đầu phiên"), false),
        "PreInvocation" => {
            if prev_had_error {
                (AntiAgentState::FixingError, String::from("Đang xử lý lỗi"), true)
            } else if let Some(tp) = transcript.as_deref()
                && let Some((inferred, hint)) = infer_state_from_transcript_tail(Path::new(tp), false)
            {
                (inferred, hint, false)
            } else {
                (AntiAgentState::Analyzing, String::new(), false)
            }
        }
        "PreToolUse" => {
            let tool_obj = extract_json_object_section(stdin_json, "toolCall").unwrap_or("");
            let tool_name = extract_json_string_field_top_level(tool_obj, "name").unwrap_or_default();
            let args = extract_json_object_section(tool_obj, "args").unwrap_or("");
            let (st, hint) = classify_tool_call(&tool_name, args, prev_had_error);
            (st, hint, prev_had_error)
        }
        "PostToolUse" => {
            let err = extract_json_string_field_top_level(stdin_json, "error").unwrap_or_default();
            if !err.trim().is_empty() {
                (AntiAgentState::Error, sanitize_detail(&err), true)
            } else {
                let tool_obj = extract_json_object_section(stdin_json, "toolCall").unwrap_or("");
                let tool_name =
                    extract_json_string_field_top_level(tool_obj, "name").unwrap_or_default();
                let args = extract_json_object_section(tool_obj, "args").unwrap_or("");
                let (st, hint) = classify_tool_call(&tool_name, args, false);
                (st, hint, false)
            }
        }
        "PostInvocation" => {
            if let Some(tp) = transcript.as_deref()
                && let Some((st, hint)) = infer_state_from_transcript_tail(Path::new(tp), prev_had_error)
            {
                (st, hint, prev_had_error)
            } else if prev_had_error {
                (AntiAgentState::FixingError, String::new(), true)
            } else {
                (AntiAgentState::Analyzing, String::new(), false)
            }
        }
        "Stop" => {
            let reason = extract_json_string_field_top_level(stdin_json, "terminationReason")
                .unwrap_or_default();
            let err = extract_json_string_field_top_level(stdin_json, "error").unwrap_or_default();
            if reason == "USER_CANCELED" {
                (AntiAgentState::Canceled, String::from("Người dùng hủy"), false)
            } else if reason == "ERROR" || !err.trim().is_empty() {
                (AntiAgentState::Error, sanitize_detail(&err), true)
            } else {
                (AntiAgentState::Done, String::from("Hoàn tất tác vụ"), false)
            }
        }
        _ => (AntiAgentState::Analyzing, String::new(), prev_had_error),
    };

    let prev_steps = if hook_evt == "SessionStart" {
        &[][..]
    } else {
        same_conv_prev.map_or(&[][..], |p| p.recent_steps.as_slice())
    };
    let recent_steps = append_recent_step(prev_steps, state, &detail);

    AntiProgressSnapshot {
        source,
        conversation_id: conv_id,
        state,
        detail,
        recent_steps,
        had_error,
        updated_unix_ms: now_ms,
    }
}

/// Chạy khi binary được gọi với `--anti-hook <EVENT>` từ `~/.gemini/config/hooks.json`.
pub fn run_anti_hook_cli(event_name: &str) -> i32 {
    let mut stdin_buf = String::new();
    let _ = std::io::stdin().read_to_string(&mut stdin_buf);
    let now = now_unix_ms();
    if let Some(path) = anti_progress_path() {
        let transcript = extract_json_string_field_top_level(&stdin_buf, "transcriptPath");
        let artifact_dir =
            extract_json_string_field_top_level(&stdin_buf, "artifactDirectoryPath");
        let source = detect_source_from_runtime(transcript.as_deref(), artifact_dir.as_deref());
        let conv_id = extract_json_string_field_top_level(&stdin_buf, "conversationId")
            .unwrap_or_default();
        let session_path = anti_sessions_dir().map(|dir| {
            let safe_conv: String = if conv_id.trim().is_empty() {
                String::from("default")
            } else {
                conv_id
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                    .take(64)
                    .collect()
            };
            dir.join(format!("{}-{safe_conv}.json", source.as_key()))
        });
        let prev = session_path
            .as_ref()
            .and_then(|p| read_anti_progress(p))
            .or_else(|| read_anti_progress(&path));
        let snap = derive_anti_progress_from_hook_with_source(
            event_name,
            &stdin_buf,
            source,
            prev.as_ref(),
            now,
        );
        let _ = write_anti_progress_atomic(&path, &snap);
        if let Some(sp) = session_path {
            let _ = write_anti_progress_atomic(&sp, &snap);
            if let Some(dir) = sp.parent() {
                prune_old_session_files(dir, now);
            }
        }
    }
    println!("{{}}");
    0
}

fn prune_old_session_files(dir: &Path, now_ms: u64) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(meta) = entry.metadata()
            && let Ok(modified) = meta.modified()
            && let Ok(dur) = modified.duration_since(UNIX_EPOCH)
            && now_ms.saturating_sub(dur.as_millis() as u64) > SESSION_RETENTION_MS
        {
            let _ = fs::remove_file(path);
        }
    }
}

pub fn global_hooks_json_path() -> Option<PathBuf> {
    let profile = std::env::var_os("USERPROFILE")?;
    if profile.is_empty() {
        return None;
    }
    Some(PathBuf::from(profile).join(".gemini").join("config").join("hooks.json"))
}

pub fn are_anti_hooks_installed() -> bool {
    let Some(path) = global_hooks_json_path() else {
        return false;
    };
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    text.contains(&format!("\"{ANTI_HOOK_KEY}\""))
}

fn build_hook_entry_json(exe_path: &Path) -> String {
    let exe_escaped = escape_json_string(&exe_path.to_string_lossy());
    let flat_event = |evt: &str| -> String {
        format!(
            "[\n      {{\n        \"type\": \"command\",\n        \"command\": \"\\\"{}\\\" {} {}\",\n        \"timeout\": 5\n      }}\n    ]",
            exe_escaped, ANTI_HOOK_FLAG, evt
        )
    };
    let tool_event = |evt: &str| -> String {
        format!(
            "[\n      {{\n        \"matcher\": \"*\",\n        \"hooks\": [\n          {{\n            \"type\": \"command\",\n            \"command\": \"\\\"{}\\\" {} {}\",\n            \"timeout\": 5\n          }}\n        ]\n      }}\n    ]",
            exe_escaped, ANTI_HOOK_FLAG, evt
        )
    };
    format!(
        "  \"{}\": {{\n    \"SessionStart\": {},\n    \"PreInvocation\": {},\n    \"PostToolUse\": {},\n    \"PostInvocation\": {},\n    \"Stop\": {}\n  }}",
        ANTI_HOOK_KEY,
        flat_event("SessionStart"),
        flat_event("PreInvocation"),
        tool_event("PostToolUse"),
        flat_event("PostInvocation"),
        flat_event("Stop"),
    )
}

pub fn merge_anti_hook_into_json(existing: Option<&str>, exe_path: &Path) -> Result<String, String> {
    let without_old = remove_anti_hook_from_json(existing.unwrap_or("{}"))?;
    let trimmed = without_old.trim();
    if trimmed == "{}" {
        return Ok(format!("{{\n{}\n}}\n", build_hook_entry_json(exe_path)));
    }
    let last_brace = trimmed
        .rfind('}')
        .ok_or_else(|| String::from("Tep hooks.json khong hop le (thieu dau ngoac dong)."))?;
    let body = trimmed[..last_brace].trim_end();
    Ok(format!(
        "{},\n{}\n}}\n",
        body,
        build_hook_entry_json(exe_path)
    ))
}

pub fn remove_anti_hook_from_json(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::from("{}\n"));
    }
    let Some(section) = extract_json_object_section(trimmed, ANTI_HOOK_KEY) else {
        return Ok(trimmed.to_string());
    };
    let section_start = section.as_ptr() as usize - trimmed.as_ptr() as usize;
    let section_end = section_start + section.len();

    let key_pattern = format!("\"{ANTI_HOOK_KEY}\"");
    let key_start = trimmed[..section_start]
        .rfind(&key_pattern)
        .ok_or_else(|| String::from("Khong xac dinh duoc vi tri khoa hook Anti."))?;

    let mut cut_start = key_start;
    let bytes = trimmed.as_bytes();
    while cut_start > 0 && bytes[cut_start - 1].is_ascii_whitespace() {
        cut_start -= 1;
    }
    let has_leading_comma = cut_start > 0 && bytes[cut_start - 1] == b',';
    if has_leading_comma {
        cut_start -= 1;
    }

    let mut cut_end = section_end;
    while cut_end < bytes.len() && bytes[cut_end].is_ascii_whitespace() {
        cut_end += 1;
    }
    if !has_leading_comma && cut_end < bytes.len() && bytes[cut_end] == b',' {
        cut_end += 1;
    }

    let rebuilt = format!("{}{}", trimmed[..cut_start].trim_end(), trimmed[cut_end..].trim_start());
    let clean = rebuilt.trim();
    if clean == "{}" || clean == "{ }" || clean.is_empty() {
        Ok(String::from("{}\n"))
    } else {
        Ok(format!("{clean}\n"))
    }
}

pub fn install_anti_hooks() -> Result<(), String> {
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Khong xac dinh duoc duong dan ung dung: {e}"))?;
    let hooks_path = global_hooks_json_path()
        .ok_or_else(|| String::from("Khong tim thay thu muc USERPROFILE."))?;
    if let Some(parent) = hooks_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Khong tao duoc thu muc config: {e}"))?;
    }
    let existing = fs::read_to_string(&hooks_path).ok();
    if let Some(prev) = &existing {
        let backup = hooks_path.with_extension("json.bak");
        let _ = fs::write(backup, prev);
    }
    let next = merge_anti_hook_into_json(existing.as_deref(), &exe_path)?;
    fs::write(&hooks_path, next).map_err(|e| format!("Khong ghi duoc hooks.json: {e}"))?;
    Ok(())
}

pub fn uninstall_anti_hooks() -> Result<(), String> {
    let hooks_path = global_hooks_json_path()
        .ok_or_else(|| String::from("Khong tim thay thu muc USERPROFILE."))?;
    let Ok(existing) = fs::read_to_string(&hooks_path) else {
        return Ok(());
    };
    let backup = hooks_path.with_extension("json.bak");
    let _ = fs::write(backup, &existing);
    let next = remove_anti_hook_from_json(&existing)?;
    fs::write(&hooks_path, next).map_err(|e| format!("Khong cap nhat duoc hooks.json: {e}"))?;
    Ok(())
}

struct SendHandle(HANDLE);
unsafe impl Send for SendHandle {}

struct HandleGuard(HANDLE);
impl Drop for HandleGuard {
    fn drop(&mut self) {
        if !self.0.is_invalid() && self.0 != HANDLE::default() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

pub fn sort_and_prune_anti_slots(slots: &mut Vec<AntiProgressSnapshot>, now_ms: u64) {
    slots.retain(|s| {
        if s.is_stale(now_ms) {
            return false;
        }
        if !s.state.is_running() && now_ms.saturating_sub(s.updated_unix_ms) > 25_000 {
            return false;
        }
        true
    });
    slots.sort_by(|a, b| {
        b.state
            .is_running()
            .cmp(&a.state.is_running())
            .then_with(|| b.updated_unix_ms.cmp(&a.updated_unix_ms))
    });
    if slots.len() > 4 {
        slots.truncate(4);
    }
}

pub fn upsert_anti_slot(
    slots: &mut Vec<AntiProgressSnapshot>,
    incoming: AntiProgressSnapshot,
    now_ms: u64,
) {
    let key = incoming.slot_key();
    if let Some(pos) = slots.iter().position(|s| s.slot_key() == key) {
        slots[pos] = incoming;
    } else if incoming.conversation_id.is_empty()
        && let Some(pos) = slots.iter().position(|s| s.source == incoming.source)
    {
        slots[pos] = incoming;
    } else {
        if incoming.state.is_running() {
            slots.retain(|s| !(s.source == incoming.source && !s.state.is_running()));
        }
        slots.push(incoming);
    }
    sort_and_prune_anti_slots(slots, now_ms);
}

pub struct AntiWatcher {
    stop_event: SendHandle,
    rx: Receiver<Vec<AntiProgressSnapshot>>,
    join: Option<JoinHandle<()>>,
}

impl AntiWatcher {
    pub fn start(hwnd: HWND) -> Self {
        let hwnd_raw = hwnd.0 as usize;
        let (tx, rx) = mpsc::channel::<Vec<AntiProgressSnapshot>>();
        let stop_handle = unsafe { CreateEventW(None, true, false, None) }.unwrap_or_default();
        let worker_stop = SendHandle(stop_handle);
        let dir_path = anti_progress_dir();
        let file_path = anti_progress_path();
        let brain_dir = ide_brain_dir();

        let join = thread::Builder::new()
            .name(String::from("anti-progress-watch"))
            .spawn(move || {
                run_anti_watch_loop(hwnd_raw, worker_stop, dir_path, file_path, brain_dir, tx);
            })
            .ok();

        Self {
            stop_event: SendHandle(stop_handle),
            rx,
            join,
        }
    }

    pub fn drain_latest(&self) -> Option<Vec<AntiProgressSnapshot>> {
        let mut latest = None;
        while let Ok(item) = self.rx.try_recv() {
            latest = Some(item);
        }
        latest
    }
}

impl Drop for AntiWatcher {
    fn drop(&mut self) {
        if !self.stop_event.0.is_invalid() && self.stop_event.0 != HANDLE::default() {
            unsafe {
                let _ = SetEvent(self.stop_event.0);
            }
        }
        if let Some(handle) = self.join.take() {
            let _ = handle.join();
        }
        if !self.stop_event.0.is_invalid() && self.stop_event.0 != HANDLE::default() {
            unsafe {
                let _ = CloseHandle(self.stop_event.0);
            }
        }
    }
}

fn post_anti_wake(hwnd_raw: usize) {
    let hwnd = HWND(hwnd_raw as *mut c_void);
    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_APP_ANTI_UPDATE, WPARAM(0), LPARAM(0));
    }
}

struct DirWatch {
    _dir_guard: HandleGuard,
    _io_guard: HandleGuard,
    dir_handle: HANDLE,
    io_event: HANDLE,
    buf: [u8; 8192],
    overlapped: windows::Win32::System::IO::OVERLAPPED,
    pending: bool,
    subtree: bool,
}

impl DirWatch {
    fn open(path: &Path, subtree: bool) -> Option<Self> {
        let wide_dir = to_wide_null(&path.to_string_lossy());
        let dir_raw = unsafe {
            CreateFileW(
                PCWSTR(wide_dir.as_ptr()),
                FILE_LIST_DIRECTORY.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS
                    | windows::Win32::Storage::FileSystem::FILE_FLAG_OVERLAPPED,
                None,
            )
        }
        .ok()?;
        if dir_raw == INVALID_HANDLE_VALUE {
            return None;
        }
        let dir_guard = HandleGuard(dir_raw);
        let io_event = unsafe { CreateEventW(None, true, false, None) }.unwrap_or_default();
        if io_event.is_invalid() || io_event == HANDLE::default() {
            return None;
        }
        let io_guard = HandleGuard(io_event);
        Some(Self {
            _dir_guard: dir_guard,
            _io_guard: io_guard,
            dir_handle: dir_raw,
            io_event,
            buf: [0_u8; 8192],
            overlapped: windows::Win32::System::IO::OVERLAPPED {
                hEvent: io_event,
                ..Default::default()
            },
            pending: false,
            subtree,
        })
    }

    fn arm(&mut self) -> bool {
        if self.pending {
            return true;
        }
        unsafe {
            let _ = ResetEvent(self.io_event);
        }
        let mut bytes_returned = 0_u32;
        let issued = unsafe {
            ReadDirectoryChangesW(
                self.dir_handle,
                self.buf.as_mut_ptr().cast(),
                self.buf.len() as u32,
                self.subtree,
                FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_LAST_WRITE,
                Some(&mut bytes_returned),
                Some(&mut self.overlapped),
                None,
            )
        };
        if issued.is_ok() {
            self.pending = true;
            true
        } else {
            false
        }
    }

    fn collect_changed_names(&mut self) -> Option<Vec<String>> {
        if !self.pending {
            return None;
        }
        self.pending = false;
        let mut got = 0_u32;
        let ok = unsafe {
            windows::Win32::System::IO::GetOverlappedResult(
                self.dir_handle,
                &self.overlapped,
                &mut got,
                false,
            )
        };
        if ok.is_err() || got == 0 {
            return None;
        }
        Some(extract_changed_filenames(&self.buf[..got as usize]))
    }
}

fn extract_changed_filenames(buf: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    let mut offset = 0_usize;
    while offset + size_of::<FILE_NOTIFY_INFORMATION>() <= buf.len() {
        let info = unsafe { &*(buf.as_ptr().add(offset) as *const FILE_NOTIFY_INFORMATION) };
        if matches!(
            info.Action,
            FILE_ACTION_ADDED | FILE_ACTION_MODIFIED | FILE_ACTION_RENAMED_NEW_NAME
        ) {
            let name_u16_len = info.FileNameLength as usize / 2;
            let name_ptr = info.FileName.as_ptr();
            let slice = unsafe { std::slice::from_raw_parts(name_ptr, name_u16_len) };
            names.push(String::from_utf16_lossy(slice));
        }
        if info.NextEntryOffset == 0 {
            break;
        }
        offset = offset.saturating_add(info.NextEntryOffset as usize);
    }
    names
}

fn collect_initial_slots(
    file: &Path,
    sessions_dir: Option<&Path>,
    brain_dir: Option<&Path>,
    now_ms: u64,
) -> Vec<AntiProgressSnapshot> {
    let mut slots = Vec::new();
    if let Some(sess_dir) = sessions_dir
        && let Ok(entries) = fs::read_dir(sess_dir)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json")
                && let Some(snap) = read_anti_progress(&path)
                && !snap.is_stale(now_ms)
            {
                upsert_anti_slot(&mut slots, snap, now_ms);
            }
        }
    }
    if slots.is_empty()
        && let Some(snap) = read_anti_progress(file)
        && !snap.is_stale(now_ms)
    {
        upsert_anti_slot(&mut slots, snap, now_ms);
    }
    if let Some(brain) = brain_dir
        && let Ok(entries) = fs::read_dir(brain)
    {
        let running = running_anti_exes();
        let source = detect_ide_source_from_exes(&running);
        if source != AntiSource::Antigravity {
            for entry in entries.flatten() {
                let conv_dir = entry.path();
                if !conv_dir.is_dir() {
                    continue;
                }
                let conv_name = conv_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if conv_name.is_empty() || conv_name.starts_with('.') {
                    continue;
                }
                let transcript = conv_dir
                    .join(".system_generated")
                    .join("logs")
                    .join(IDE_TRANSCRIPT_FILE_NAME);
                if let Ok(meta) = fs::metadata(&transcript)
                    && let Ok(modified) = meta.modified()
                    && let Ok(dur) = modified.duration_since(UNIX_EPOCH)
                    && now_ms.saturating_sub(dur.as_millis() as u64) <= IDE_STARTUP_RECENT_MS
                    && let Some(snap) =
                        parse_ide_transcript_tail(&transcript, &conv_name, source, now_ms)
                {
                    upsert_anti_slot(&mut slots, snap, now_ms);
                }
            }
        }
    }
    sort_and_prune_anti_slots(&mut slots, now_ms);
    slots
}

fn run_anti_watch_loop(
    hwnd_raw: usize,
    stop_event: SendHandle,
    dir_path: Option<PathBuf>,
    file_path: Option<PathBuf>,
    brain_dir: Option<PathBuf>,
    tx: mpsc::Sender<Vec<AntiProgressSnapshot>>,
) {
    let (Some(dir), Some(file)) = (dir_path, file_path) else {
        return;
    };
    let _ = fs::create_dir_all(&dir);
    let sess_dir = dir.join(ANTI_SESSIONS_DIR_NAME);
    let _ = fs::create_dir_all(&sess_dir);

    let init_now = now_unix_ms();
    let mut slots = collect_initial_slots(&file, Some(&sess_dir), brain_dir.as_deref(), init_now);
    let mut last_sent = slots.clone();
    if !last_sent.is_empty() {
        let _ = tx.send(last_sent.clone());
        post_anti_wake(hwnd_raw);
    }

    let Some(mut local_watch) = DirWatch::open(&dir, true) else {
        return;
    };
    let mut brain_watch = brain_dir
        .as_deref()
        .and_then(|b| if b.is_dir() { DirWatch::open(b, true) } else { None });

    loop {
        if !local_watch.arm() {
            thread::sleep(Duration::from_millis(250));
            continue;
        }
        let brain_armed = brain_watch.as_mut().is_some_and(DirWatch::arm);

        let mut wait_handles = Vec::with_capacity(3);
        wait_handles.push(stop_event.0);
        wait_handles.push(local_watch.io_event);
        if brain_armed && let Some(bw) = &brain_watch {
            wait_handles.push(bw.io_event);
        }

        let wait_res = unsafe { WaitForMultipleObjects(&wait_handles, false, u32::MAX) };
        if wait_res == WAIT_OBJECT_0 {
            break;
        }

        let now = now_unix_ms();
        let mut dirty = false;

        if wait_res.0 == WAIT_OBJECT_0.0 + 1
            && let Some(names) = local_watch.collect_changed_names()
        {
            for rel in names {
                let lower = rel.replace('/', "\\").to_ascii_lowercase();
                if lower == ANTI_PROGRESS_FILE_NAME
                    || lower.ends_with(&format!("\\{ANTI_PROGRESS_FILE_NAME}"))
                {
                    if let Some(snap) = read_anti_progress(&file).filter(|s| !s.is_stale(now)) {
                        upsert_anti_slot(&mut slots, snap, now);
                        dirty = true;
                    }
                } else if lower.contains(ANTI_SESSIONS_DIR_NAME) && lower.ends_with(".json") {
                    let full = dir.join(&rel);
                    if let Some(snap) = read_anti_progress(&full).filter(|s| !s.is_stale(now)) {
                        upsert_anti_slot(&mut slots, snap, now);
                        dirty = true;
                    }
                }
            }
        }

        if brain_armed
            && wait_res.0 == WAIT_OBJECT_0.0 + 2
            && let Some(bw) = brain_watch.as_mut()
            && let Some(names) = bw.collect_changed_names()
            && let Some(brain_root) = brain_dir.as_deref()
        {
            let running = running_anti_exes();
            let source = detect_ide_source_from_exes(&running);
            for rel in names {
                let norm = rel.replace('\\', "/");
                if !norm.to_ascii_lowercase().ends_with(IDE_TRANSCRIPT_FILE_NAME) {
                    continue;
                }
                let full = brain_root.join(&rel);
                let conv_id = extract_conv_id_from_brain_path(brain_root, &full);
                if let Some(snap) = parse_ide_transcript_tail(&full, &conv_id, source, now)
                    && !snap.is_stale(now)
                {
                    upsert_anti_slot(&mut slots, snap, now);
                    dirty = true;
                }
            }
        }

        if dirty {
            sort_and_prune_anti_slots(&mut slots, now);
            if slots != last_sent {
                last_sent = slots.clone();
                let _ = tx.send(last_sent.clone());
                post_anti_wake(hwnd_raw);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_all_ten_anti_states_across_cli_ide_app() {
        let cli_read = derive_anti_progress_from_hook(
            "SessionStart",
            r#"{"conversationId":"c1","transcriptPath":"C:\\Users\\ASUS\\.gemini\\antigravity-cli\\brain\\c1\\transcript.jsonl"}"#,
            None,
            1000,
        );
        assert_eq!(cli_read.source, AntiSource::AgyCli);
        assert_eq!(cli_read.state, AntiAgentState::ReadingProject);

        let ide_analyze = derive_anti_progress_from_hook(
            "PreInvocation",
            r#"{"conversationId":"c2","transcriptPath":"C:/Users/ASUS/.gemini/antigravity-ide/brain/c2/transcript.jsonl"}"#,
            None,
            2000,
        );
        assert_eq!(ide_analyze.source, AntiSource::AntiIde);
        assert_eq!(ide_analyze.state, AntiAgentState::Analyzing);

        let app_code = derive_anti_progress_from_hook(
            "PreToolUse",
            r#"{"conversationId":"c3","transcriptPath":"C:\\Users\\ASUS\\.gemini\\antigravity\\brain\\c3\\t.jsonl","toolCall":{"name":"write_file","args":{"Path":"src/anti.rs"}}}"#,
            None,
            3000,
        );
        assert_eq!(app_code.source, AntiSource::AntiApp);
        assert_eq!(app_code.state, AntiAgentState::Coding);

        let testing = derive_anti_progress_from_hook(
            "PreToolUse",
            r#"{"conversationId":"c3","toolCall":{"name":"run_command","args":{"CommandLine":"cargo test --locked"}}}"#,
            None,
            4000,
        );
        assert_eq!(testing.state, AntiAgentState::Testing);

        let building = derive_anti_progress_from_hook(
            "PreToolUse",
            r#"{"conversationId":"c3","toolCall":{"name":"run_command","args":{"CommandLine":"cargo build --release"}}}"#,
            None,
            5000,
        );
        assert_eq!(building.state, AntiAgentState::Building);

        let waiting = derive_anti_progress_from_hook(
            "PreToolUse",
            r#"{"conversationId":"c3","toolCall":{"name":"ask_question","args":{}}}"#,
            None,
            6000,
        );
        assert_eq!(waiting.state, AntiAgentState::WaitingApproval);

        let err = derive_anti_progress_from_hook(
            "PostToolUse",
            r#"{"conversationId":"c3","error":"exit status 1","toolCall":{"name":"run_command","args":{"CommandLine":"cargo test"}}}"#,
            None,
            7000,
        );
        assert_eq!(err.state, AntiAgentState::Error);
        assert!(err.had_error);

        let fixing = derive_anti_progress_from_hook(
            "PreInvocation",
            r#"{"conversationId":"c3"}"#,
            Some(&err),
            8000,
        );
        assert_eq!(fixing.state, AntiAgentState::FixingError);

        let canceled = derive_anti_progress_from_hook(
            "Stop",
            r#"{"conversationId":"c3","terminationReason":"USER_CANCELED"}"#,
            None,
            9000,
        );
        assert_eq!(canceled.state, AntiAgentState::Canceled);

        let done = derive_anti_progress_from_hook(
            "Stop",
            r#"{"conversationId":"c3","terminationReason":"NO_TOOL_CALL","fullyIdle":true}"#,
            None,
            10_000,
        );
        assert_eq!(done.state, AntiAgentState::Done);
    }

    #[test]
    fn anti_progress_json_roundtrip_and_hook_merge_preserve_existing_hooks() {
        let snap = AntiProgressSnapshot {
            source: AntiSource::AgyCli,
            conversation_id: String::from("conv-123"),
            state: AntiAgentState::Building,
            detail: String::from("cargo build --release"),
            recent_steps: vec![
                String::from("Đang build: cargo build --release"),
                String::from("Đang code: Sửa anti.rs"),
            ],
            had_error: false,
            updated_unix_ms: 1_795_836_400_000,
        };
        let raw = snap.to_json();
        let parsed = AntiProgressSnapshot::from_json(&raw).expect("parse anti progress json");
        assert_eq!(parsed, snap);

        let legacy = r#"{"source":"anti-ide","conversation_id":"c0","state":"coding","detail":"Sửa main.rs","had_error":false,"updated_unix_ms":10}"#;
        let parsed_legacy = AntiProgressSnapshot::from_json(legacy).expect("legacy json");
        assert_eq!(parsed_legacy.source, AntiSource::AntiIde);
        assert!(parsed_legacy.recent_steps.is_empty());

        let existing = r#"{
  "my-custom-lint": {
    "Stop": [{"type": "command", "command": "echo ok"}]
  }
}"#;
        let exe = Path::new(r"C:\Tools\win-dynamic-island.exe");
        let merged = merge_anti_hook_into_json(Some(existing), exe).expect("merge hooks");
        assert!(merged.contains("\"my-custom-lint\""));
        assert!(merged.contains("\"win-dynamic-island-status\""));

        let removed = remove_anti_hook_from_json(&merged).expect("remove hooks");
        assert!(removed.contains("\"my-custom-lint\""));
        assert!(!removed.contains("\"win-dynamic-island-status\""));
    }

    #[test]
    fn classifies_anti_sources_independent_of_install_drive_and_summarizes_commands() {
        let empty_env = AntiEnvHints {
            ls_version: None,
            jetski_app_data_dir: None,
            agentapi_exe: None,
            vscode_pid_exe: None,
        };
        let shared_path = Some(r"C:\Users\Bob\.gemini\antigravity\brain\123\transcript.jsonl");

        // Cài Anti IDE ở bất kỳ ổ/thư mục nào (ví dụ D:\Tools\Custom\Antigravity IDE.exe) nhưng dùng chung .gemini/antigravity/
        let ide_src = classify_anti_source_inputs(
            &empty_env,
            &[String::from("cmd.exe"), String::from("Antigravity IDE.exe")],
            &[],
            shared_path,
            None,
        );
        assert_eq!(ide_src, AntiSource::AntiIde);

        // Anti App chạy từ Antigravity.exe
        let app_src = classify_anti_source_inputs(
            &empty_env,
            &[String::from("Antigravity.exe")],
            &[],
            shared_path,
            None,
        );
        assert_eq!(app_src, AntiSource::AntiApp);

        // agy CLI qua env phiên dù không có đường dẫn antigravity-cli/
        let cli_env = AntiEnvHints {
            ls_version: Some("cli-1.5.18"),
            jetski_app_data_dir: Some("antigravity-cli"),
            agentapi_exe: Some(r"E:\Portable\agy.exe"),
            vscode_pid_exe: None,
        };
        assert_eq!(
            classify_anti_source_inputs(&cli_env, &[], &[], shared_path, None),
            AntiSource::AgyCli
        );

        // Cây cha bị ngắt khi dùng chung .gemini/antigravity/, chỉ có Antigravity IDE.exe đang chạy
        assert_eq!(
            classify_anti_source_inputs(
                &empty_env,
                &[],
                &[String::from("Antigravity IDE.exe")],
                shared_path,
                None,
            ),
            AntiSource::AntiIde
        );

        // Rút gọn lệnh PowerShell thô & ưu tiên toolSummary
        assert_eq!(
            summarize_command_hint(r#"$winSrc = "C:\Users\ASUS\.cargo\registry""#),
            "Truy vấn PowerShell"
        );
        let (st, hint) = classify_tool_call(
            "run_command",
            r#"{"CommandLine":"$winSrc = 'C:\\x'","toolSummary":"Inspect device APIs"}"#,
            false,
        );
        assert_eq!(st, AntiAgentState::Analyzing);
        assert_eq!(hint, "Inspect device APIs");
    }

    #[test]
    fn infer_state_from_transcript_tail_reads_only_tail_on_large_files() {
        let dir = std::env::temp_dir().join(format!("wdi_anti_tail_{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("transcript.jsonl");

        let filler = r#"{"type":"USER_INPUT","text":"hello filler line padding bytes 0123456789abcdef"}"#;
        let mut content = String::with_capacity(200 * 1024);
        while content.len() < 180 * 1024 {
            content.push_str(filler);
            content.push('\n');
        }
        content.push_str(
            r#"{"type":"PLANNER_RESPONSE","tool_calls":[{"name":"run_command","args":{"CommandLine":"cargo test --locked"}}]}"#,
        );
        content.push('\n');
        fs::write(&path, content.as_bytes()).expect("write large transcript");

        let inferred = infer_state_from_transcript_tail(&path, false);
        assert_eq!(
            inferred.as_ref().map(|(s, _)| *s),
            Some(AntiAgentState::Testing)
        );

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn parses_ide_transcript_and_ranks_dual_anti_slots() {
        let dir = std::env::temp_dir().join(format!("wdi_ide_transcript_{}", std::process::id()));
        let brain = dir.join("brain");
        let log_dir = brain.join("conv-ide-1").join(".system_generated").join("logs");
        let _ = fs::create_dir_all(&log_dir);
        let path = log_dir.join("transcript.jsonl");

        assert_eq!(
            extract_conv_id_from_brain_path(&brain, &path),
            "conv-ide-1"
        );
        assert_eq!(
            detect_ide_source_from_exes(&[String::from("Antigravity IDE.exe")]),
            AntiSource::AntiIde
        );

        let running_content = concat!(
            r#"{"step_index":0,"source":"USER_EXPLICIT","type":"USER_INPUT","status":"DONE"}"#,
            "\n",
            r#"{"step_index":1,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","tool_calls":[{"name":"write_file","args":{"Path":"src/anti.rs"}}]}"#,
            "\n"
        );
        fs::write(&path, running_content.as_bytes()).expect("write ide running transcript");
        let running_snap = parse_ide_transcript_tail(&path, "conv-ide-1", AntiSource::AntiIde, 5000)
            .expect("parse running ide transcript");
        assert_eq!(running_snap.source, AntiSource::AntiIde);
        assert_eq!(running_snap.state, AntiAgentState::Coding);
        assert_eq!(running_snap.detail, "Sửa anti.rs");

        let done_content = format!(
            "{running_content}{}\n",
            r#"{"step_index":2,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","content":"Đã xong mọi bước"}"#
        );
        fs::write(&path, done_content.as_bytes()).expect("write ide done transcript");
        let done_snap = parse_ide_transcript_tail(&path, "conv-ide-1", AntiSource::AntiIde, 6000)
            .expect("parse done ide transcript");
        assert_eq!(done_snap.state, AntiAgentState::Done);

        let mut slots = Vec::new();
        let cli_done = AntiProgressSnapshot {
            source: AntiSource::AgyCli,
            conversation_id: String::from("cli-1"),
            state: AntiAgentState::Done,
            detail: String::from("Hoàn tất"),
            recent_steps: Vec::new(),
            had_error: false,
            updated_unix_ms: 9000,
        };
        let mut ide_coding = running_snap;
        ide_coding.updated_unix_ms = 8000;
        upsert_anti_slot(&mut slots, cli_done, 9100);
        upsert_anti_slot(&mut slots, ide_coding, 9100);
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].source, AntiSource::AntiIde);
        assert_eq!(slots[1].source, AntiSource::AgyCli);

        let entries = vec![
            (12024_u32, 1000_u32, String::from("explorer.exe")),
            (3616_u32, 12024_u32, String::from("WindowsTerminal.exe")),
            (8964_u32, 3616_u32, String::from("powershell.exe")),
            (3196_u32, 8964_u32, String::from("agy.exe")),
            (21204_u32, 12024_u32, String::from("Antigravity IDE.exe")),
            (3444_u32, 21204_u32, String::from("Antigravity IDE.exe")),
            (8008_u32, 3444_u32, String::from("agy.exe")),
        ];
        let cli_pids = collect_ancestor_pids_from_entries(&entries, AntiSource::AgyCli);
        assert!(cli_pids.contains(&3196));
        assert!(cli_pids.contains(&8964));
        assert!(cli_pids.contains(&3616));
        assert!(!cli_pids.contains(&8008));
        assert!(!cli_pids.contains(&3444));
        assert!(!cli_pids.contains(&12024));
        assert!(!cli_pids.contains(&21204));

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(&dir);
    }
}
