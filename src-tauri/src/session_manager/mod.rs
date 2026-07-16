pub mod providers;

use serde::{Deserialize, Serialize};
use std::collections::{hash_map::DefaultHasher, BTreeMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use providers::{claude, codex, gemini, openclaw, opencode};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    pub provider_id: String,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_active_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume_command: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ts: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPage {
    pub sessions: Vec<SessionMeta>,
    pub next_cursor: Option<String>,
    pub total: usize,
    pub scanned_at: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSessionRequest {
    pub provider_id: String,
    pub session_id: String,
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSessionOutcome {
    pub provider_id: String,
    pub session_id: String,
    pub source_path: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

struct SessionCache {
    providers: BTreeMap<String, ProviderSessionCache>,
    sessions: Vec<SessionMeta>,
    loaded_at: Instant,
    scanned_at: u64,
}

#[derive(Clone)]
struct ProviderSessionCache {
    fingerprint: Option<u64>,
    sessions: Vec<SessionMeta>,
}

const SESSION_PROVIDERS: [&str; 5] = ["codex", "claude", "opencode", "gemini", "openclaw"];
const SESSION_CACHE_TTL: Duration = Duration::from_secs(15);
const MAX_FINGERPRINT_DEPTH: usize = 16;
const MAX_FINGERPRINT_ENTRIES: usize = 100_000;

pub fn scan_sessions() -> Vec<SessionMeta> {
    scan_sessions_with_refresh(false)
}

pub fn scan_sessions_with_refresh(refresh: bool) -> Vec<SessionMeta> {
    if !refresh {
        let cache = lock_session_cache();
        if cache.loaded_at.elapsed() < SESSION_CACHE_TTL {
            return cache.sessions.clone();
        }
    }

    let _scan_guard = session_scan_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !refresh {
        let cache = lock_session_cache();
        if cache.loaded_at.elapsed() < SESSION_CACHE_TTL {
            return cache.sessions.clone();
        }
    }

    let (previous, previous_scan_id) = {
        let cache = lock_session_cache();
        (cache.providers.clone(), cache.scanned_at)
    };
    let (providers, sessions) = scan_sessions_incremental(previous);
    let mut cache = lock_session_cache();
    cache.providers = providers;
    cache.sessions = sessions.clone();
    cache.loaded_at = Instant::now();
    cache.scanned_at = now_millis().max(previous_scan_id.saturating_add(1));
    sessions
}

pub fn scan_sessions_page(
    cursor: Option<&str>,
    limit: usize,
    provider_id: Option<&str>,
    refresh: bool,
) -> Result<SessionPage, String> {
    scan_sessions_page_with_query(cursor, limit, provider_id, None, refresh)
}

pub fn scan_sessions_page_with_query(
    cursor: Option<&str>,
    limit: usize,
    provider_id: Option<&str>,
    query: Option<&str>,
    refresh: bool,
) -> Result<SessionPage, String> {
    let query = query.map(str::trim).filter(|value| !value.is_empty());
    if query.is_some_and(|value| value.chars().count() > 256) {
        return Err("Session search query is too long".to_string());
    }
    let signature = session_filter_signature(provider_id, query);
    let (cursor_scan_id, offset, cursor_signature) = parse_session_cursor(cursor)?;
    if cursor_signature.is_some_and(|value| value != signature) {
        return Err("Session cursor does not match the current filters".to_string());
    }
    let limit = limit.clamp(1, 200);
    let (sessions, scanned_at) = if let Some(cursor_scan_id) = cursor_scan_id {
        let cache = lock_session_cache();
        if cache.scanned_at != cursor_scan_id {
            return Err("Session cursor expired; restart pagination".to_string());
        }
        (cache.sessions.clone(), cache.scanned_at)
    } else {
        scan_sessions_with_refresh(refresh);
        let cache = lock_session_cache();
        (cache.sessions.clone(), cache.scanned_at)
    };
    let sessions = sessions
        .into_iter()
        .filter(|session| provider_id.map_or(true, |id| session.provider_id == id))
        .filter(|session| query.map_or(true, |query| session_matches_query(session, query)))
        .collect::<Vec<_>>();
    let total = sessions.len();
    let page = sessions
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>();
    let next_offset = offset.saturating_add(page.len());
    Ok(SessionPage {
        sessions: page,
        next_cursor: (next_offset < total)
            .then(|| format!("{scanned_at}:{next_offset}:{signature:016x}")),
        total,
        scanned_at,
    })
}

fn session_filter_signature(provider_id: Option<&str>, query: Option<&str>) -> u64 {
    let mut hasher = DefaultHasher::new();
    "session-filter-v1".hash(&mut hasher);
    provider_id.unwrap_or_default().hash(&mut hasher);
    query.unwrap_or_default().to_lowercase().hash(&mut hasher);
    hasher.finish()
}

fn session_matches_query(session: &SessionMeta, query: &str) -> bool {
    let haystack = [
        Some(session.provider_id.as_str()),
        Some(session.session_id.as_str()),
        session.title.as_deref(),
        session.summary.as_deref(),
        session.project_dir.as_deref(),
        session.source_path.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n")
    .to_lowercase();
    query
        .to_lowercase()
        .split_whitespace()
        .all(|term| haystack.contains(term))
}

fn scan_sessions_incremental(
    mut previous: BTreeMap<String, ProviderSessionCache>,
) -> (BTreeMap<String, ProviderSessionCache>, Vec<SessionMeta>) {
    let fingerprints = scan_provider_fingerprints();
    let providers_to_scan = SESSION_PROVIDERS
        .iter()
        .copied()
        .filter(|provider_id| {
            let fingerprint = fingerprints.get(*provider_id).copied().flatten();
            !matches!(
                (previous.get(*provider_id), fingerprint),
                (Some(cached), Some(fingerprint))
                    if cached.fingerprint == Some(fingerprint)
            )
        })
        .collect::<Vec<_>>();
    let mut scanned = scan_provider_sessions(&providers_to_scan);
    let mut providers = BTreeMap::new();
    for provider_id in SESSION_PROVIDERS {
        let fingerprint = fingerprints.get(provider_id).copied().flatten();
        let sessions = scanned
            .remove(provider_id)
            .or_else(|| previous.remove(provider_id).map(|entry| entry.sessions))
            .unwrap_or_default();
        providers.insert(
            provider_id.to_string(),
            ProviderSessionCache {
                fingerprint,
                sessions,
            },
        );
    }
    let mut sessions = providers
        .values()
        .flat_map(|entry| entry.sessions.iter().cloned())
        .collect::<Vec<_>>();
    sort_sessions(&mut sessions);
    (providers, sessions)
}

fn scan_provider_fingerprints() -> BTreeMap<String, Option<u64>> {
    std::thread::scope(|scope| {
        let handles = SESSION_PROVIDERS
            .iter()
            .copied()
            .map(|provider_id| {
                (
                    provider_id,
                    scope.spawn(move || {
                        provider_root(provider_id)
                            .ok()
                            .and_then(|root| fingerprint_session_tree(&root))
                    }),
                )
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|(provider_id, handle)| {
                (provider_id.to_string(), handle.join().unwrap_or_default())
            })
            .collect()
    })
}

fn scan_provider_sessions(provider_ids: &[&str]) -> BTreeMap<String, Vec<SessionMeta>> {
    std::thread::scope(|scope| {
        let handles = provider_ids
            .iter()
            .copied()
            .map(|provider_id| {
                (
                    provider_id,
                    scope.spawn(move || scan_provider_sessions_uncached(provider_id)),
                )
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|(provider_id, handle)| {
                (provider_id.to_string(), handle.join().unwrap_or_default())
            })
            .collect()
    })
}

fn scan_provider_sessions_uncached(provider_id: &str) -> Vec<SessionMeta> {
    match provider_id {
        "codex" => codex::scan_sessions(),
        "claude" => claude::scan_sessions(),
        "opencode" => opencode::scan_sessions(),
        "gemini" => gemini::scan_sessions(),
        "openclaw" => openclaw::scan_sessions(),
        _ => Vec::new(),
    }
}

fn sort_sessions(sessions: &mut [SessionMeta]) {
    sessions.sort_by(|a, b| {
        let a_ts = a.last_active_at.or(a.created_at).unwrap_or(0);
        let b_ts = b.last_active_at.or(b.created_at).unwrap_or(0);
        b_ts.cmp(&a_ts)
    });
}

pub fn load_messages(provider_id: &str, source_path: &str) -> Result<Vec<SessionMessage>, String> {
    // SQLite sessions use a "sqlite:" prefixed source_path
    if provider_id == "opencode" && source_path.starts_with("sqlite:") {
        return opencode::load_messages_sqlite(source_path);
    }
    let root = provider_root(provider_id)?;
    let path = validate_source_path(Path::new(source_path), &root)?;
    match provider_id {
        "codex" => codex::load_messages(&path),
        "claude" => claude::load_messages(&path),
        "opencode" => opencode::load_messages(&path),
        "gemini" => gemini::load_messages(&path),
        "openclaw" => openclaw::load_messages(&path),
        _ => Err(format!("Unsupported provider: {provider_id}")),
    }
}

pub fn delete_session(
    provider_id: &str,
    session_id: &str,
    source_path: &str,
) -> Result<bool, String> {
    // SQLite sessions bypass the file-based deletion path
    if provider_id == "opencode" && source_path.starts_with("sqlite:") {
        let deleted = opencode::delete_session_sqlite(session_id, source_path)?;
        if deleted {
            invalidate_session_cache();
        }
        return Ok(deleted);
    }
    let root = provider_root(provider_id)?;
    let deleted = delete_session_with_root(provider_id, session_id, Path::new(source_path), &root)?;
    if deleted {
        invalidate_session_cache();
    }
    Ok(deleted)
}

pub fn delete_sessions(requests: &[DeleteSessionRequest]) -> Vec<DeleteSessionOutcome> {
    collect_delete_session_outcomes(requests, |request| {
        delete_session(
            &request.provider_id,
            &request.session_id,
            &request.source_path,
        )
    })
}

fn delete_session_with_root(
    provider_id: &str,
    session_id: &str,
    source_path: &Path,
    root: &Path,
) -> Result<bool, String> {
    let validated_root = canonicalize_existing_path(root, "session root")?;
    let validated_source = validate_source_path(source_path, &validated_root)?;

    match provider_id {
        "codex" => codex::delete_session(&validated_root, &validated_source, session_id),
        "claude" => claude::delete_session(&validated_root, &validated_source, session_id),
        "opencode" => opencode::delete_session(&validated_root, &validated_source, session_id),
        "gemini" => gemini::delete_session(&validated_root, &validated_source, session_id),
        "openclaw" => openclaw::delete_session(&validated_root, &validated_source, session_id),
        _ => Err(format!("Unsupported provider: {provider_id}")),
    }
}

fn provider_root(provider_id: &str) -> Result<PathBuf, String> {
    let root = match provider_id {
        "codex" => crate::codex_config::get_codex_config_dir()
            .map_err(|e| e.to_string())?
            .join("sessions"),
        "claude" => crate::config::get_claude_config_dir()
            .map_err(|e| e.to_string())?
            .join("projects"),
        "opencode" => opencode::get_opencode_data_dir(),
        "gemini" => crate::gemini_config::get_gemini_dir()
            .map_err(|e| e.to_string())?
            .join("tmp"),
        "openclaw" => openclaw::get_agents_root(),
        _ => return Err(format!("Unsupported provider: {provider_id}")),
    };

    Ok(root)
}

fn validate_source_path(source_path: &Path, root: &Path) -> Result<PathBuf, String> {
    if std::fs::symlink_metadata(source_path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err("Session source path must not be a symbolic link".to_string());
    }
    let validated_root = canonicalize_existing_path(root, "session root")?;
    let validated_source = canonicalize_existing_path(source_path, "session source")?;
    if !validated_source.starts_with(&validated_root) {
        return Err("Session source path is outside provider root".to_string());
    }
    Ok(validated_source)
}

fn session_cache() -> &'static Mutex<SessionCache> {
    static CACHE: OnceLock<Mutex<SessionCache>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(SessionCache {
            providers: BTreeMap::new(),
            sessions: Vec::new(),
            loaded_at: Instant::now() - Duration::from_secs(60),
            scanned_at: 0,
        })
    })
}

fn lock_session_cache() -> MutexGuard<'static, SessionCache> {
    session_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn session_scan_lock() -> &'static Mutex<()> {
    static SCAN_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    SCAN_LOCK.get_or_init(|| Mutex::new(()))
}

fn parse_session_cursor(cursor: Option<&str>) -> Result<(Option<u64>, usize, Option<u64>), String> {
    let Some(cursor) = cursor else {
        return Ok((None, 0, None));
    };
    let mut parts = cursor.split(':');
    let (Some(scan_id), Some(offset)) = (parts.next(), parts.next()) else {
        return Err("Invalid session cursor".to_string());
    };
    let signature = parts.next();
    if scan_id.is_empty()
        || offset.is_empty()
        || signature.is_some_and(str::is_empty)
        || parts.next().is_some()
    {
        return Err("Invalid session cursor".to_string());
    }
    let scan_id = scan_id
        .parse::<u64>()
        .map_err(|_| "Invalid session cursor".to_string())?;
    let offset = offset
        .parse::<usize>()
        .map_err(|_| "Invalid session cursor".to_string())?;
    let signature = signature
        .map(|value| u64::from_str_radix(value, 16))
        .transpose()
        .map_err(|_| "Invalid session cursor".to_string())?;
    Ok((Some(scan_id), offset, signature))
}

fn fingerprint_session_tree(root: &Path) -> Option<u64> {
    fingerprint_session_tree_with_limits(root, MAX_FINGERPRINT_DEPTH, MAX_FINGERPRINT_ENTRIES)
}

fn fingerprint_session_tree_with_limits(
    root: &Path,
    max_depth: usize,
    max_entries: usize,
) -> Option<u64> {
    let root_metadata = match std::fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut hasher = DefaultHasher::new();
            "missing-session-root".hash(&mut hasher);
            return Some(hasher.finish());
        }
        Err(_) => return None,
    };
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return None;
    }

    let mut entries = Vec::new();
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    while let Some((directory, depth)) = pending.pop() {
        let children = std::fs::read_dir(&directory).ok()?;
        for child in children {
            let child = child.ok()?;
            let path = child.path();
            let metadata = std::fs::symlink_metadata(&path).ok()?;
            if metadata.file_type().is_symlink() {
                continue;
            }

            if entries.len() >= max_entries {
                return None;
            }
            let child_depth = depth.saturating_add(1);
            if child_depth > max_depth {
                return None;
            }
            let relative_path = path.strip_prefix(root).ok()?.to_path_buf();
            let kind = if metadata.is_dir() {
                0u8
            } else if metadata.is_file() {
                1u8
            } else {
                2u8
            };
            let modified = metadata.modified().ok().map(system_time_hash_key);
            entries.push((relative_path, kind, metadata.len(), modified));

            if metadata.is_dir() {
                pending.push((path, child_depth));
            }
        }
    }

    entries.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = DefaultHasher::new();
    "session-tree-v1".hash(&mut hasher);
    entries.hash(&mut hasher);
    Some(hasher.finish())
}

fn system_time_hash_key(value: SystemTime) -> (bool, u64, u32) {
    match value.duration_since(UNIX_EPOCH) {
        Ok(duration) => (false, duration.as_secs(), duration.subsec_nanos()),
        Err(error) => {
            let duration = error.duration();
            (true, duration.as_secs(), duration.subsec_nanos())
        }
    }
}

fn invalidate_session_cache() {
    if let Ok(mut cache) = session_cache().lock() {
        cache.loaded_at = Instant::now() - Duration::from_secs(60);
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn canonicalize_existing_path(path: &Path, label: &str) -> Result<PathBuf, String> {
    if !path.exists() {
        return Err(format!("{label} not found"));
    }

    path.canonicalize()
        .map_err(|_| format!("Failed to resolve {label}"))
}

fn collect_delete_session_outcomes<F>(
    requests: &[DeleteSessionRequest],
    mut deleter: F,
) -> Vec<DeleteSessionOutcome>
where
    F: FnMut(&DeleteSessionRequest) -> Result<bool, String>,
{
    requests
        .iter()
        .map(|request| match deleter(request) {
            Ok(true) => DeleteSessionOutcome {
                provider_id: request.provider_id.clone(),
                session_id: request.session_id.clone(),
                source_path: request.source_path.clone(),
                success: true,
                error: None,
            },
            Ok(false) => DeleteSessionOutcome {
                provider_id: request.provider_id.clone(),
                session_id: request.session_id.clone(),
                source_path: request.source_path.clone(),
                success: false,
                error: Some("Session was not deleted".to_string()),
            },
            Err(error) => DeleteSessionOutcome {
                provider_id: request.provider_id.clone(),
                session_id: request.session_id.clone(),
                source_path: request.source_path.clone(),
                success: false,
                error: Some(error),
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn rejects_source_path_outside_provider_root() {
        let root = tempdir().expect("tempdir");
        let outside = tempdir().expect("tempdir");
        let source = outside.path().join("session.jsonl");
        std::fs::write(&source, "{}").expect("write source");

        let err = delete_session_with_root("codex", "session-1", &source, root.path())
            .expect_err("expected outside-root path to be rejected");

        assert!(err.contains("outside provider root"));
        assert!(!err.contains(&source.display().to_string()));
    }

    #[test]
    fn rejects_missing_source_path() {
        let root = tempdir().expect("tempdir");
        let missing = root.path().join("missing.jsonl");

        let err = delete_session_with_root("codex", "session-1", &missing, root.path())
            .expect_err("expected missing source path to fail");

        assert!(err.contains("session source not found"));
        assert!(!err.contains(&missing.display().to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symbolic_link_session_source() {
        use std::os::unix::fs::symlink;

        let root = tempdir().expect("tempdir");
        let target = root.path().join("target.jsonl");
        let source = root.path().join("linked.jsonl");
        std::fs::write(&target, "{}").expect("write target");
        symlink(&target, &source).expect("create source symlink");

        let error = validate_source_path(&source, root.path())
            .expect_err("symbolic link source must be rejected");

        assert_eq!(error, "Session source path must not be a symbolic link");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_session_source_through_parent_symlink() {
        use std::os::unix::fs::symlink;

        let root = tempdir().expect("tempdir");
        let outside = tempdir().expect("outside tempdir");
        let outside_source = outside.path().join("outside.jsonl");
        std::fs::write(&outside_source, "{}").expect("write outside source");
        let linked_parent = root.path().join("linked-parent");
        symlink(outside.path(), &linked_parent).expect("create parent symlink");
        let source = linked_parent.join("outside.jsonl");

        let error = validate_source_path(&source, root.path())
            .expect_err("parent symlink escape must be rejected");

        assert_eq!(error, "Session source path is outside provider root");
        assert!(!error.contains(&source.display().to_string()));
    }

    #[test]
    fn parses_snapshot_bound_session_cursors() {
        assert_eq!(parse_session_cursor(None).unwrap(), (None, 0, None));
        assert_eq!(
            parse_session_cursor(Some("42:7")).unwrap(),
            (Some(42), 7, None)
        );
        assert_eq!(
            parse_session_cursor(Some("42:7:000000000000000a")).unwrap(),
            (Some(42), 7, Some(10))
        );

        for invalid in [
            "",
            "7",
            ":7",
            "42:",
            "42:7:",
            "42:7:not-hex",
            "42:7:1:2",
            "scan:7",
            "42:offset",
        ] {
            assert_eq!(
                parse_session_cursor(Some(invalid)).unwrap_err(),
                "Invalid session cursor"
            );
        }
    }

    #[test]
    fn session_page_cursor_expires_when_snapshot_changes() {
        static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _test_guard = TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let original = {
            let mut cache = lock_session_cache();
            std::mem::replace(
                &mut *cache,
                SessionCache {
                    providers: BTreeMap::new(),
                    sessions: vec![test_session("newer", 20), test_session("older", 10)],
                    loaded_at: Instant::now(),
                    scanned_at: 42,
                },
            )
        };

        let first = scan_sessions_page(None, 1, None, false).expect("first page");
        let cursor = first.next_cursor.clone().expect("next cursor");
        let second = scan_sessions_page(Some(&cursor), 1, None, false).expect("second page");
        {
            let mut cache = lock_session_cache();
            cache.sessions.reverse();
            cache.scanned_at = 43;
        }
        let expired =
            scan_sessions_page(Some(&cursor), 1, None, false).expect_err("old cursor must expire");

        *lock_session_cache() = original;

        assert_eq!(first.scanned_at, 42);
        assert_eq!(first.sessions[0].session_id, "newer");
        assert!(cursor.starts_with("42:1:"));
        assert_eq!(second.sessions[0].session_id, "older");
        assert_eq!(expired, "Session cursor expired; restart pagination");
    }

    #[test]
    fn session_page_searches_the_full_snapshot_and_binds_cursor_to_filters() {
        static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _test_guard = TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut alpha = test_session("alpha", 20);
        alpha.title = Some("Routine maintenance".to_string());
        let mut beta = test_session("beta", 10);
        beta.summary = Some("Quarterly migration".to_string());
        let original = {
            let mut cache = lock_session_cache();
            std::mem::replace(
                &mut *cache,
                SessionCache {
                    providers: BTreeMap::new(),
                    sessions: vec![alpha, beta],
                    loaded_at: Instant::now(),
                    scanned_at: 84,
                },
            )
        };

        let page = scan_sessions_page_with_query(None, 1, None, Some("migration"), false)
            .expect("search page");
        assert_eq!(page.total, 1);
        assert_eq!(page.sessions[0].session_id, "beta");

        let unfiltered =
            scan_sessions_page_with_query(None, 1, None, None, false).expect("unfiltered page");
        let cursor = unfiltered.next_cursor.expect("unfiltered cursor");
        let mismatch =
            scan_sessions_page_with_query(Some(&cursor), 1, None, Some("migration"), false)
                .expect_err("cursor must be bound to its search");
        assert_eq!(
            mismatch,
            "Session cursor does not match the current filters"
        );

        *lock_session_cache() = original;
    }

    #[test]
    fn session_tree_fingerprint_is_stable_and_tracks_file_changes() {
        let root = tempdir().expect("tempdir");
        let nested = root.path().join("project");
        std::fs::create_dir(&nested).expect("create nested directory");
        let session = nested.join("session.jsonl");
        std::fs::write(&session, "a").expect("write session");

        let first = fingerprint_session_tree(root.path()).expect("first fingerprint");
        let unchanged = fingerprint_session_tree(root.path()).expect("unchanged fingerprint");
        std::fs::write(&session, "longer session").expect("update session");
        let changed = fingerprint_session_tree(root.path()).expect("changed fingerprint");

        assert_eq!(first, unchanged);
        assert_ne!(first, changed);
    }

    #[test]
    fn session_tree_fingerprint_returns_none_when_limits_are_exceeded() {
        let root = tempdir().expect("tempdir");
        std::fs::write(root.path().join("one"), "1").expect("write first file");
        std::fs::write(root.path().join("two"), "2").expect("write second file");
        assert!(fingerprint_session_tree_with_limits(root.path(), 16, 1).is_none());

        let nested = root.path().join("a");
        std::fs::create_dir(&nested).expect("create nested directory");
        std::fs::write(nested.join("deep"), "3").expect("write nested file");
        assert!(fingerprint_session_tree_with_limits(root.path(), 1, 100).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn session_tree_fingerprint_ignores_symbolic_links() {
        use std::os::unix::fs::symlink;

        let root = tempdir().expect("tempdir");
        let outside = tempdir().expect("outside tempdir");
        let target = outside.path().join("target.jsonl");
        std::fs::write(&target, "first").expect("write target");
        symlink(&target, root.path().join("linked.jsonl")).expect("create symlink");

        let first = fingerprint_session_tree(root.path()).expect("first fingerprint");
        std::fs::write(&target, "different target contents").expect("update target");
        let second = fingerprint_session_tree(root.path()).expect("second fingerprint");

        assert_eq!(first, second);
    }

    #[test]
    fn batch_delete_collects_successes_and_failures_in_order() {
        let requests = vec![
            DeleteSessionRequest {
                provider_id: "codex".to_string(),
                session_id: "s1".to_string(),
                source_path: "/tmp/s1".to_string(),
            },
            DeleteSessionRequest {
                provider_id: "claude".to_string(),
                session_id: "s2".to_string(),
                source_path: "/tmp/s2".to_string(),
            },
            DeleteSessionRequest {
                provider_id: "gemini".to_string(),
                session_id: "s3".to_string(),
                source_path: "/tmp/s3".to_string(),
            },
        ];

        let outcomes = collect_delete_session_outcomes(&requests, |request| {
            match request.session_id.as_str() {
                "s1" => Ok(true),
                "s2" => Err("boom".to_string()),
                _ => Ok(false),
            }
        });

        assert_eq!(outcomes.len(), 3);
        assert!(outcomes[0].success);
        assert_eq!(outcomes[0].error, None);
        assert!(!outcomes[1].success);
        assert_eq!(outcomes[1].error.as_deref(), Some("boom"));
        assert!(!outcomes[2].success);
        assert_eq!(
            outcomes[2].error.as_deref(),
            Some("Session was not deleted")
        );
    }

    fn test_session(session_id: &str, last_active_at: i64) -> SessionMeta {
        SessionMeta {
            provider_id: "codex".to_string(),
            session_id: session_id.to_string(),
            title: None,
            summary: None,
            project_dir: None,
            created_at: None,
            last_active_at: Some(last_active_at),
            source_path: None,
            resume_command: None,
        }
    }
}
