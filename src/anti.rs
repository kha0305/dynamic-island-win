use std::ffi::c_void;
use std::fs;
use std::io::Read;
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
use windows::Win32::System::Threading::{CreateEventW, ResetEvent, SetEvent, WaitForMultipleObjects};
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
/// Nếu trạng thái đang chạy không cập nhật quá 6 phút thì coi như phiên đã kết thúc/treo.
const STALE_RUNNING_MS: u64 = 360_000;

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
    pub had_error: bool,
    pub updated_unix_ms: u64,
}

impl AntiProgressSnapshot {
    pub fn is_stale(&self, now_ms: u64) -> bool {
        self.state.is_running() && now_ms.saturating_sub(self.updated_unix_ms) > STALE_RUNNING_MS
    }

    pub fn to_json(&self) -> String {
        format!(
            "{{\n  \"source\": \"{}\",\n  \"conversation_id\": \"{}\",\n  \"state\": \"{}\",\n  \"label\": \"{}\",\n  \"detail\": \"{}\",\n  \"active\": {},\n  \"had_error\": {},\n  \"updated_unix_ms\": {}\n}}\n",
            self.source.as_key(),
            escape_json_string(&self.conversation_id),
            self.state.as_key(),
            escape_json_string(self.state.label_vi()),
            escape_json_string(&self.detail),
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
        let had_error = extract_json_bool_field_top_level(raw, "had_error").unwrap_or(false);
        let updated_unix_ms =
            extract_json_u64_field_top_level(raw, "updated_unix_ms").unwrap_or(0);
        Some(Self {
            source,
            conversation_id,
            state,
            detail,
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

fn sanitize_detail(raw: &str) -> String {
    let compact: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut out = String::new();
    for ch in compact.chars().take(48) {
        out.push(ch);
    }
    out
}

fn classify_tool_call(
    tool_name: &str,
    tool_args_json: &str,
    had_prev_error: bool,
) -> (AntiAgentState, String) {
    let name = tool_name.trim().to_ascii_lowercase();
    if name.contains("ask_question") || name.contains("request_feedback") {
        return (AntiAgentState::WaitingApproval, String::from("Chờ xác nhận"));
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
        return (st, sanitize_detail(&file_hint));
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
        return (AntiAgentState::ReadingProject, sanitize_detail(&hint));
    }
    if name == "run_command" || name == "command_status" {
        let cmd = extract_json_string_field_top_level(tool_args_json, "CommandLine")
            .or_else(|| extract_json_string_field_top_level(tool_args_json, "toolSummary"))
            .unwrap_or_default();
        let lower = cmd.to_ascii_lowercase();
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
        return (st, sanitize_detail(&cmd));
    }
    if had_prev_error {
        return (AntiAgentState::FixingError, sanitize_detail(tool_name));
    }
    (AntiAgentState::Analyzing, sanitize_detail(tool_name))
}

/// Đọc dòng cuối của `transcript.jsonl` (nếu có) để nắm ngay tool đang chuẩn bị chạy mà không cần PreToolUse hook.
fn infer_state_from_transcript_tail(
    transcript_path: &Path,
    had_prev_error: bool,
) -> Option<(AntiAgentState, String)> {
    let text = fs::read_to_string(transcript_path).ok()?;
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

pub fn derive_anti_progress_from_hook(
    event_name: &str,
    stdin_json: &str,
    prev: Option<&AntiProgressSnapshot>,
    now_ms: u64,
) -> AntiProgressSnapshot {
    let conv_id = extract_json_string_field_top_level(stdin_json, "conversationId")
        .unwrap_or_default();
    let transcript = extract_json_string_field_top_level(stdin_json, "transcriptPath");
    let artifact_dir = extract_json_string_field_top_level(stdin_json, "artifactDirectoryPath");
    let source = detect_source_from_paths(transcript.as_deref(), artifact_dir.as_deref());
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

    AntiProgressSnapshot {
        source,
        conversation_id: conv_id,
        state,
        detail,
        had_error,
        updated_unix_ms: now_ms,
    }
}

/// Chạy khi binary được gọi với `--anti-hook <EVENT>` từ `~/.gemini/config/hooks.json`.
pub fn run_anti_hook_cli(event_name: &str) -> i32 {
    let mut stdin_buf = String::new();
    let _ = std::io::stdin().read_to_string(&mut stdin_buf);
    if let Some(path) = anti_progress_path() {
        let prev = read_anti_progress(&path);
        let snap = derive_anti_progress_from_hook(event_name, &stdin_buf, prev.as_ref(), now_unix_ms());
        let _ = write_anti_progress_atomic(&path, &snap);
    }
    println!("{{}}");
    0
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

pub struct AntiWatcher {
    stop_event: SendHandle,
    rx: Receiver<Option<AntiProgressSnapshot>>,
    join: Option<JoinHandle<()>>,
}

impl AntiWatcher {
    pub fn start(hwnd: HWND) -> Self {
        let hwnd_raw = hwnd.0 as usize;
        let (tx, rx) = mpsc::channel::<Option<AntiProgressSnapshot>>();
        let stop_handle = unsafe { CreateEventW(None, true, false, None) }.unwrap_or_default();
        let worker_stop = SendHandle(stop_handle);
        let dir_path = anti_progress_dir();
        let file_path = anti_progress_path();

        let join = thread::Builder::new()
            .name(String::from("anti-progress-watch"))
            .spawn(move || {
                run_anti_watch_loop(hwnd_raw, worker_stop, dir_path, file_path, tx);
            })
            .ok();

        Self {
            stop_event: SendHandle(stop_handle),
            rx,
            join,
        }
    }

    pub fn drain_latest(&self) -> Option<Option<AntiProgressSnapshot>> {
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

fn run_anti_watch_loop(
    hwnd_raw: usize,
    stop_event: SendHandle,
    dir_path: Option<PathBuf>,
    file_path: Option<PathBuf>,
    tx: mpsc::Sender<Option<AntiProgressSnapshot>>,
) {
    let (Some(dir), Some(file)) = (dir_path, file_path) else {
        return;
    };
    let _ = fs::create_dir_all(&dir);

    let mut last_sent = read_anti_progress(&file).filter(|s| !s.is_stale(now_unix_ms()));
    if last_sent.is_some() {
        let _ = tx.send(last_sent.clone());
        post_anti_wake(hwnd_raw);
    }

    let wide_dir = to_wide_null(&dir.to_string_lossy());
    let dir_handle = unsafe {
        CreateFileW(
            PCWSTR(wide_dir.as_ptr()),
            FILE_LIST_DIRECTORY.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | windows::Win32::Storage::FileSystem::FILE_FLAG_OVERLAPPED,
            None,
        )
    };
    let Ok(dir_raw) = dir_handle else {
        return;
    };
    if dir_raw == INVALID_HANDLE_VALUE {
        return;
    }
    let _dir_guard = HandleGuard(dir_raw);

    let io_event = unsafe { CreateEventW(None, true, false, None) }.unwrap_or_default();
    if io_event.is_invalid() || io_event == HANDLE::default() {
        return;
    }
    let _io_guard = HandleGuard(io_event);

    let mut buf = [0_u8; 4096];
    let mut overlapped = windows::Win32::System::IO::OVERLAPPED {
        hEvent: io_event,
        ..Default::default()
    };

    loop {
        unsafe {
            let _ = ResetEvent(io_event);
        }
        let mut bytes_returned = 0_u32;
        let issued = unsafe {
            ReadDirectoryChangesW(
                dir_raw,
                buf.as_mut_ptr().cast(),
                buf.len() as u32,
                false,
                FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_LAST_WRITE,
                Some(&mut bytes_returned),
                Some(&mut overlapped),
                None,
            )
        };
        if issued.is_err() {
            thread::sleep(Duration::from_millis(250));
            continue;
        }

        let wait_handles = [stop_event.0, io_event];
        let wait_res = unsafe { WaitForMultipleObjects(&wait_handles, false, u32::MAX) };
        if wait_res == WAIT_OBJECT_0 {
            break;
        }

        let mut got = 0_u32;
        let ok = unsafe {
            windows::Win32::System::IO::GetOverlappedResult(dir_raw, &overlapped, &mut got, false)
        };
        if ok.is_err() || got == 0 {
            continue;
        }

        if buffer_touches_progress_file(&buf[..got as usize]) {
            let next = read_anti_progress(&file).filter(|s| !s.is_stale(now_unix_ms()));
            if next != last_sent {
                last_sent = next.clone();
                let _ = tx.send(next);
                post_anti_wake(hwnd_raw);
            }
        }
    }
}

fn buffer_touches_progress_file(buf: &[u8]) -> bool {
    let mut offset = 0_usize;
    while offset + size_of::<FILE_NOTIFY_INFORMATION>() <= buf.len() {
        let info = unsafe { &*(buf.as_ptr().add(offset) as *const FILE_NOTIFY_INFORMATION) };
        if matches!(
            info.Action,
            FILE_ACTION_ADDED | FILE_ACTION_MODIFIED | FILE_ACTION_RENAMED_NEW_NAME
        ) {
            let name_bytes = info.FileNameLength as usize;
            let name_u16_len = name_bytes / 2;
            let name_ptr = info.FileName.as_ptr();
            let slice = unsafe { std::slice::from_raw_parts(name_ptr, name_u16_len) };
            let filename = String::from_utf16_lossy(slice);
            if filename.eq_ignore_ascii_case(ANTI_PROGRESS_FILE_NAME) {
                return true;
            }
        }
        if info.NextEntryOffset == 0 {
            break;
        }
        offset = offset.saturating_add(info.NextEntryOffset as usize);
    }
    false
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
            had_error: false,
            updated_unix_ms: 1_795_836_400_000,
        };
        let raw = snap.to_json();
        let parsed = AntiProgressSnapshot::from_json(&raw).expect("parse anti progress json");
        assert_eq!(parsed, snap);

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
}
