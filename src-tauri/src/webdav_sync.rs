use crate::{
    app_config::MultiAppConfig,
    database::SCHEMA_VERSION,
    error::AppError,
    services::ConfigService,
    settings::{self, WebDavSettings},
    store::AppState,
};
use chrono::{SecondsFormat, Utc};
use futures::StreamExt;
use reqwest::{Client, Method, RequestBuilder, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::time::MissedTickBehavior;

const SNAPSHOT_KIND: &str = "cc-switch-web-snapshot";
const SNAPSHOT_FILE_EXT: &str = "json";
const BACKUP_DIR_SUFFIX: &str = "history";
const BACKUP_INDEX_FILE: &str = "index.json";
const MAX_BACKUPS: usize = 20;
const MAX_SNAPSHOT_BYTES: usize = 10 * 1024 * 1024;
const MAX_INDEX_BYTES: usize = 1024 * 1024;
const MAX_ERROR_BODY_BYTES: usize = 4 * 1024;
const DEFAULT_AUTO_SYNC_INTERVAL_SECS: u64 = 5 * 60;
const MIN_AUTO_SYNC_INTERVAL_SECS: u64 = 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavCompatibilityCheck {
    pub name: String,
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavSnapshotPreview {
    pub exists: bool,
    pub remote_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<String>,
    pub artifact_list: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<i32>,
    pub compatible: bool,
    pub checks: Vec<WebDavCompatibilityCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavBackupEntry {
    pub id: String,
    pub remote_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    pub artifact_list: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<i32>,
    pub compatible: bool,
    pub checks: Vec<WebDavCompatibilityCheck>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavSyncResult {
    pub success: bool,
    pub message: String,
    pub remote_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<WebDavSnapshotPreview>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavAutoSyncResult {
    pub action: String,
    pub message: String,
    pub local_config_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_preview: Option<WebDavSnapshotPreview>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<WebDavSyncResult>,
}

#[derive(Debug, Clone)]
struct RemoteTarget {
    snapshot_url: Url,
    backup_index_url: Url,
    backup_segments: Vec<String>,
    collection_urls: Vec<Url>,
}

#[derive(Debug)]
struct ParsedSnapshot {
    config: MultiAppConfig,
    artifact_list: Vec<String>,
    schema_version: Option<i32>,
    snapshot_id: Option<String>,
    created_at: Option<String>,
    config_hash: Option<String>,
}

#[derive(Debug, Clone)]
struct SnapshotPayload {
    bytes: Vec<u8>,
    backup_id: String,
    created_at: String,
    artifact_list: Vec<String>,
    config_version: u32,
    schema_version: i32,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupIndex {
    #[serde(default)]
    backups: Vec<WebDavBackupEntry>,
}

pub async fn upload_snapshot(
    state: &AppState,
    settings: &WebDavSettings,
) -> Result<WebDavSyncResult, AppError> {
    let settings = normalized_settings(settings)?;
    let target = remote_target(&settings)?;
    let client = webdav_client()?;
    ensure_remote_collections(&client, &settings, &target.collection_urls).await?;

    let config = state.load_config()?;
    let payload = build_snapshot_payload(config)?;
    let backup_url = backup_file_url(&settings, &target, &payload.backup_id)?;

    let response = with_auth(
        client
            .put(target.snapshot_url.clone())
            .header("content-type", "application/json")
            .body(payload.bytes.clone()),
        &settings,
    )
    .send()
    .await
    .map_err(reqwest_error)?;
    if !response.status().is_success() {
        return Err(status_error("WebDAV upload failed", response).await);
    }

    let backup_response = with_auth(
        client
            .put(backup_url.clone())
            .header("content-type", "application/json")
            .body(payload.bytes.clone()),
        &settings,
    )
    .send()
    .await
    .map_err(reqwest_error)?;
    if !backup_response.status().is_success() {
        return Err(status_error("WebDAV backup upload failed", backup_response).await);
    }

    let preview = preview_snapshot_with_client(&client, &settings, target.snapshot_url.clone())
        .await?
        .unwrap_or_else(|| missing_preview(target.snapshot_url.as_str()));
    update_backup_index(&client, &settings, &target, &payload, &backup_url, &preview).await?;

    Ok(WebDavSyncResult {
        success: true,
        message: "Snapshot uploaded".to_string(),
        remote_path: target.snapshot_url.to_string(),
        backup_id: Some(payload.backup_id),
        preview: Some(preview),
    })
}

pub async fn preview_snapshot(
    settings: &WebDavSettings,
) -> Result<WebDavSnapshotPreview, AppError> {
    let settings = normalized_settings(settings)?;
    let target = remote_target(&settings)?;
    let client = webdav_client()?;
    Ok(
        preview_snapshot_with_client(&client, &settings, target.snapshot_url.clone())
            .await?
            .unwrap_or_else(|| missing_preview(target.snapshot_url.as_str())),
    )
}

pub async fn sync_snapshot(
    state: &AppState,
    settings: &WebDavSettings,
) -> Result<WebDavAutoSyncResult, AppError> {
    let settings = normalized_settings(settings)?;
    let local_config_hash = local_config_hash(state)?;
    let remote_preview = preview_snapshot(&settings).await?;
    let action = decide_sync_action(
        &local_config_hash,
        &remote_preview,
        settings.last_sync_config_hash.as_deref(),
    );

    match action {
        "upload" => {
            let result = upload_snapshot(state, &settings).await?;
            Ok(WebDavAutoSyncResult {
                action: "uploaded".to_string(),
                message: "Local snapshot uploaded".to_string(),
                local_config_hash,
                remote_preview: result.preview.clone(),
                result: Some(result),
            })
        }
        "download" => {
            let result = download_snapshot(state, &settings).await?;
            Ok(WebDavAutoSyncResult {
                action: "downloaded".to_string(),
                message: "Remote snapshot downloaded".to_string(),
                local_config_hash,
                remote_preview: result.preview.clone(),
                result: Some(result),
            })
        }
        "unchanged" => Ok(WebDavAutoSyncResult {
            action: "unchanged".to_string(),
            message: "Local and remote snapshots are already in sync".to_string(),
            local_config_hash,
            remote_preview: Some(remote_preview),
            result: None,
        }),
        _ => Ok(WebDavAutoSyncResult {
            action: "conflict".to_string(),
            message: "Local and remote snapshots both need review before sync".to_string(),
            local_config_hash,
            remote_preview: Some(remote_preview),
            result: None,
        }),
    }
}

pub fn start_auto_sync_worker(state: Arc<AppState>) {
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Ordering::SeqCst) {
        log::debug!("WebDAV auto sync worker is already running");
        return;
    }

    tokio::spawn(async move {
        let interval_secs = auto_sync_interval_secs();
        log::info!("WebDAV auto sync worker started; interval={interval_secs}s");
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            interval.tick().await;
            match auto_sync_once_if_enabled(&state).await {
                Ok(Some(result)) => {
                    log::info!(
                        "WebDAV auto sync finished: action={}, message={}",
                        result.action,
                        result.message
                    );
                }
                Ok(None) => {}
                Err(err) => log::warn!("WebDAV auto sync failed: {err}"),
            }
        }
    });
}

async fn auto_sync_once_if_enabled(
    state: &AppState,
) -> Result<Option<WebDavAutoSyncResult>, AppError> {
    let webdav_settings = settings::get_settings().webdav;
    if !should_auto_sync(&webdav_settings) {
        return Ok(None);
    }

    let result = sync_snapshot(state, &webdav_settings).await?;
    if result.action != "conflict" {
        persist_sync_marker_from_result(&result)?;
    }
    Ok(Some(result))
}

fn should_auto_sync(settings: &WebDavSettings) -> bool {
    settings.enabled
        && settings.auto_sync
        && !settings.base_url.trim().is_empty()
        && !settings.profile.trim().is_empty()
}

fn auto_sync_interval_secs() -> u64 {
    std::env::var("WEBDAV_AUTO_SYNC_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_AUTO_SYNC_INTERVAL_SECS)
        .max(MIN_AUTO_SYNC_INTERVAL_SECS)
}

fn persist_sync_marker_from_result(result: &WebDavAutoSyncResult) -> Result<(), AppError> {
    let Some(preview) = sync_marker_preview(result) else {
        return Ok(());
    };
    let Some(config_hash) = preview.config_hash.as_deref() else {
        return Ok(());
    };

    let mut app_settings = settings::get_settings();
    app_settings.webdav.last_sync_config_hash = Some(config_hash.to_string());
    app_settings.webdav.last_sync_at = Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true));
    app_settings.webdav.last_sync_remote_snapshot_id = preview.snapshot_id.clone();
    settings::update_settings(app_settings)
}

fn sync_marker_preview(result: &WebDavAutoSyncResult) -> Option<&WebDavSnapshotPreview> {
    result
        .result
        .as_ref()
        .and_then(|sync_result| sync_result.preview.as_ref())
        .or(result.remote_preview.as_ref())
}

pub async fn list_backups(settings: &WebDavSettings) -> Result<Vec<WebDavBackupEntry>, AppError> {
    let settings = normalized_settings(settings)?;
    let target = remote_target(&settings)?;
    let client = webdav_client()?;
    Ok(
        load_backup_index(&client, &settings, target.backup_index_url.clone())
            .await?
            .backups,
    )
}

pub async fn restore_backup(
    state: &AppState,
    settings: &WebDavSettings,
    backup_id: &str,
) -> Result<WebDavSyncResult, AppError> {
    let backup_id = sanitize_backup_id(backup_id)?;
    let settings = normalized_settings(settings)?;
    let target = remote_target(&settings)?;
    let client = webdav_client()?;
    let index = load_backup_index(&client, &settings, target.backup_index_url.clone()).await?;
    let entry = index
        .backups
        .iter()
        .find(|backup| backup.id == backup_id)
        .cloned()
        .ok_or_else(|| AppError::InvalidInput("WebDAV backup was not found".into()))?;

    let backup_url = backup_file_url(&settings, &target, &backup_id)?;
    let Some(downloaded) = download_snapshot_bytes(&client, &settings, backup_url.clone()).await?
    else {
        return Err(AppError::InvalidInput(
            "Remote WebDAV backup file was not found".into(),
        ));
    };
    let parsed = parse_snapshot_value(&downloaded.value)?;
    let preview = build_preview(
        backup_url.as_str(),
        Some(downloaded.bytes_len as u64),
        downloaded.modified_at,
        &parsed,
    );
    if !preview.compatible {
        return Err(AppError::InvalidInput(
            "Remote WebDAV backup is not compatible with this version".into(),
        ));
    }

    let local_backup_id = ConfigService::apply_import_config(parsed.config, state)?;
    Ok(WebDavSyncResult {
        success: true,
        message: "Backup restored".to_string(),
        remote_path: entry.remote_path,
        backup_id: Some(local_backup_id),
        preview: Some(preview),
    })
}

pub async fn download_snapshot(
    state: &AppState,
    settings: &WebDavSettings,
) -> Result<WebDavSyncResult, AppError> {
    let settings = normalized_settings(settings)?;
    let target = remote_target(&settings)?;
    let client = webdav_client()?;
    let Some(downloaded) =
        download_snapshot_bytes(&client, &settings, target.snapshot_url.clone()).await?
    else {
        return Err(AppError::InvalidInput(
            "Remote WebDAV snapshot not found".into(),
        ));
    };
    let parsed = parse_snapshot_value(&downloaded.value)?;
    let preview = build_preview(
        target.snapshot_url.as_str(),
        Some(downloaded.bytes_len as u64),
        downloaded.modified_at,
        &parsed,
    );
    if !preview.compatible {
        return Err(AppError::InvalidInput(
            "Remote WebDAV snapshot is not compatible with this version".into(),
        ));
    }

    let backup_id = ConfigService::apply_import_config(parsed.config, state)?;
    Ok(WebDavSyncResult {
        success: true,
        message: "Snapshot downloaded".to_string(),
        remote_path: target.snapshot_url.to_string(),
        backup_id: Some(backup_id),
        preview: Some(preview),
    })
}

fn webdav_client() -> Result<Client, AppError> {
    Client::builder()
        .timeout(Duration::from_secs(45))
        .build()
        .map_err(reqwest_error)
}

fn normalized_settings(settings: &WebDavSettings) -> Result<WebDavSettings, AppError> {
    let mut next = settings.clone();
    next.base_url = next.base_url.trim().trim_end_matches('/').to_string();
    next.username = next.username.trim().to_string();
    next.remote_dir = next.remote_dir.trim().trim_matches('/').to_string();
    next.profile = next.profile.trim().to_string();
    if next.base_url.is_empty() {
        return Err(AppError::InvalidInput("WebDAV base URL is required".into()));
    }
    if next.profile.is_empty() {
        return Err(AppError::InvalidInput("WebDAV profile is required".into()));
    }
    Ok(next)
}

fn remote_target(settings: &WebDavSettings) -> Result<RemoteTarget, AppError> {
    let base = Url::parse(&settings.base_url)
        .map_err(|e| AppError::InvalidInput(format!("Invalid WebDAV base URL: {e}")))?;
    if !matches!(base.scheme(), "http" | "https") {
        return Err(AppError::InvalidInput(
            "WebDAV base URL must use http or https".into(),
        ));
    }
    if !base.username().is_empty() || base.password().is_some() {
        return Err(AppError::InvalidInput(
            "WebDAV credentials must be configured separately".into(),
        ));
    }

    let base_segments = base
        .path_segments()
        .map(|segments| {
            segments
                .filter(|segment| !segment.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let remote_segments = split_relative_segments(&settings.remote_dir)?;
    let profile = sanitize_profile(&settings.profile)?;
    let file_name = format!("{}.{}", profile, SNAPSHOT_FILE_EXT);

    let mut collection_urls = Vec::new();
    for index in 1..=remote_segments.len() {
        collection_urls.push(build_url(
            base.clone(),
            &base_segments,
            &remote_segments[..index],
            None,
        )?);
    }
    let mut backup_segments = remote_segments.clone();
    backup_segments.push(format!("{profile}.{BACKUP_DIR_SUFFIX}"));
    collection_urls.push(build_url(
        base.clone(),
        &base_segments,
        &backup_segments,
        None,
    )?);
    let snapshot_url = build_url(base, &base_segments, &remote_segments, Some(&file_name))?;
    let base = Url::parse(&settings.base_url)
        .map_err(|e| AppError::InvalidInput(format!("Invalid WebDAV base URL: {e}")))?;
    let backup_index_url = build_url(
        base,
        &base_segments,
        &backup_segments,
        Some(BACKUP_INDEX_FILE),
    )?;

    Ok(RemoteTarget {
        snapshot_url,
        backup_index_url,
        backup_segments,
        collection_urls,
    })
}

fn split_relative_segments(path: &str) -> Result<Vec<String>, AppError> {
    if path.is_empty() {
        return Ok(Vec::new());
    }
    path.split('/')
        .filter(|segment| !segment.trim().is_empty())
        .map(|segment| {
            let trimmed = segment.trim();
            if trimmed == "." || trimmed == ".." || trimmed.contains('\\') {
                return Err(AppError::InvalidInput(
                    "WebDAV remote directory must be a relative path".into(),
                ));
            }
            Ok(trimmed.to_string())
        })
        .collect()
}

fn sanitize_profile(profile: &str) -> Result<String, AppError> {
    let sanitized = profile
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('.')
        .trim_matches('-')
        .to_string();
    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        return Err(AppError::InvalidInput("Invalid WebDAV profile".into()));
    }
    Ok(sanitized)
}

fn sanitize_backup_id(backup_id: &str) -> Result<String, AppError> {
    let value = backup_id.trim();
    if value.is_empty()
        || value.len() > 96
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Err(AppError::InvalidInput("Invalid WebDAV backup id".into()));
    }
    Ok(value.trim_end_matches(".json").to_string())
}

fn build_url(
    mut base: Url,
    base_segments: &[String],
    remote_segments: &[String],
    file_name: Option<&str>,
) -> Result<Url, AppError> {
    base.set_path("/");
    {
        let mut segments = base
            .path_segments_mut()
            .map_err(|_| AppError::InvalidInput("Invalid WebDAV base URL".into()))?;
        for segment in base_segments.iter().chain(remote_segments.iter()) {
            segments.push(segment);
        }
        match file_name {
            Some(file_name) => {
                segments.push(file_name);
            }
            None => {
                segments.push("");
            }
        }
    }
    Ok(base)
}

fn backup_file_url(
    settings: &WebDavSettings,
    target: &RemoteTarget,
    backup_id: &str,
) -> Result<Url, AppError> {
    let base = Url::parse(&settings.base_url)
        .map_err(|e| AppError::InvalidInput(format!("Invalid WebDAV base URL: {e}")))?;
    let base_segments = base
        .path_segments()
        .map(|segments| {
            segments
                .filter(|segment| !segment.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    build_url(
        base,
        &base_segments,
        &target.backup_segments,
        Some(&format!(
            "{}.{}",
            sanitize_backup_id(backup_id)?,
            SNAPSHOT_FILE_EXT
        )),
    )
}

async fn ensure_remote_collections(
    client: &Client,
    settings: &WebDavSettings,
    urls: &[Url],
) -> Result<(), AppError> {
    let mkcol = Method::from_bytes(b"MKCOL")
        .map_err(|e| AppError::Config(format!("Invalid WebDAV MKCOL method: {e}")))?;
    for url in urls {
        let response = with_auth(client.request(mkcol.clone(), url.clone()), settings)
            .send()
            .await
            .map_err(reqwest_error)?;
        let status = response.status();
        if status.is_success()
            || matches!(
                status,
                StatusCode::METHOD_NOT_ALLOWED | StatusCode::CONFLICT | StatusCode::OK
            )
        {
            continue;
        }
        return Err(status_error("WebDAV MKCOL failed", response).await);
    }
    Ok(())
}

struct DownloadedSnapshot {
    value: Value,
    bytes_len: usize,
    modified_at: Option<String>,
}

async fn preview_snapshot_with_client(
    client: &Client,
    settings: &WebDavSettings,
    url: Url,
) -> Result<Option<WebDavSnapshotPreview>, AppError> {
    let Some(downloaded) = download_snapshot_bytes(client, settings, url.clone()).await? else {
        return Ok(None);
    };
    let parsed = parse_snapshot_value(&downloaded.value)?;
    Ok(Some(build_preview(
        url.as_str(),
        Some(downloaded.bytes_len as u64),
        downloaded.modified_at,
        &parsed,
    )))
}

fn build_snapshot_payload(config: MultiAppConfig) -> Result<SnapshotPayload, AppError> {
    let artifact_list = artifact_list(&config);
    let config_version = config.version;
    let config_hash = config_hash_for_config(&config)?;
    let created_at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let backup_id = created_at
        .replace(':', "")
        .replace('.', "")
        .replace('Z', "z");
    let payload = json!({
        "kind": SNAPSHOT_KIND,
        "appVersion": env!("CARGO_PKG_VERSION"),
        "schemaVersion": SCHEMA_VERSION,
        "snapshotId": backup_id,
        "createdAt": created_at,
        "configHash": config_hash,
        "artifactList": artifact_list,
        "config": config,
    });
    let bytes =
        serde_json::to_vec_pretty(&payload).map_err(|e| AppError::JsonSerialize { source: e })?;

    Ok(SnapshotPayload {
        bytes,
        backup_id,
        created_at,
        artifact_list,
        config_version,
        schema_version: SCHEMA_VERSION,
    })
}

fn local_config_hash(state: &AppState) -> Result<String, AppError> {
    let config = state.load_config()?;
    config_hash_for_config(&config)
}

fn config_hash_for_config(config: &MultiAppConfig) -> Result<String, AppError> {
    let config_bytes =
        serde_json::to_vec(config).map_err(|e| AppError::JsonSerialize { source: e })?;
    Ok(sha256_hex(&config_bytes))
}

fn decide_sync_action(
    local_hash: &str,
    remote_preview: &WebDavSnapshotPreview,
    last_sync_hash: Option<&str>,
) -> &'static str {
    if !remote_preview.exists {
        return "upload";
    }
    if !remote_preview.compatible {
        return "conflict";
    }
    let Some(remote_hash) = remote_preview.config_hash.as_deref() else {
        return "conflict";
    };
    if remote_hash == local_hash {
        return "unchanged";
    }
    let Some(last_hash) = last_sync_hash.filter(|value| !value.trim().is_empty()) else {
        return "conflict";
    };
    let local_changed = local_hash != last_hash;
    let remote_changed = remote_hash != last_hash;
    match (local_changed, remote_changed) {
        (false, true) => "download",
        (true, false) => "upload",
        (false, false) => "unchanged",
        (true, true) => "conflict",
    }
}

async fn update_backup_index(
    client: &Client,
    settings: &WebDavSettings,
    target: &RemoteTarget,
    payload: &SnapshotPayload,
    backup_url: &Url,
    preview: &WebDavSnapshotPreview,
) -> Result<(), AppError> {
    let mut index = load_backup_index(client, settings, target.backup_index_url.clone()).await?;
    index.backups.retain(|entry| entry.id != payload.backup_id);
    index.backups.insert(
        0,
        WebDavBackupEntry {
            id: payload.backup_id.clone(),
            remote_path: backup_url.to_string(),
            size_bytes: Some(payload.bytes.len() as u64),
            modified_at: preview.modified_at.clone(),
            created_at: Some(payload.created_at.clone()),
            artifact_list: payload.artifact_list.clone(),
            config_version: Some(payload.config_version),
            schema_version: Some(payload.schema_version),
            compatible: preview.compatible,
            checks: preview.checks.clone(),
        },
    );
    index.backups.sort_by(|a, b| b.id.cmp(&a.id));
    index.backups.truncate(MAX_BACKUPS);

    let bytes =
        serde_json::to_vec_pretty(&index).map_err(|e| AppError::JsonSerialize { source: e })?;
    let response = with_auth(
        client
            .put(target.backup_index_url.clone())
            .header("content-type", "application/json")
            .body(bytes),
        settings,
    )
    .send()
    .await
    .map_err(reqwest_error)?;
    if !response.status().is_success() {
        return Err(status_error("WebDAV backup index upload failed", response).await);
    }
    Ok(())
}

async fn load_backup_index(
    client: &Client,
    settings: &WebDavSettings,
    url: Url,
) -> Result<BackupIndex, AppError> {
    let response = with_auth(client.get(url), settings)
        .send()
        .await
        .map_err(reqwest_error)?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(BackupIndex::default());
    }
    if !response.status().is_success() {
        return Err(status_error("WebDAV backup index download failed", response).await);
    }
    let bytes =
        read_limited_response_body(response, MAX_INDEX_BYTES, "WebDAV backup index").await?;
    let mut index = serde_json::from_slice::<BackupIndex>(&bytes)
        .map_err(|e| AppError::Config(format!("Invalid WebDAV backup index JSON: {e}")))?;
    index.backups.retain(|backup| !backup.id.trim().is_empty());
    index.backups.sort_by(|a, b| b.id.cmp(&a.id));
    index.backups.truncate(MAX_BACKUPS);
    Ok(index)
}

async fn download_snapshot_bytes(
    client: &Client,
    settings: &WebDavSettings,
    url: Url,
) -> Result<Option<DownloadedSnapshot>, AppError> {
    let response = with_auth(client.get(url), settings)
        .send()
        .await
        .map_err(reqwest_error)?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(status_error("WebDAV download failed", response).await);
    }
    let modified_at = response
        .headers()
        .get(reqwest::header::LAST_MODIFIED)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let bytes = read_limited_response_body(response, MAX_SNAPSHOT_BYTES, "WebDAV snapshot").await?;
    let value = serde_json::from_slice::<Value>(&bytes)
        .map_err(|e| AppError::Config(format!("Invalid WebDAV snapshot JSON: {e}")))?;
    Ok(Some(DownloadedSnapshot {
        value,
        bytes_len: bytes.len(),
        modified_at,
    }))
}

fn parse_snapshot_value(value: &Value) -> Result<ParsedSnapshot, AppError> {
    let is_envelope = value
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind == SNAPSHOT_KIND);
    let config_value = if is_envelope {
        value
            .get("config")
            .cloned()
            .ok_or_else(|| AppError::Config("WebDAV snapshot is missing config".into()))?
    } else {
        value.clone()
    };
    MultiAppConfig::ensure_not_v1_value(&config_value)?;
    let has_skills_in_config = config_value
        .as_object()
        .is_some_and(|map| map.contains_key("skills"));
    let mut config: MultiAppConfig = serde_json::from_value(config_value)
        .map_err(|e| AppError::Config(format!("Invalid WebDAV config snapshot: {e}")))?;
    let _ = config.normalize_after_load(has_skills_in_config)?;
    let artifact_list = if is_envelope {
        value
            .get("artifactList")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .filter(|items| !items.is_empty())
            .unwrap_or_else(|| artifact_list(&config))
    } else {
        artifact_list(&config)
    };
    let schema_version = value
        .get("schemaVersion")
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok());
    let snapshot_id = value
        .get("snapshotId")
        .and_then(Value::as_str)
        .map(str::to_string);
    let created_at = value
        .get("createdAt")
        .and_then(Value::as_str)
        .map(str::to_string);
    let config_hash = value
        .get("configHash")
        .and_then(Value::as_str)
        .map(str::to_string);

    Ok(ParsedSnapshot {
        config,
        artifact_list,
        schema_version,
        snapshot_id,
        created_at,
        config_hash,
    })
}

fn build_preview(
    remote_path: &str,
    size_bytes: Option<u64>,
    modified_at: Option<String>,
    parsed: &ParsedSnapshot,
) -> WebDavSnapshotPreview {
    let mut checks = Vec::new();
    checks.push(WebDavCompatibilityCheck {
        name: "configVersion".to_string(),
        ok: parsed.config.version == 2,
        message: format!("config version {}", parsed.config.version),
    });
    checks.push(WebDavCompatibilityCheck {
        name: "databaseSchema".to_string(),
        ok: parsed
            .schema_version
            .map(|schema_version| schema_version <= SCHEMA_VERSION)
            .unwrap_or(true),
        message: parsed
            .schema_version
            .map(|schema_version| format!("schema {schema_version}, supported {SCHEMA_VERSION}"))
            .unwrap_or_else(|| "schema not declared".to_string()),
    });
    checks.push(WebDavCompatibilityCheck {
        name: "artifacts".to_string(),
        ok: !parsed.artifact_list.is_empty(),
        message: parsed.artifact_list.join(", "),
    });
    let compatible = checks.iter().all(|check| check.ok);

    WebDavSnapshotPreview {
        exists: true,
        remote_path: remote_path.to_string(),
        snapshot_id: parsed.snapshot_id.clone(),
        created_at: parsed.created_at.clone(),
        config_hash: parsed.config_hash.clone(),
        size_bytes,
        modified_at,
        artifact_list: parsed.artifact_list.clone(),
        config_version: Some(parsed.config.version),
        schema_version: parsed.schema_version,
        compatible,
        checks,
    }
}

fn missing_preview(remote_path: &str) -> WebDavSnapshotPreview {
    WebDavSnapshotPreview {
        exists: false,
        remote_path: remote_path.to_string(),
        snapshot_id: None,
        created_at: None,
        config_hash: None,
        size_bytes: None,
        modified_at: None,
        artifact_list: Vec::new(),
        config_version: None,
        schema_version: None,
        compatible: false,
        checks: vec![WebDavCompatibilityCheck {
            name: "exists".to_string(),
            ok: false,
            message: "remote snapshot not found".to_string(),
        }],
    }
}

fn artifact_list(config: &MultiAppConfig) -> Vec<String> {
    let mut artifacts = Vec::new();
    let provider_count: usize = config
        .apps
        .values()
        .map(|manager| manager.providers.len())
        .sum();
    if provider_count > 0 {
        artifacts.push(format!("providers:{provider_count}"));
    }
    let mcp_count = config
        .mcp
        .servers
        .as_ref()
        .map(|servers| servers.len())
        .unwrap_or(0)
        + config.mcp.claude.servers.len()
        + config.mcp.codex.servers.len()
        + config.mcp.gemini.servers.len()
        + config.mcp.opencode.servers.len();
    if mcp_count > 0 {
        artifacts.push(format!("mcp:{mcp_count}"));
    }
    let prompt_count: usize = config.prompts.claude.prompts.len()
        + config.prompts.codex.prompts.len()
        + config.prompts.gemini.prompts.len()
        + config.prompts.opencode.prompts.len();
    if prompt_count > 0 {
        artifacts.push(format!("prompts:{prompt_count}"));
    }
    let skill_count = config.skills.repos.len() + config.skills.skills.len();
    if skill_count > 0 {
        artifacts.push(format!("skills:{skill_count}"));
    }
    if config.common_config_snippets.claude.is_some()
        || config.common_config_snippets.codex.is_some()
        || config.common_config_snippets.gemini.is_some()
    {
        artifacts.push("commonConfigSnippets".to_string());
    }
    if artifacts.is_empty() {
        artifacts.push("emptyConfig".to_string());
    }
    artifacts
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn with_auth(builder: RequestBuilder, settings: &WebDavSettings) -> RequestBuilder {
    if settings.username.is_empty() {
        return builder;
    }
    builder.basic_auth(&settings.username, Some(&settings.password))
}

fn reqwest_error(err: reqwest::Error) -> AppError {
    AppError::Message(format!("WebDAV request failed: {err}"))
}

async fn read_limited_response_body(
    response: reqwest::Response,
    max_bytes: usize,
    label: &str,
) -> Result<Vec<u8>, AppError> {
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(AppError::InvalidInput(format!(
            "{label} exceeds the {max_bytes} byte limit"
        )));
    }

    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(reqwest_error)?;
        if bytes.len().saturating_add(chunk.len()) > max_bytes {
            return Err(AppError::InvalidInput(format!(
                "{label} exceeds the {max_bytes} byte limit"
            )));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn response_body_preview(response: reqwest::Response, max_bytes: usize) -> String {
    let mut bytes = Vec::new();
    let mut truncated = response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64);
    let mut stream = response.bytes_stream();
    while bytes.len() < max_bytes {
        let Some(chunk) = stream.next().await else {
            break;
        };
        let Ok(chunk) = chunk else {
            break;
        };
        let remaining = max_bytes.saturating_sub(bytes.len());
        if chunk.len() > remaining {
            bytes.extend_from_slice(&chunk[..remaining]);
            truncated = true;
            break;
        }
        bytes.extend_from_slice(&chunk);
    }
    let mut body = String::from_utf8_lossy(&bytes).to_string();
    if truncated {
        body.push_str("...");
    }
    body
}

async fn status_error(context: &str, response: reqwest::Response) -> AppError {
    let status = response.status();
    let body = response_body_preview(response, MAX_ERROR_BODY_BYTES).await;
    let detail = body.trim();
    if detail.is_empty() {
        AppError::Message(format!("{context}: HTTP {status}"))
    } else {
        AppError::Message(format!("{context}: HTTP {status}: {detail}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{Provider, ProviderManager};
    use serde_json::json;

    #[test]
    fn remote_target_builds_profile_snapshot_url_and_collection_urls() {
        let settings = WebDavSettings {
            enabled: true,
            base_url: "https://dav.example.com/remote.php/dav/files/me/".to_string(),
            username: "me".to_string(),
            password: "secret".to_string(),
            remote_dir: "/cc-switch-web/prod/".to_string(),
            profile: "main profile".to_string(),
            ..WebDavSettings::default()
        };

        let target = remote_target(&settings).expect("remote target");

        assert_eq!(
            target.snapshot_url.as_str(),
            "https://dav.example.com/remote.php/dav/files/me/cc-switch-web/prod/main-profile.json"
        );
        assert_eq!(target.collection_urls.len(), 3);
        assert_eq!(
            target.collection_urls[0].as_str(),
            "https://dav.example.com/remote.php/dav/files/me/cc-switch-web/"
        );
        assert_eq!(
            target.collection_urls[2].as_str(),
            "https://dav.example.com/remote.php/dav/files/me/cc-switch-web/prod/main-profile.history/"
        );
        assert_eq!(
            target.backup_index_url.as_str(),
            "https://dav.example.com/remote.php/dav/files/me/cc-switch-web/prod/main-profile.history/index.json"
        );
    }

    #[test]
    fn backup_file_url_rejects_path_like_backup_id() {
        let settings = WebDavSettings {
            enabled: true,
            base_url: "https://dav.example.com/remote.php/dav/files/me/".to_string(),
            remote_dir: "cc-switch-web".to_string(),
            profile: "default".to_string(),
            ..WebDavSettings::default()
        };
        let target = remote_target(&settings).expect("remote target");

        let err = backup_file_url(&settings, &target, "../bad").unwrap_err();

        assert!(err.to_string().contains("backup id"));
    }

    #[test]
    fn remote_target_rejects_parent_segments() {
        let settings = WebDavSettings {
            base_url: "https://dav.example.com".to_string(),
            remote_dir: "../bad".to_string(),
            profile: "default".to_string(),
            ..WebDavSettings::default()
        };

        let err = remote_target(&settings).unwrap_err();

        assert!(err.to_string().contains("relative path"));
    }

    #[tokio::test]
    async fn read_limited_response_body_rejects_oversized_body() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let addr = listener.local_addr().expect("test server addr");
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept test client");
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buffer = [0u8; 1024];
            let _ = socket.read(&mut buffer).await;
            let body = b"0123456789abcdef";
            let response = format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n", body.len());
            socket
                .write_all(response.as_bytes())
                .await
                .expect("write headers");
            socket.write_all(body).await.expect("write body");
        });

        let response = reqwest::get(format!("http://{addr}/snapshot.json"))
            .await
            .expect("fetch response");
        let err = read_limited_response_body(response, 8, "WebDAV snapshot")
            .await
            .unwrap_err();

        assert!(err.to_string().contains("exceeds"));
        server.await.expect("server join");
    }

    #[test]
    fn snapshot_preview_reports_artifacts_and_compatibility() {
        let mut config = MultiAppConfig::default();
        let mut manager = ProviderManager::default();
        manager.providers.insert(
            "p1".to_string(),
            Provider::with_id(
                "p1".to_string(),
                "Provider".to_string(),
                json!({ "env": {} }),
                None,
            ),
        );
        config.apps.insert("claude".to_string(), manager);

        let value = json!({
            "kind": SNAPSHOT_KIND,
            "schemaVersion": SCHEMA_VERSION,
            "artifactList": ["providers:1"],
            "config": config,
        });
        let parsed = parse_snapshot_value(&value).expect("snapshot parse");
        let preview = build_preview(
            "https://dav.example.com/default.json",
            Some(100),
            None,
            &parsed,
        );

        assert!(preview.exists);
        assert!(preview.compatible);
        assert_eq!(preview.artifact_list, vec!["providers:1"]);
        assert_eq!(preview.config_version, Some(2));
    }

    #[test]
    fn sync_decision_uploads_when_remote_missing() {
        let remote = missing_preview("https://dav.example.com/default.json");

        assert_eq!(decide_sync_action("local", &remote, None), "upload");
    }

    #[test]
    fn sync_decision_noops_when_hashes_match() {
        let mut remote = missing_preview("https://dav.example.com/default.json");
        remote.exists = true;
        remote.compatible = true;
        remote.config_hash = Some("same".to_string());

        assert_eq!(decide_sync_action("same", &remote, None), "unchanged");
    }

    #[test]
    fn sync_decision_uses_last_sync_hash_for_fast_forward() {
        let mut remote = missing_preview("https://dav.example.com/default.json");
        remote.exists = true;
        remote.compatible = true;
        remote.config_hash = Some("last".to_string());

        assert_eq!(
            decide_sync_action("local-new", &remote, Some("last")),
            "upload"
        );

        remote.config_hash = Some("remote-new".to_string());
        assert_eq!(
            decide_sync_action("last", &remote, Some("last")),
            "download"
        );
    }

    #[test]
    fn sync_decision_conflicts_when_both_sides_changed_or_hash_missing() {
        let mut remote = missing_preview("https://dav.example.com/default.json");
        remote.exists = true;
        remote.compatible = true;

        assert_eq!(
            decide_sync_action("local", &remote, Some("last")),
            "conflict"
        );

        remote.config_hash = Some("remote-new".to_string());
        assert_eq!(decide_sync_action("local-new", &remote, None), "conflict");
        assert_eq!(
            decide_sync_action("local-new", &remote, Some("last")),
            "conflict"
        );
    }

    #[test]
    fn webdav_auto_sync_requires_enabled_auto_sync_and_target() {
        let enabled = WebDavSettings {
            enabled: true,
            auto_sync: true,
            base_url: "https://dav.example.com".to_string(),
            profile: "default".to_string(),
            ..WebDavSettings::default()
        };
        assert!(should_auto_sync(&enabled));

        assert!(!should_auto_sync(&WebDavSettings {
            enabled: false,
            ..enabled.clone()
        }));
        assert!(!should_auto_sync(&WebDavSettings {
            auto_sync: false,
            ..enabled.clone()
        }));
        assert!(!should_auto_sync(&WebDavSettings {
            base_url: "   ".to_string(),
            ..enabled.clone()
        }));
        assert!(!should_auto_sync(&WebDavSettings {
            profile: "   ".to_string(),
            ..enabled
        }));
    }

    #[test]
    fn webdav_auto_sync_marker_prefers_result_preview() {
        let mut remote_preview = missing_preview("https://dav.example.com/default.json");
        remote_preview.config_hash = Some("remote".to_string());
        remote_preview.snapshot_id = Some("remote-id".to_string());

        let mut result_preview = missing_preview("https://dav.example.com/default.json");
        result_preview.config_hash = Some("result".to_string());
        result_preview.snapshot_id = Some("result-id".to_string());

        let result = WebDavAutoSyncResult {
            action: "uploaded".to_string(),
            message: "uploaded".to_string(),
            local_config_hash: "local".to_string(),
            remote_preview: Some(remote_preview),
            result: Some(WebDavSyncResult {
                success: true,
                message: "Snapshot uploaded".to_string(),
                remote_path: "https://dav.example.com/default.json".to_string(),
                backup_id: None,
                preview: Some(result_preview),
            }),
        };

        let preview = sync_marker_preview(&result).expect("marker preview");

        assert_eq!(preview.config_hash.as_deref(), Some("result"));
        assert_eq!(preview.snapshot_id.as_deref(), Some("result-id"));
    }
}
