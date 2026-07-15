use serde_json::Value;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::utils::{extract_text, parse_timestamp_to_ms, read_head_tail_lines, truncate_summary};
use crate::session_manager::{SessionMessage, SessionMeta};

const PROVIDER_ID: &str = "openclaw";
const MAX_MESSAGE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_SESSIONS: usize = 5000;

pub fn get_agents_root() -> PathBuf {
    crate::openclaw_config::get_openclaw_dir().join("agents")
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    let root = get_agents_root();
    scan_sessions_at_root(&root)
}

fn scan_sessions_at_root(root: &Path) -> Vec<SessionMeta> {
    let Ok(root_meta) = fs::symlink_metadata(root) else {
        return Vec::new();
    };
    if root_meta.file_type().is_symlink() || !root_meta.is_dir() {
        return Vec::new();
    }

    let mut sessions = Vec::new();
    let Ok(agents) = fs::read_dir(root) else {
        return sessions;
    };
    'agents: for agent in agents.filter_map(Result::ok) {
        let Ok(agent_meta) = agent.metadata() else {
            continue;
        };
        if !agent_meta.is_dir()
            || fs::symlink_metadata(agent.path()).is_ok_and(|meta| meta.file_type().is_symlink())
        {
            continue;
        }
        let agent_id = agent.file_name().to_string_lossy().to_string();
        let session_dir = agent.path().join("sessions");
        let Ok(dir_meta) = fs::symlink_metadata(&session_dir) else {
            continue;
        };
        if dir_meta.file_type().is_symlink() || !dir_meta.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&session_dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            if sessions.len() >= MAX_SESSIONS {
                break 'agents;
            }
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
                continue;
            }
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                continue;
            }
            if let Some(session) = parse_session_meta(&path, &agent_id) {
                sessions.push(session);
            }
        }
    }
    sessions
}

fn parse_session_meta(path: &Path, agent_id: &str) -> Option<SessionMeta> {
    let session_id = path.file_stem()?.to_string_lossy().to_string();
    if session_id.is_empty() {
        return None;
    }
    let (head, tail) = read_head_tail_lines(path, 160, 80).ok()?;
    let mut title = None;
    let mut summary = None;
    let mut project_dir = None;
    let mut created_at = None;
    let mut last_active_at = None;

    for line in head.iter().chain(tail.iter()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let timestamp = extract_timestamp(&value);
        if let Some(timestamp) = timestamp {
            created_at = Some(created_at.map_or(timestamp, |current: i64| current.min(timestamp)));
            last_active_at =
                Some(last_active_at.map_or(timestamp, |current: i64| current.max(timestamp)));
        }
        if project_dir.is_none() {
            project_dir = extract_project_dir(&value);
        }
        if summary.is_none() {
            summary = value
                .get("summary")
                .or_else(|| value.pointer("/session/summary"))
                .and_then(Value::as_str)
                .map(|text| truncate_summary(text, 180))
                .filter(|text| !text.is_empty());
        }
        if title.is_none() {
            let (role, content) = extract_message(&value);
            if role.as_deref() == Some("user") {
                title = content
                    .map(|text| truncate_summary(&text, 80))
                    .filter(|text| !text.is_empty());
            }
        }
    }

    if let Ok(metadata) = fs::metadata(path) {
        let modified = metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_millis() as i64);
        last_active_at = last_active_at.or(modified);
        created_at = created_at.or(modified);
    }

    Some(SessionMeta {
        provider_id: PROVIDER_ID.to_string(),
        session_id: session_id.clone(),
        title: title.or_else(|| Some(format!("{agent_id} / {session_id}"))),
        summary: summary.or_else(|| Some(format!("OpenClaw agent: {agent_id}"))),
        project_dir,
        created_at,
        last_active_at,
        source_path: Some(path.to_string_lossy().to_string()),
        resume_command: None,
    })
}

pub fn load_messages(path: &Path) -> Result<Vec<SessionMessage>, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open OpenClaw session: {e}"))?;
    let reader = BufReader::new(file.take(MAX_MESSAGE_BYTES));
    let mut messages = Vec::new();
    for line in reader.lines().map_while(Result::ok) {
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let (Some(role), Some(content)) = extract_message(&value) else {
            continue;
        };
        let content = content.trim().to_string();
        if content.is_empty() {
            continue;
        }
        messages.push(SessionMessage {
            role,
            content,
            ts: extract_timestamp(&value),
        });
    }
    Ok(messages)
}

pub fn delete_session(root: &Path, source_path: &Path, session_id: &str) -> Result<bool, String> {
    if source_path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
        return Err("OpenClaw session source must be a JSONL file".to_string());
    }
    let file_id = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "OpenClaw session filename is invalid".to_string())?;
    if session_id != file_id {
        return Err(format!(
            "OpenClaw session path does not match session ID: expected {session_id}, found {file_id}"
        ));
    }
    let parent = source_path
        .parent()
        .ok_or_else(|| "OpenClaw session has no parent directory".to_string())?;
    let agent_dir = parent
        .parent()
        .ok_or_else(|| "OpenClaw session has no agent directory".to_string())?;
    let agents_dir = agent_dir
        .parent()
        .ok_or_else(|| "OpenClaw session has no agents directory".to_string())?;
    if parent.file_name().and_then(|value| value.to_str()) != Some("sessions")
        || agents_dir != root
        || agent_dir.file_name().is_none()
    {
        return Err("OpenClaw session path is outside an agent sessions directory".to_string());
    }
    fs::remove_file(source_path)
        .map(|_| true)
        .map_err(|e| format!("Failed to delete OpenClaw session: {e}"))
}

fn extract_message(value: &Value) -> (Option<String>, Option<String>) {
    let message = value.get("message").unwrap_or(value);
    let role = message
        .get("role")
        .or_else(|| value.get("role"))
        .and_then(Value::as_str)
        .map(|role| role.to_ascii_lowercase());
    let content = message
        .get("content")
        .or_else(|| message.get("text"))
        .or_else(|| value.get("content"))
        .map(extract_text)
        .filter(|text| !text.trim().is_empty());
    (role, content)
}

fn extract_timestamp(value: &Value) -> Option<i64> {
    [
        value.get("timestamp"),
        value.get("ts"),
        value.pointer("/message/timestamp"),
        value.pointer("/session/timestamp"),
    ]
    .into_iter()
    .flatten()
    .find_map(parse_timestamp_to_ms)
}

fn extract_project_dir(value: &Value) -> Option<String> {
    [
        value.get("cwd"),
        value.get("projectDir"),
        value.get("project_dir"),
        value.pointer("/context/cwd"),
        value.pointer("/session/cwd"),
    ]
    .into_iter()
    .flatten()
    .find_map(Value::as_str)
    .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_nested_openclaw_message() {
        let value = json!({
            "timestamp": "2026-07-12T10:00:00Z",
            "message": {
                "role": "user",
                "content": [{"type": "text", "text": "hello"}]
            }
        });
        let (role, content) = extract_message(&value);
        assert_eq!(role.as_deref(), Some("user"));
        assert_eq!(content.as_deref(), Some("hello"));
        assert!(extract_timestamp(&value).is_some());
    }

    #[test]
    fn rejects_sessions_nested_below_an_agent_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("agents");
        let path = root.join("agent-1").join("nested").join("sessions");
        fs::create_dir_all(&path).expect("create nested sessions");
        let file = path.join("session-1.jsonl");
        fs::write(&file, "{}").expect("write session");

        let error = delete_session(&root, &file, "session-1")
            .expect_err("nested session directory must be rejected");
        assert!(error.contains("outside an agent sessions directory"));
        assert!(file.exists());
    }

    #[test]
    fn deletes_only_exact_agent_session_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("agents");
        let path = root.join("agent-1").join("sessions");
        fs::create_dir_all(&path).expect("create sessions");
        let file = path.join("session-1.jsonl");
        fs::write(&file, "{}").expect("write session");

        assert!(delete_session(&root, &file, "session-1").expect("delete session"));
        assert!(!file.exists());
    }

    #[cfg(unix)]
    #[test]
    fn scan_ignores_symlinked_agents_directories_and_files() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("agents");
        let valid_sessions = root.join("valid-agent").join("sessions");
        fs::create_dir_all(&valid_sessions).expect("create valid sessions");
        fs::write(
            valid_sessions.join("valid.jsonl"),
            r#"{"message":{"role":"user","content":"valid"}}"#,
        )
        .expect("write valid session");

        let outside = tempfile::tempdir().expect("outside tempdir");
        let outside_sessions = outside.path().join("sessions");
        fs::create_dir_all(&outside_sessions).expect("create outside sessions");
        let outside_file = outside_sessions.join("outside.jsonl");
        fs::write(
            &outside_file,
            r#"{"message":{"role":"user","content":"outside"}}"#,
        )
        .expect("write outside session");

        symlink(outside.path(), root.join("linked-agent")).expect("link agent directory");
        let linked_sessions_agent = root.join("linked-sessions-agent");
        fs::create_dir_all(&linked_sessions_agent).expect("create linked sessions agent");
        symlink(&outside_sessions, linked_sessions_agent.join("sessions"))
            .expect("link sessions directory");
        let linked_file_sessions = root.join("linked-file-agent").join("sessions");
        fs::create_dir_all(&linked_file_sessions).expect("create linked file sessions");
        symlink(&outside_file, linked_file_sessions.join("linked.jsonl"))
            .expect("link session file");

        let sessions = scan_sessions_at_root(&root);

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "valid");
        assert!(sessions[0]
            .source_path
            .as_deref()
            .is_some_and(|path| path.ends_with("valid.jsonl")));
    }
}
