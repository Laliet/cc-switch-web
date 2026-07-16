use chrono::NaiveDate;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{atomic_write, get_app_config_dir};
use crate::openclaw_config::get_openclaw_workspace_dir;

pub const ALLOWED_FILES: &[&str] = &[
    "AGENTS.md",
    "SOUL.md",
    "USER.md",
    "IDENTITY.md",
    "TOOLS.md",
    "MEMORY.md",
    "HEARTBEAT.md",
    "BOOTSTRAP.md",
    "BOOT.md",
];

const MAX_FILE_BYTES: usize = 1024 * 1024;
const MAX_BACKUPS_PER_FILE: usize = 20;

#[derive(Debug)]
pub enum WorkspaceError {
    InvalidInput(String),
    NotFound(String),
    Conflict(String),
    TooLarge(String),
    Io(String),
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message)
            | Self::NotFound(message)
            | Self::Conflict(message)
            | Self::TooLarge(message)
            | Self::Io(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for WorkspaceError {}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFileInfo {
    pub name: String,
    pub exists: bool,
    pub size_bytes: u64,
    pub modified_at: Option<u64>,
    pub etag: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFileContent {
    pub name: String,
    pub content: String,
    pub size_bytes: u64,
    pub modified_at: u64,
    pub etag: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceWriteOutcome {
    pub name: String,
    pub size_bytes: u64,
    pub modified_at: u64,
    pub etag: String,
    pub backup_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceBackupInfo {
    pub id: String,
    pub size_bytes: u64,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyMemoryInfo {
    pub date: String,
    pub size_bytes: u64,
    pub modified_at: u64,
    pub etag: String,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyMemorySearchResult {
    pub date: String,
    pub size_bytes: u64,
    pub modified_at: u64,
    pub etag: String,
    pub snippet: String,
    pub match_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyMemoryDeleteOutcome {
    pub date: String,
    pub deleted: bool,
    pub backup_id: Option<String>,
}

pub fn list_files() -> Result<Vec<WorkspaceFileInfo>, WorkspaceError> {
    let root = workspace_root(false)?;
    ALLOWED_FILES
        .iter()
        .map(|name| {
            let path = root.join(name);
            if !path.exists() {
                return Ok(WorkspaceFileInfo {
                    name: (*name).to_string(),
                    exists: false,
                    size_bytes: 0,
                    modified_at: None,
                    etag: None,
                });
            }
            let bytes = read_file_bytes(&root, &path)?;
            let metadata = fs::metadata(&path).map_err(|e| io_error("read metadata", &path, e))?;
            Ok(WorkspaceFileInfo {
                name: (*name).to_string(),
                exists: true,
                size_bytes: metadata.len(),
                modified_at: Some(modified_at(&metadata)),
                etag: Some(etag(&bytes)),
            })
        })
        .collect()
}

pub fn read_file(name: &str) -> Result<WorkspaceFileContent, WorkspaceError> {
    validate_filename(name)?;
    let root = workspace_root(false)?;
    read_content(&root, &root.join(name), name)
}

pub fn write_file(
    name: &str,
    content: &str,
    expected_etag: Option<&str>,
) -> Result<WorkspaceWriteOutcome, WorkspaceError> {
    validate_filename(name)?;
    write_named_file(
        &workspace_root(true)?,
        Path::new(name),
        name,
        content,
        expected_etag,
    )
}

pub fn list_backups(name: &str) -> Result<Vec<WorkspaceBackupInfo>, WorkspaceError> {
    validate_filename(name)?;
    let dir = backup_dir(name)?;
    if !validate_optional_backup_directory(&dir)? {
        return Ok(Vec::new());
    }
    let mut backups = fs::read_dir(&dir)
        .map_err(|e| io_error("list backups", &dir, e))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            if file_type.is_symlink() || !file_type.is_file() {
                return None;
            }
            let id = entry.file_name().to_string_lossy().to_string();
            if validate_backup_id(&id).is_err() {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            if metadata.len() > MAX_FILE_BYTES as u64 {
                return None;
            }
            Some(WorkspaceBackupInfo {
                id,
                size_bytes: metadata.len(),
                created_at: modified_at(&metadata),
            })
        })
        .collect::<Vec<_>>();
    backups.sort_by_key(|backup| std::cmp::Reverse(backup.created_at));
    Ok(backups)
}

pub fn restore_backup(
    name: &str,
    backup_id: &str,
    expected_etag: Option<&str>,
) -> Result<WorkspaceWriteOutcome, WorkspaceError> {
    validate_filename(name)?;
    validate_backup_id(backup_id)?;
    let path = backup_dir(name)?.join(backup_id);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(WorkspaceError::NotFound(format!(
                "Workspace backup not found: {backup_id}"
            )));
        }
        Err(error) => return Err(io_error("read backup metadata", &path, error)),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(WorkspaceError::InvalidInput(
            "Workspace backup must be a regular file".to_string(),
        ));
    }
    let bytes = fs::read(&path).map_err(|e| io_error("read backup", &path, e))?;
    check_size(bytes.len())?;
    let content = String::from_utf8(bytes).map_err(|_| {
        WorkspaceError::InvalidInput("Workspace backup is not valid UTF-8".to_string())
    })?;
    write_file(name, &content, expected_etag)
}

pub fn list_daily_memory() -> Result<Vec<DailyMemoryInfo>, WorkspaceError> {
    let root = workspace_root(false)?;
    let memory = root.join("memory");
    if !memory.exists() {
        return Ok(Vec::new());
    }
    validate_directory(&root, &memory)?;
    let mut result = Vec::new();
    for entry in fs::read_dir(&memory).map_err(|e| io_error("list daily memory", &memory, e))? {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(date) = name.strip_suffix(".md") else {
            continue;
        };
        if validate_date(date).is_err() {
            continue;
        }
        let path = entry.path();
        let bytes = match read_file_bytes(&memory, &path) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        let Ok(content) = std::str::from_utf8(&bytes) else {
            continue;
        };
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        result.push(DailyMemoryInfo {
            date: date.to_string(),
            size_bytes: metadata.len(),
            modified_at: modified_at(&metadata),
            etag: etag(&bytes),
            preview: content.chars().take(200).collect(),
        });
    }
    result.sort_by(|left, right| right.date.cmp(&left.date));
    Ok(result)
}

pub fn read_daily_memory(date: &str) -> Result<WorkspaceFileContent, WorkspaceError> {
    validate_date(date)?;
    let root = workspace_root(false)?;
    let memory = root.join("memory");
    validate_directory(&root, &memory)?;
    let name = format!("{date}.md");
    read_content(&memory, &memory.join(&name), &name)
}

pub fn write_daily_memory(
    date: &str,
    content: &str,
    expected_etag: Option<&str>,
) -> Result<WorkspaceWriteOutcome, WorkspaceError> {
    validate_date(date)?;
    let root = workspace_root(true)?;
    let memory = root.join("memory");
    ensure_directory(&root, &memory)?;
    let name = format!("{date}.md");
    write_named_file(
        &memory,
        Path::new(&name),
        &format!("memory-{date}"),
        content,
        expected_etag,
    )
}

pub fn search_daily_memory(query: &str) -> Result<Vec<DailyMemorySearchResult>, WorkspaceError> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    if query.chars().count() > 256 {
        return Err(WorkspaceError::InvalidInput(
            "Daily memory search query is too long".to_string(),
        ));
    }

    let root = workspace_root(false)?;
    let memory = root.join("memory");
    if !memory.exists() {
        return Ok(Vec::new());
    }
    validate_directory(&root, &memory)?;
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    for entry in fs::read_dir(&memory).map_err(|e| io_error("search daily memory", &memory, e))? {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(date) = name.strip_suffix(".md") else {
            continue;
        };
        if validate_date(date).is_err() {
            continue;
        }
        let path = entry.path();
        let bytes = match read_file_bytes(&memory, &path) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        let Ok(content) = std::str::from_utf8(&bytes) else {
            continue;
        };
        let mut match_count = 0;
        let mut first_matching_line = None;
        for line in content.lines() {
            let lower = line.to_lowercase();
            let line_matches = lower.matches(&query_lower).count();
            if line_matches > 0 && first_matching_line.is_none() {
                first_matching_line = Some(line.trim());
            }
            match_count += line_matches;
        }
        let date_matches = date.to_lowercase().contains(&query_lower);
        if match_count == 0 && !date_matches {
            continue;
        }
        let snippet_source = first_matching_line.unwrap_or(content.trim());
        let snippet = truncate_chars(snippet_source, 240);
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        results.push(DailyMemorySearchResult {
            date: date.to_string(),
            size_bytes: metadata.len(),
            modified_at: modified_at(&metadata),
            etag: etag(&bytes),
            snippet,
            match_count,
        });
    }
    results.sort_by(|left, right| right.date.cmp(&left.date));
    results.truncate(200);
    Ok(results)
}

pub fn delete_daily_memory(
    date: &str,
    expected_etag: Option<&str>,
) -> Result<DailyMemoryDeleteOutcome, WorkspaceError> {
    validate_date(date)?;
    let root = workspace_root(false)?;
    let memory = root.join("memory");
    if !memory.exists() {
        return Ok(DailyMemoryDeleteOutcome {
            date: date.to_string(),
            deleted: false,
            backup_id: None,
        });
    }
    validate_directory(&root, &memory)?;
    let path = memory.join(format!("{date}.md"));
    if !path.exists() {
        return Ok(DailyMemoryDeleteOutcome {
            date: date.to_string(),
            deleted: false,
            backup_id: None,
        });
    }

    let _guard = workspace_lock()
        .lock()
        .map_err(|_| WorkspaceError::Io("Workspace lock poisoned".to_string()))?;
    let bytes = read_file_bytes(&memory, &path)?;
    match expected_etag {
        Some(expected) if etag(&bytes) != expected => {
            return Err(WorkspaceError::Conflict(
                "Daily memory changed since it was loaded".to_string(),
            ));
        }
        None => {
            return Err(WorkspaceError::Conflict(
                "expectedEtag is required when deleting daily memory".to_string(),
            ));
        }
        _ => {}
    }
    let backup_id = Some(create_backup(&format!("memory-{date}"), &bytes)?);
    fs::remove_file(&path).map_err(|e| io_error("delete daily memory", &path, e))?;
    Ok(DailyMemoryDeleteOutcome {
        date: date.to_string(),
        deleted: true,
        backup_id,
    })
}

fn truncate_chars(value: &str, limit: usize) -> String {
    let mut chars = value.chars();
    let result = chars.by_ref().take(limit).collect::<String>();
    if chars.next().is_some() {
        format!("{result}...")
    } else {
        result
    }
}

fn write_named_file(
    root: &Path,
    relative: &Path,
    backup_name: &str,
    content: &str,
    expected_etag: Option<&str>,
) -> Result<WorkspaceWriteOutcome, WorkspaceError> {
    check_size(content.len())?;
    let _guard = workspace_lock()
        .lock()
        .map_err(|_| WorkspaceError::Io("Workspace lock poisoned".to_string()))?;
    let path = root.join(relative);
    let current = if path.exists() {
        Some(read_file_bytes(root, &path)?)
    } else {
        None
    };
    match (current.as_ref(), expected_etag) {
        (Some(bytes), Some(expected)) if etag(bytes) != expected => {
            return Err(WorkspaceError::Conflict(
                "Workspace file changed since it was loaded".to_string(),
            ));
        }
        (Some(_), None) => {
            return Err(WorkspaceError::Conflict(
                "expectedEtag is required when overwriting a workspace file".to_string(),
            ));
        }
        (None, Some(expected)) if !expected.is_empty() => {
            return Err(WorkspaceError::Conflict(
                "Workspace file no longer exists".to_string(),
            ));
        }
        _ => {}
    }

    let backup_id = current
        .as_ref()
        .map(|bytes| create_backup(backup_name, bytes))
        .transpose()?;
    atomic_write(&path, content.as_bytes())
        .map_err(|_| WorkspaceError::Io("Failed to write workspace file".to_string()))?;
    let metadata = fs::metadata(&path).map_err(|e| io_error("read metadata", &path, e))?;
    Ok(WorkspaceWriteOutcome {
        name: relative.to_string_lossy().to_string(),
        size_bytes: metadata.len(),
        modified_at: modified_at(&metadata),
        etag: etag(content.as_bytes()),
        backup_id,
    })
}

fn read_content(
    root: &Path,
    path: &Path,
    name: &str,
) -> Result<WorkspaceFileContent, WorkspaceError> {
    if !path.exists() {
        return Err(WorkspaceError::NotFound(format!(
            "Workspace file not found: {name}"
        )));
    }
    let bytes = read_file_bytes(root, path)?;
    let content = String::from_utf8(bytes.clone()).map_err(|_| {
        WorkspaceError::InvalidInput(format!("Workspace file is not valid UTF-8: {name}"))
    })?;
    let metadata = fs::metadata(path).map_err(|e| io_error("read metadata", path, e))?;
    Ok(WorkspaceFileContent {
        name: name.to_string(),
        content,
        size_bytes: metadata.len(),
        modified_at: modified_at(&metadata),
        etag: etag(&bytes),
    })
}

fn read_file_bytes(root: &Path, path: &Path) -> Result<Vec<u8>, WorkspaceError> {
    validate_regular_file(root, path)?;
    let metadata = fs::metadata(path).map_err(|e| io_error("read metadata", path, e))?;
    check_size(metadata.len() as usize)?;
    let bytes = fs::read(path).map_err(|e| io_error("read file", path, e))?;
    check_size(bytes.len())?;
    Ok(bytes)
}

fn workspace_root(create: bool) -> Result<PathBuf, WorkspaceError> {
    let root = get_openclaw_workspace_dir();
    if create {
        fs::create_dir_all(&root).map_err(|e| io_error("create workspace", &root, e))?;
    }
    if root.exists() {
        let metadata = fs::symlink_metadata(&root)
            .map_err(|e| io_error("read workspace metadata", &root, e))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(WorkspaceError::InvalidInput(
                "OpenClaw workspace root must be a real directory".to_string(),
            ));
        }
    }
    Ok(root)
}

fn ensure_directory(root: &Path, path: &Path) -> Result<(), WorkspaceError> {
    if path.exists() {
        return validate_directory(root, path);
    }
    fs::create_dir(path).map_err(|e| io_error("create directory", path, e))?;
    validate_directory(root, path)
}

fn validate_directory(root: &Path, path: &Path) -> Result<(), WorkspaceError> {
    if !path.exists() {
        return Err(WorkspaceError::NotFound(format!(
            "Workspace directory not found: {}",
            path.display()
        )));
    }
    let metadata =
        fs::symlink_metadata(path).map_err(|e| io_error("read directory metadata", path, e))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(WorkspaceError::InvalidInput(
            "Workspace subdirectory must be a real directory".to_string(),
        ));
    }
    validate_containment(root, path)
}

fn validate_regular_file(root: &Path, path: &Path) -> Result<(), WorkspaceError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|e| io_error("read file metadata", path, e))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(WorkspaceError::InvalidInput(
            "Workspace entry must be a regular file".to_string(),
        ));
    }
    validate_containment(root, path)
}

fn validate_containment(root: &Path, path: &Path) -> Result<(), WorkspaceError> {
    let canonical_root = root
        .canonicalize()
        .map_err(|e| io_error("resolve workspace root", root, e))?;
    let canonical_path = path
        .canonicalize()
        .map_err(|e| io_error("resolve workspace path", path, e))?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(WorkspaceError::InvalidInput(
            "Workspace path escapes the configured root".to_string(),
        ));
    }
    Ok(())
}

fn validate_filename(name: &str) -> Result<(), WorkspaceError> {
    if !ALLOWED_FILES.contains(&name) {
        return Err(WorkspaceError::InvalidInput(format!(
            "Invalid workspace filename: {name}"
        )));
    }
    Ok(())
}

fn validate_date(date: &str) -> Result<(), WorkspaceError> {
    if date.len() != 10
        || NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .ok()
            .filter(|parsed| parsed.format("%Y-%m-%d").to_string() == date)
            .is_none()
    {
        return Err(WorkspaceError::InvalidInput(
            "Invalid daily memory date; expected a real YYYY-MM-DD date".to_string(),
        ));
    }
    Ok(())
}

fn validate_backup_id(id: &str) -> Result<(), WorkspaceError> {
    let valid = id
        .strip_suffix(".bak")
        .and_then(|stem| stem.split_once('-'))
        .is_some_and(|(timestamp, hash)| {
            !timestamp.is_empty()
                && timestamp.chars().all(|ch| ch.is_ascii_digit())
                && (8..=64).contains(&hash.len())
                && hash.chars().all(|ch| ch.is_ascii_hexdigit())
        });
    if id.len() > 160 || !valid {
        return Err(WorkspaceError::InvalidInput(
            "Invalid workspace backup id".to_string(),
        ));
    }
    Ok(())
}

fn check_size(size: usize) -> Result<(), WorkspaceError> {
    if size > MAX_FILE_BYTES {
        return Err(WorkspaceError::TooLarge(format!(
            "Workspace file exceeds {} MiB",
            MAX_FILE_BYTES / 1024 / 1024
        )));
    }
    Ok(())
}

fn backup_dir(name: &str) -> Result<PathBuf, WorkspaceError> {
    let app_dir = get_app_config_dir().map_err(|_| {
        WorkspaceError::Io("Failed to resolve workspace backup directory".to_string())
    })?;
    validate_optional_backup_directory(&app_dir)?;
    let backups = app_dir.join("backups");
    validate_optional_backup_directory(&backups)?;
    let workspace = backups.join("workspace");
    validate_optional_backup_directory(&workspace)?;
    let dir = workspace.join(name);
    validate_optional_backup_directory(&dir)?;
    Ok(dir)
}

fn create_backup(name: &str, bytes: &[u8]) -> Result<String, WorkspaceError> {
    let dir = backup_dir(name)?;
    ensure_backup_directory(&dir)?;
    let id = format!(
        "{}-{}.bak",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        &etag(bytes)[..12]
    );
    let path = dir.join(&id);
    atomic_write(&path, bytes)
        .map_err(|_| WorkspaceError::Io("Failed to create workspace backup".to_string()))?;
    cleanup_backups(&dir);
    Ok(id)
}

fn cleanup_backups(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut files = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            let id = entry.file_name().to_string_lossy().to_string();
            validate_backup_id(&id).is_ok()
                && entry
                    .file_type()
                    .is_ok_and(|file_type| file_type.is_file() && !file_type.is_symlink())
                && entry
                    .metadata()
                    .is_ok_and(|metadata| metadata.len() <= MAX_FILE_BYTES as u64)
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|entry| entry.metadata().and_then(|meta| meta.modified()).ok());
    let remove_count = files.len().saturating_sub(MAX_BACKUPS_PER_FILE);
    for entry in files.into_iter().take(remove_count) {
        let _ = fs::remove_file(entry.path());
    }
}

fn etag(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn modified_at(metadata: &fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_secs())
        .unwrap_or_default()
}

fn validate_optional_backup_directory(path: &Path) -> Result<bool, WorkspaceError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(WorkspaceError::InvalidInput(
                    "Workspace backup path must contain only real directories".to_string(),
                ));
            }
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error("read backup directory metadata", path, error)),
    }
}

fn ensure_backup_directory(path: &Path) -> Result<(), WorkspaceError> {
    let app_dir = get_app_config_dir().map_err(|_| {
        WorkspaceError::Io("Failed to resolve workspace backup directory".to_string())
    })?;
    if !validate_optional_backup_directory(&app_dir)? {
        fs::create_dir_all(&app_dir)
            .map_err(|error| io_error("create application directory", &app_dir, error))?;
        validate_optional_backup_directory(&app_dir)?;
    }

    let relative = path.strip_prefix(&app_dir).map_err(|_| {
        WorkspaceError::InvalidInput("Workspace backup path escapes its root".to_string())
    })?;
    let mut current = app_dir;
    for component in relative.components() {
        current.push(component);
        if validate_optional_backup_directory(&current)? {
            continue;
        }
        fs::create_dir(&current)
            .map_err(|error| io_error("create backup directory", &current, error))?;
        validate_optional_backup_directory(&current)?;
    }
    Ok(())
}

fn io_error(action: &str, _path: &Path, error: std::io::Error) -> WorkspaceError {
    WorkspaceError::Io(format!("Failed to {action}: {error}"))
}

fn workspace_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn validates_real_daily_dates() {
        assert!(validate_date("2026-07-12").is_ok());
        assert!(validate_date("2026-02-30").is_err());
        assert!(validate_date("../../etc").is_err());
    }

    #[test]
    fn workspace_filename_is_strictly_whitelisted() {
        assert!(validate_filename("AGENTS.md").is_ok());
        assert!(validate_filename("../AGENTS.md").is_err());
        assert!(validate_filename("agents.md").is_err());
    }

    #[test]
    fn backup_id_rejects_path_components() {
        assert!(validate_backup_id("123-deadbeef.bak").is_ok());
        assert!(validate_backup_id("../backup.bak").is_err());
        assert!(validate_backup_id("notes.txt").is_err());
        assert!(validate_backup_id("123-not-hex.bak").is_err());
    }

    #[test]
    fn workspace_size_limit_is_inclusive() {
        assert!(check_size(MAX_FILE_BYTES).is_ok());
        assert!(matches!(
            check_size(MAX_FILE_BYTES + 1),
            Err(WorkspaceError::TooLarge(_))
        ));
    }

    #[test]
    fn workspace_write_requires_matching_etag_for_overwrite() {
        let root = tempdir().expect("workspace root");
        let created = write_named_file(
            root.path(),
            Path::new("AGENTS.md"),
            "AGENTS.md",
            "first",
            None,
        )
        .expect("create workspace file");
        assert_eq!(created.etag, etag(b"first"));

        let error = write_named_file(
            root.path(),
            Path::new("AGENTS.md"),
            "AGENTS.md",
            "second",
            Some("stale-etag"),
        )
        .expect_err("stale write must fail");
        assert!(matches!(error, WorkspaceError::Conflict(_)));
        assert_eq!(
            fs::read_to_string(root.path().join("AGENTS.md")).expect("read current file"),
            "first"
        );
    }

    #[cfg(unix)]
    #[test]
    fn workspace_reads_reject_symbolic_links() {
        use std::os::unix::fs::symlink;

        let root = tempdir().expect("workspace root");
        let outside = tempdir().expect("outside root");
        let target = outside.path().join("secret.md");
        fs::write(&target, "secret").expect("write target");
        let link = root.path().join("AGENTS.md");
        symlink(&target, &link).expect("create symlink");

        let error = read_file_bytes(root.path(), &link).expect_err("symlink must fail");
        assert!(matches!(error, WorkspaceError::InvalidInput(_)));

        let error = write_named_file(
            root.path(),
            Path::new("AGENTS.md"),
            "AGENTS.md",
            "replacement",
            None,
        )
        .expect_err("writing through a symlink must fail");
        assert!(matches!(error, WorkspaceError::InvalidInput(_)));
        assert_eq!(
            fs::read_to_string(target).expect("target remains"),
            "secret"
        );
    }
}
