use crate::settings;
use crate::{
    app_config::MultiAppConfig, database::SCHEMA_VERSION, error::AppError, services::ConfigService,
    settings::WebDavSettings, store::AppState,
};
use chrono::{SecondsFormat, Utc};
use futures::StreamExt;
use reqwest::{Client, Method, RequestBuilder, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, OnceLock,
};
use std::time::Duration;
use tokio::time::MissedTickBehavior;

mod archive;
use archive::{
    backup_current_skills, restore_skills_from_backup, restore_skills_zip, zip_skills_ssot,
};

const PROTOCOL_FORMAT: &str = "cc-switch-webdav-sync";
const PROTOCOL_VERSION: u32 = 2;
const DB_COMPAT_VERSION: u32 = SCHEMA_VERSION as u32;
const REMOTE_DB_SQL: &str = "db.sql";
const REMOTE_SKILLS_ZIP: &str = "skills.zip";
const REMOTE_MANIFEST: &str = "manifest.json";
const HISTORY_DIR: &str = "history";
const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_SYNC_ARTIFACT_BYTES: u64 = 512 * 1024 * 1024;
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
    manifest_url: Url,
    db_url: Url,
    skills_url: Url,
    legacy_manifest_url: Option<Url>,
    legacy_db_url: Option<Url>,
    legacy_skills_url: Option<Url>,
    snapshot_url: Url,
    backup_index_url: Url,
    backup_segments: Vec<String>,
    previous_schema_backup_index_url: Option<Url>,
    previous_schema_backup_segments: Option<Vec<String>>,
    legacy_backup_index_url: Url,
    legacy_backup_segments: Vec<String>,
    collection_urls: Vec<Url>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncManifest {
    format: String,
    version: u32,
    db_compat_version: u32,
    device_name: String,
    created_at: String,
    artifacts: BTreeMap<String, ArtifactMeta>,
    snapshot_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ArtifactMeta {
    sha256: String,
    size: u64,
}

struct V2SnapshotPayload {
    db_sql: Vec<u8>,
    skills_zip: Vec<u8>,
    manifest: SyncManifest,
    manifest_bytes: Vec<u8>,
}

struct DownloadedManifest {
    manifest: SyncManifest,
    modified_at: Option<String>,
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

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupIndex {
    #[serde(default)]
    backups: Vec<WebDavBackupEntry>,
}

fn sync_mutex() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

pub async fn upload_snapshot(
    state: &AppState,
    settings: &WebDavSettings,
) -> Result<WebDavSyncResult, AppError> {
    let _sync_guard = sync_mutex().lock().await;
    let settings = normalized_settings(settings)?;
    let target = remote_target(&settings)?;
    let client = webdav_client()?;
    ensure_remote_collections(&client, &settings, &target.collection_urls).await?;

    let payload = build_v2_snapshot_payload(state)?;
    let backup_id = payload.manifest.snapshot_id.clone();
    let backup_segments = backup_snapshot_segments(&target, &backup_id)?;
    let backup_collection = collection_url(&settings, &backup_segments)?;
    ensure_remote_collections(&client, &settings, &[backup_collection]).await?;

    // Artifacts are uploaded first. The manifest is the commit marker.
    put_bytes(
        &client,
        &settings,
        target.db_url.clone(),
        payload.db_sql.clone(),
        "application/sql",
    )
    .await?;
    put_bytes(
        &client,
        &settings,
        target.skills_url.clone(),
        payload.skills_zip.clone(),
        "application/zip",
    )
    .await?;
    put_bytes(
        &client,
        &settings,
        target.manifest_url.clone(),
        payload.manifest_bytes.clone(),
        "application/json",
    )
    .await?;

    let backup_db_url = artifact_url(&settings, &backup_segments, REMOTE_DB_SQL)?;
    let backup_skills_url = artifact_url(&settings, &backup_segments, REMOTE_SKILLS_ZIP)?;
    let backup_manifest_url = artifact_url(&settings, &backup_segments, REMOTE_MANIFEST)?;
    put_bytes(
        &client,
        &settings,
        backup_db_url,
        payload.db_sql,
        "application/sql",
    )
    .await?;
    put_bytes(
        &client,
        &settings,
        backup_skills_url,
        payload.skills_zip,
        "application/zip",
    )
    .await?;
    put_bytes(
        &client,
        &settings,
        backup_manifest_url.clone(),
        payload.manifest_bytes,
        "application/json",
    )
    .await?;

    let preview = preview_from_manifest(
        target.manifest_url.as_str(),
        Some(
            payload
                .manifest
                .artifacts
                .values()
                .map(|meta| meta.size)
                .sum(),
        ),
        None,
        &payload.manifest,
    );
    update_v2_backup_index(
        &client,
        &settings,
        &target,
        &payload.manifest,
        &backup_manifest_url,
        &preview,
    )
    .await?;

    Ok(WebDavSyncResult {
        success: true,
        message: "WebDAV v2 snapshot uploaded".to_string(),
        remote_path: target.manifest_url.to_string(),
        backup_id: Some(backup_id),
        preview: Some(preview),
    })
}

pub async fn preview_snapshot(
    settings: &WebDavSettings,
) -> Result<WebDavSnapshotPreview, AppError> {
    let settings = normalized_settings(settings)?;
    let target = remote_target(&settings)?;
    let client = webdav_client()?;
    if let Some(manifest) =
        download_manifest(&client, &settings, target.manifest_url.clone()).await?
    {
        return Ok(preview_from_manifest(
            target.manifest_url.as_str(),
            Some(
                manifest
                    .manifest
                    .artifacts
                    .values()
                    .map(|meta| meta.size)
                    .sum(),
            ),
            manifest.modified_at,
            &manifest.manifest,
        ));
    }
    if let Some(legacy_manifest_url) = target.legacy_manifest_url.clone() {
        if let Some(manifest) =
            download_manifest(&client, &settings, legacy_manifest_url.clone()).await?
        {
            return Ok(preview_from_manifest(
                legacy_manifest_url.as_str(),
                Some(
                    manifest
                        .manifest
                        .artifacts
                        .values()
                        .map(|meta| meta.size)
                        .sum(),
                ),
                manifest.modified_at,
                &manifest.manifest,
            ));
        }
    }
    Ok(
        preview_snapshot_with_client(&client, &settings, target.snapshot_url.clone())
            .await?
            .unwrap_or_else(|| missing_preview(target.manifest_url.as_str())),
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
    crate::webdav_auto_sync::start_worker(state.clone());
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
                Err(err) => {
                    let _ = persist_sync_state("error", Some(err.to_string()));
                    log::warn!("WebDAV auto sync failed: {err}");
                }
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

pub(crate) async fn auto_sync_upload_if_enabled(state: &AppState) -> Result<(), AppError> {
    let webdav_settings = settings::get_settings().webdav;
    if !should_auto_sync(&webdav_settings) {
        return Ok(());
    }
    persist_sync_state("syncing", None)?;
    match upload_snapshot(state, &webdav_settings).await {
        Ok(result) => {
            let marker = WebDavAutoSyncResult {
                action: "uploaded".to_string(),
                message: result.message.clone(),
                local_config_hash: result
                    .preview
                    .as_ref()
                    .and_then(|preview| preview.config_hash.clone())
                    .unwrap_or_default(),
                remote_preview: result.preview.clone(),
                result: Some(result),
            };
            persist_sync_marker_from_result(&marker)?;
            persist_sync_state("success", None)
        }
        Err(error) => {
            let _ = persist_sync_state("error", Some(error.to_string()));
            Err(error)
        }
    }
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
    app_settings.webdav.last_sync_status = "success".to_string();
    app_settings.webdav.last_sync_error = None;
    settings::update_settings(app_settings)
}

fn persist_sync_state(status: &str, error: Option<String>) -> Result<(), AppError> {
    let mut app_settings = settings::get_settings();
    app_settings.webdav.last_sync_status = status.to_string();
    app_settings.webdav.last_sync_error = error;
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
    let mut backups = load_backup_index(&client, &settings, target.backup_index_url.clone())
        .await?
        .backups;
    if let Some(previous_index_url) = target.previous_schema_backup_index_url.clone() {
        let previous = load_backup_index(&client, &settings, previous_index_url)
            .await?
            .backups;
        for entry in previous {
            if !backups.iter().any(|current| current.id == entry.id) {
                backups.push(entry);
            }
        }
    }
    let legacy = load_backup_index(&client, &settings, target.legacy_backup_index_url.clone())
        .await?
        .backups;
    for entry in legacy {
        if !backups.iter().any(|current| current.id == entry.id) {
            backups.push(entry);
        }
    }
    backups.sort_by(|left, right| right.id.cmp(&left.id));
    backups.truncate(MAX_BACKUPS);
    Ok(backups)
}

pub async fn restore_backup(
    state: &AppState,
    settings: &WebDavSettings,
    backup_id: &str,
) -> Result<WebDavSyncResult, AppError> {
    let _sync_guard = sync_mutex().lock().await;
    let backup_id = sanitize_backup_id(backup_id)?;
    let settings = normalized_settings(settings)?;
    let target = remote_target(&settings)?;
    let client = webdav_client()?;
    let index = load_backup_index(&client, &settings, target.backup_index_url.clone()).await?;
    if let Some(entry) = index
        .backups
        .iter()
        .find(|backup| backup.id == backup_id)
        .cloned()
    {
        return restore_v2_backup(
            state,
            &client,
            &settings,
            &target.backup_segments,
            entry,
            "WebDAV v2 backup restored",
        )
        .await;
    }

    if let (Some(previous_index_url), Some(previous_segments)) = (
        target.previous_schema_backup_index_url.clone(),
        target.previous_schema_backup_segments.as_deref(),
    ) {
        let previous_index = load_backup_index(&client, &settings, previous_index_url).await?;
        if let Some(entry) = previous_index
            .backups
            .iter()
            .find(|backup| backup.id == backup_id)
            .cloned()
        {
            return restore_v2_backup(
                state,
                &client,
                &settings,
                previous_segments,
                entry,
                "WebDAV v2 backup restored from previous schema",
            )
            .await;
        }
    }

    let legacy_index =
        load_backup_index(&client, &settings, target.legacy_backup_index_url.clone()).await?;
    let entry = legacy_index
        .backups
        .iter()
        .find(|backup| backup.id == backup_id)
        .cloned()
        .ok_or_else(|| AppError::InvalidInput("WebDAV backup was not found".into()))?;

    let backup_url = legacy_backup_file_url(&settings, &target, &backup_id)?;
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

    let local_backup_id = {
        let _guard = crate::webdav_auto_sync::AutoSyncSuppressionGuard::new();
        ConfigService::apply_import_config(parsed.config, state)?
    };
    Ok(WebDavSyncResult {
        success: true,
        message: "Backup restored".to_string(),
        remote_path: entry.remote_path,
        backup_id: Some(local_backup_id),
        preview: Some(preview),
    })
}

async fn restore_v2_backup(
    state: &AppState,
    client: &Client,
    settings: &WebDavSettings,
    backup_segments: &[String],
    entry: WebDavBackupEntry,
    message: &str,
) -> Result<WebDavSyncResult, AppError> {
    let mut segments = backup_segments.to_vec();
    segments.push(sanitize_backup_id(&entry.id)?);
    let manifest_url = artifact_url(settings, &segments, REMOTE_MANIFEST)?;
    let manifest = download_manifest(client, settings, manifest_url.clone())
        .await?
        .ok_or_else(|| {
            AppError::InvalidInput("Remote WebDAV backup manifest was not found".into())
        })?;
    let preview = preview_from_manifest(
        manifest_url.as_str(),
        Some(
            manifest
                .manifest
                .artifacts
                .values()
                .map(|meta| meta.size)
                .sum(),
        ),
        manifest.modified_at,
        &manifest.manifest,
    );
    ensure_preview_compatible(&preview)?;
    let db_sql = download_and_verify(
        client,
        settings,
        artifact_url(settings, &segments, REMOTE_DB_SQL)?,
        REMOTE_DB_SQL,
        &manifest.manifest.artifacts,
    )
    .await?;
    let skills_zip = download_and_verify(
        client,
        settings,
        artifact_url(settings, &segments, REMOTE_SKILLS_ZIP)?,
        REMOTE_SKILLS_ZIP,
        &manifest.manifest.artifacts,
    )
    .await?;
    let local_backup_id = apply_v2_snapshot(state, &db_sql, &skills_zip)?;
    Ok(WebDavSyncResult {
        success: true,
        message: message.to_string(),
        remote_path: entry.remote_path,
        backup_id: Some(local_backup_id),
        preview: Some(preview),
    })
}

pub async fn download_snapshot(
    state: &AppState,
    settings: &WebDavSettings,
) -> Result<WebDavSyncResult, AppError> {
    let _sync_guard = sync_mutex().lock().await;
    let settings = normalized_settings(settings)?;
    let target = remote_target(&settings)?;
    let client = webdav_client()?;
    if let Some(result) = download_v2_snapshot_from_urls(
        state,
        &client,
        &settings,
        target.manifest_url.clone(),
        target.db_url.clone(),
        target.skills_url.clone(),
        "WebDAV v2 snapshot downloaded",
    )
    .await?
    {
        return Ok(result);
    }
    if let (Some(manifest_url), Some(db_url), Some(skills_url)) = (
        target.legacy_manifest_url.clone(),
        target.legacy_db_url.clone(),
        target.legacy_skills_url.clone(),
    ) {
        if let Some(result) = download_v2_snapshot_from_urls(
            state,
            &client,
            &settings,
            manifest_url,
            db_url,
            skills_url,
            "WebDAV v2 snapshot downloaded from previous schema",
        )
        .await?
        {
            return Ok(result);
        }
    }

    // 0.18.x snapshots remain read-only compatible.
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

    let backup_id = {
        let _guard = crate::webdav_auto_sync::AutoSyncSuppressionGuard::new();
        ConfigService::apply_import_config(parsed.config, state)?
    };
    Ok(WebDavSyncResult {
        success: true,
        message: "Snapshot downloaded".to_string(),
        remote_path: target.snapshot_url.to_string(),
        backup_id: Some(backup_id),
        preview: Some(preview),
    })
}

async fn download_v2_snapshot_from_urls(
    state: &AppState,
    client: &Client,
    settings: &WebDavSettings,
    manifest_url: Url,
    db_url: Url,
    skills_url: Url,
    message: &str,
) -> Result<Option<WebDavSyncResult>, AppError> {
    let Some(manifest) = download_manifest(client, settings, manifest_url.clone()).await? else {
        return Ok(None);
    };
    let preview = preview_from_manifest(
        manifest_url.as_str(),
        Some(
            manifest
                .manifest
                .artifacts
                .values()
                .map(|meta| meta.size)
                .sum(),
        ),
        manifest.modified_at,
        &manifest.manifest,
    );
    ensure_preview_compatible(&preview)?;
    let db_sql = download_and_verify(
        client,
        settings,
        db_url,
        REMOTE_DB_SQL,
        &manifest.manifest.artifacts,
    )
    .await?;
    let skills_zip = download_and_verify(
        client,
        settings,
        skills_url,
        REMOTE_SKILLS_ZIP,
        &manifest.manifest.artifacts,
    )
    .await?;
    let backup_id = apply_v2_snapshot(state, &db_sql, &skills_zip)?;
    Ok(Some(WebDavSyncResult {
        success: true,
        message: message.to_string(),
        remote_path: manifest_url.to_string(),
        backup_id: Some(backup_id),
        preview: Some(preview),
    }))
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
    let mut current_segments = remote_segments.clone();
    current_segments.extend([
        format!("v{PROTOCOL_VERSION}"),
        format!("db-v{DB_COMPAT_VERSION}"),
        profile.clone(),
    ]);
    for index in (remote_segments.len() + 1)..=current_segments.len() {
        collection_urls.push(build_url(
            base.clone(),
            &base_segments,
            &current_segments[..index],
            None,
        )?);
    }
    let mut backup_segments = current_segments.clone();
    backup_segments.push(HISTORY_DIR.to_string());
    collection_urls.push(build_url(
        base.clone(),
        &base_segments,
        &backup_segments,
        None,
    )?);

    let mut legacy_backup_segments = remote_segments.clone();
    legacy_backup_segments.push(format!("{profile}.{BACKUP_DIR_SUFFIX}"));
    let snapshot_url = build_url(
        base.clone(),
        &base_segments,
        &remote_segments,
        Some(&file_name),
    )?;
    let manifest_url = build_url(
        base.clone(),
        &base_segments,
        &current_segments,
        Some(REMOTE_MANIFEST),
    )?;
    let db_url = build_url(
        base.clone(),
        &base_segments,
        &current_segments,
        Some(REMOTE_DB_SQL),
    )?;
    let skills_url = build_url(
        base.clone(),
        &base_segments,
        &current_segments,
        Some(REMOTE_SKILLS_ZIP),
    )?;
    let backup_index_url = build_url(
        base.clone(),
        &base_segments,
        &backup_segments,
        Some(BACKUP_INDEX_FILE),
    )?;

    // Keep one read-only fallback for the previous v2 schema directory. New
    // uploads always use the current schema path, but existing v6 snapshots
    // must remain restorable after the v7 migration.
    let legacy_schema = DB_COMPAT_VERSION.saturating_sub(1);
    let legacy_segments = if legacy_schema > 0 {
        let mut segments = remote_segments.clone();
        segments.extend([
            format!("v{PROTOCOL_VERSION}"),
            format!("db-v{legacy_schema}"),
            profile.clone(),
        ]);
        Some(segments)
    } else {
        None
    };
    let legacy_manifest_url = legacy_segments
        .as_deref()
        .map(|segments| {
            build_url(
                base.clone(),
                &base_segments,
                segments,
                Some(REMOTE_MANIFEST),
            )
        })
        .transpose()?;
    let legacy_db_url = legacy_segments
        .as_deref()
        .map(|segments| build_url(base.clone(), &base_segments, segments, Some(REMOTE_DB_SQL)))
        .transpose()?;
    let legacy_skills_url = legacy_segments
        .as_deref()
        .map(|segments| {
            build_url(
                base.clone(),
                &base_segments,
                segments,
                Some(REMOTE_SKILLS_ZIP),
            )
        })
        .transpose()?;
    let legacy_backup_index_url = build_url(
        base.clone(),
        &base_segments,
        &legacy_backup_segments,
        Some(BACKUP_INDEX_FILE),
    )?;

    let previous_schema_backup_segments = legacy_segments.as_ref().map(|segments| {
        let mut backup_segments = segments.clone();
        backup_segments.push(HISTORY_DIR.to_string());
        backup_segments
    });
    let previous_schema_backup_index_url = previous_schema_backup_segments
        .as_deref()
        .map(|segments| {
            build_url(
                base.clone(),
                &base_segments,
                segments,
                Some(BACKUP_INDEX_FILE),
            )
        })
        .transpose()?;

    Ok(RemoteTarget {
        manifest_url,
        db_url,
        skills_url,
        legacy_manifest_url,
        legacy_db_url,
        legacy_skills_url,
        snapshot_url,
        backup_index_url,
        backup_segments,
        previous_schema_backup_index_url,
        previous_schema_backup_segments,
        legacy_backup_index_url,
        legacy_backup_segments,
        collection_urls,
    })
}

fn collection_url(settings: &WebDavSettings, segments: &[String]) -> Result<Url, AppError> {
    let base = Url::parse(&settings.base_url)
        .map_err(|error| AppError::InvalidInput(format!("Invalid WebDAV base URL: {error}")))?;
    let base_segments = base
        .path_segments()
        .map(|segments| {
            segments
                .filter(|segment| !segment.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    build_url(base, &base_segments, segments, None)
}

fn artifact_url(
    settings: &WebDavSettings,
    segments: &[String],
    artifact: &str,
) -> Result<Url, AppError> {
    let base = Url::parse(&settings.base_url)
        .map_err(|error| AppError::InvalidInput(format!("Invalid WebDAV base URL: {error}")))?;
    let base_segments = base
        .path_segments()
        .map(|segments| {
            segments
                .filter(|segment| !segment.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    build_url(base, &base_segments, segments, Some(artifact))
}

fn backup_snapshot_segments(
    target: &RemoteTarget,
    backup_id: &str,
) -> Result<Vec<String>, AppError> {
    let mut segments = target.backup_segments.clone();
    segments.push(sanitize_backup_id(backup_id)?);
    Ok(segments)
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

fn legacy_backup_file_url(
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
        &target.legacy_backup_segments,
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

fn build_v2_snapshot_payload(state: &AppState) -> Result<V2SnapshotPayload, AppError> {
    let db_sql = state.db.export_sql_string_for_sync()?.into_bytes();
    let temporary = tempfile::tempdir().map_err(|source| AppError::IoContext {
        context: "Failed to create WebDAV snapshot directory".to_string(),
        source,
    })?;
    let skills_path = temporary.path().join(REMOTE_SKILLS_ZIP);
    zip_skills_ssot(&skills_path)?;
    let skills_zip = fs::read(&skills_path).map_err(|error| AppError::io(&skills_path, error))?;
    validate_artifact_size(REMOTE_DB_SQL, db_sql.len() as u64)?;
    validate_artifact_size(REMOTE_SKILLS_ZIP, skills_zip.len() as u64)?;

    let mut artifacts = BTreeMap::new();
    artifacts.insert(
        REMOTE_DB_SQL.to_string(),
        ArtifactMeta {
            sha256: sha256_hex(&db_sql),
            size: db_sql.len() as u64,
        },
    );
    artifacts.insert(
        REMOTE_SKILLS_ZIP.to_string(),
        ArtifactMeta {
            sha256: sha256_hex(&skills_zip),
            size: skills_zip.len() as u64,
        },
    );
    let snapshot_id = compute_snapshot_id(&artifacts);
    let manifest = SyncManifest {
        format: PROTOCOL_FORMAT.to_string(),
        version: PROTOCOL_VERSION,
        db_compat_version: DB_COMPAT_VERSION,
        device_name: device_name(),
        created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        artifacts,
        snapshot_id,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|source| AppError::JsonSerialize { source })?;
    Ok(V2SnapshotPayload {
        db_sql,
        skills_zip,
        manifest,
        manifest_bytes,
    })
}

fn compute_snapshot_id(artifacts: &BTreeMap<String, ArtifactMeta>) -> String {
    let identity = artifacts
        .iter()
        .map(|(name, meta)| format!("{name}:{}", meta.sha256))
        .collect::<Vec<_>>()
        .join("|");
    sha256_hex(identity.as_bytes())
}

fn device_name() -> String {
    ["CC_SWITCH_DEVICE_NAME", "HOSTNAME", "COMPUTERNAME"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .map(|value| value.split_whitespace().collect::<Vec<_>>().join(" "))
        .find(|value| !value.is_empty())
        .map(|value| value.chars().take(64).collect())
        .unwrap_or_else(|| "Unknown Device".to_string())
}

async fn put_bytes(
    client: &Client,
    settings: &WebDavSettings,
    url: Url,
    bytes: Vec<u8>,
    content_type: &str,
) -> Result<(), AppError> {
    let response = with_auth(
        client
            .put(url)
            .header("content-type", content_type)
            .body(bytes),
        settings,
    )
    .send()
    .await
    .map_err(reqwest_error)?;
    if !response.status().is_success() {
        return Err(status_error("WebDAV upload failed", response).await);
    }
    Ok(())
}

async fn download_manifest(
    client: &Client,
    settings: &WebDavSettings,
    url: Url,
) -> Result<Option<DownloadedManifest>, AppError> {
    let response = with_auth(client.get(url), settings)
        .send()
        .await
        .map_err(reqwest_error)?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(status_error("WebDAV manifest download failed", response).await);
    }
    let modified_at = response
        .headers()
        .get(reqwest::header::LAST_MODIFIED)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let bytes = read_limited_response_body(response, MAX_MANIFEST_BYTES, "WebDAV manifest").await?;
    let manifest = serde_json::from_slice::<SyncManifest>(&bytes)
        .map_err(|error| AppError::Config(format!("Invalid WebDAV manifest JSON: {error}")))?;
    Ok(Some(DownloadedManifest {
        manifest,
        modified_at,
    }))
}

async fn download_and_verify(
    client: &Client,
    settings: &WebDavSettings,
    url: Url,
    artifact_name: &str,
    artifacts: &BTreeMap<String, ArtifactMeta>,
) -> Result<Vec<u8>, AppError> {
    let metadata = artifacts.get(artifact_name).ok_or_else(|| {
        AppError::InvalidInput(format!("WebDAV manifest is missing {artifact_name}"))
    })?;
    validate_artifact_size(artifact_name, metadata.size)?;
    let response = with_auth(client.get(url), settings)
        .send()
        .await
        .map_err(reqwest_error)?;
    if response.status() == StatusCode::NOT_FOUND {
        return Err(AppError::InvalidInput(format!(
            "WebDAV artifact was not found: {artifact_name}"
        )));
    }
    if !response.status().is_success() {
        return Err(status_error("WebDAV artifact download failed", response).await);
    }
    let bytes = read_limited_response_body(
        response,
        MAX_SYNC_ARTIFACT_BYTES as usize,
        "WebDAV artifact",
    )
    .await?;
    if bytes.len() as u64 != metadata.size {
        return Err(AppError::InvalidInput(format!(
            "WebDAV artifact size mismatch for {artifact_name}: expected {}, got {}",
            metadata.size,
            bytes.len()
        )));
    }
    let hash = sha256_hex(&bytes);
    if hash != metadata.sha256 {
        return Err(AppError::InvalidInput(format!(
            "WebDAV artifact SHA256 mismatch for {artifact_name}"
        )));
    }
    Ok(bytes)
}

fn validate_artifact_size(name: &str, size: u64) -> Result<(), AppError> {
    if size > MAX_SYNC_ARTIFACT_BYTES {
        return Err(AppError::InvalidInput(format!(
            "WebDAV artifact {name} exceeds the 512 MiB limit"
        )));
    }
    Ok(())
}

fn preview_from_manifest(
    remote_path: &str,
    size_bytes: Option<u64>,
    modified_at: Option<String>,
    manifest: &SyncManifest,
) -> WebDavSnapshotPreview {
    let expected_snapshot_id = compute_snapshot_id(&manifest.artifacts);
    let checks = vec![
        WebDavCompatibilityCheck {
            name: "protocolFormat".to_string(),
            ok: manifest.format == PROTOCOL_FORMAT,
            message: format!("format {}", manifest.format),
        },
        WebDavCompatibilityCheck {
            name: "protocolVersion".to_string(),
            ok: manifest.version == PROTOCOL_VERSION,
            message: format!(
                "protocol {}, supported {}",
                manifest.version, PROTOCOL_VERSION
            ),
        },
        WebDavCompatibilityCheck {
            name: "databaseSchema".to_string(),
            ok: manifest.db_compat_version > 0 && manifest.db_compat_version <= DB_COMPAT_VERSION,
            message: format!(
                "db-v{}, supported up to db-v{}",
                manifest.db_compat_version, DB_COMPAT_VERSION
            ),
        },
        WebDavCompatibilityCheck {
            name: "artifacts".to_string(),
            ok: [REMOTE_DB_SQL, REMOTE_SKILLS_ZIP]
                .iter()
                .all(|name| manifest.artifacts.contains_key(*name)),
            message: manifest
                .artifacts
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
        },
        WebDavCompatibilityCheck {
            name: "snapshotId".to_string(),
            ok: !manifest.snapshot_id.is_empty() && manifest.snapshot_id == expected_snapshot_id,
            message: if manifest.snapshot_id == expected_snapshot_id {
                "snapshot identity matches artifacts".to_string()
            } else {
                "snapshot identity does not match artifacts".to_string()
            },
        },
        WebDavCompatibilityCheck {
            name: "artifactSize".to_string(),
            ok: manifest
                .artifacts
                .iter()
                .all(|(name, meta)| validate_artifact_size(name, meta.size).is_ok()),
            message: "maximum 512 MiB per artifact".to_string(),
        },
    ];
    WebDavSnapshotPreview {
        exists: true,
        remote_path: remote_path.to_string(),
        snapshot_id: Some(manifest.snapshot_id.clone()),
        created_at: Some(manifest.created_at.clone()),
        config_hash: Some(manifest.snapshot_id.clone()),
        size_bytes,
        modified_at,
        artifact_list: manifest.artifacts.keys().cloned().collect(),
        config_version: Some(manifest.version),
        schema_version: i32::try_from(manifest.db_compat_version).ok(),
        compatible: checks.iter().all(|check| check.ok),
        checks,
    }
}

fn ensure_preview_compatible(preview: &WebDavSnapshotPreview) -> Result<(), AppError> {
    if preview.compatible {
        Ok(())
    } else {
        let failures = preview
            .checks
            .iter()
            .filter(|check| !check.ok)
            .map(|check| check.message.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        Err(AppError::InvalidInput(format!(
            "Remote WebDAV snapshot is incompatible: {failures}"
        )))
    }
}

fn apply_v2_snapshot(
    state: &AppState,
    db_sql: &[u8],
    skills_zip: &[u8],
) -> Result<String, AppError> {
    let sql = std::str::from_utf8(db_sql)
        .map_err(|error| AppError::InvalidInput(format!("WebDAV db.sql is not UTF-8: {error}")))?;
    let skills_backup = backup_current_skills()?;
    restore_skills_zip(skills_zip)?;
    match state.db.import_sql_string_for_sync(sql) {
        Ok(backup_id) => Ok(backup_id),
        Err(database_error) => {
            restore_skills_from_backup(&skills_backup).map_err(|rollback_error| {
                AppError::Config(format!(
                    "Database restore failed: {database_error}; Skills rollback failed: {rollback_error}"
                ))
            })?;
            Err(database_error)
        }
    }
}

fn local_config_hash(state: &AppState) -> Result<String, AppError> {
    Ok(build_v2_snapshot_payload(state)?.manifest.snapshot_id)
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

async fn update_v2_backup_index(
    client: &Client,
    settings: &WebDavSettings,
    target: &RemoteTarget,
    manifest: &SyncManifest,
    backup_manifest_url: &Url,
    preview: &WebDavSnapshotPreview,
) -> Result<(), AppError> {
    let mut index = load_backup_index(client, settings, target.backup_index_url.clone()).await?;
    index
        .backups
        .retain(|entry| entry.id != manifest.snapshot_id);
    index.backups.insert(
        0,
        WebDavBackupEntry {
            id: manifest.snapshot_id.clone(),
            remote_path: backup_manifest_url.to_string(),
            size_bytes: Some(manifest.artifacts.values().map(|meta| meta.size).sum()),
            modified_at: preview.modified_at.clone(),
            created_at: Some(manifest.created_at.clone()),
            artifact_list: manifest.artifacts.keys().cloned().collect(),
            config_version: Some(manifest.version),
            schema_version: i32::try_from(manifest.db_compat_version).ok(),
            compatible: preview.compatible,
            checks: preview.checks.clone(),
        },
    );
    index.backups.sort_by(|left, right| right.id.cmp(&left.id));
    index.backups.truncate(MAX_BACKUPS);
    let bytes =
        serde_json::to_vec_pretty(&index).map_err(|source| AppError::JsonSerialize { source })?;
    put_bytes(
        client,
        settings,
        target.backup_index_url.clone(),
        bytes,
        "application/json",
    )
    .await
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
            target.manifest_url.as_str(),
            "https://dav.example.com/remote.php/dav/files/me/cc-switch-web/prod/v2/db-v7/main-profile/manifest.json"
        );
        assert_eq!(
            target.snapshot_url.as_str(),
            "https://dav.example.com/remote.php/dav/files/me/cc-switch-web/prod/main-profile.json"
        );
        assert_eq!(target.collection_urls.len(), 6);
        assert_eq!(
            target.collection_urls[0].as_str(),
            "https://dav.example.com/remote.php/dav/files/me/cc-switch-web/"
        );
        assert_eq!(
            target.collection_urls[5].as_str(),
            "https://dav.example.com/remote.php/dav/files/me/cc-switch-web/prod/v2/db-v7/main-profile/history/"
        );
        assert_eq!(
            target.backup_index_url.as_str(),
            "https://dav.example.com/remote.php/dav/files/me/cc-switch-web/prod/v2/db-v7/main-profile/history/index.json"
        );
        assert_eq!(
            target
                .previous_schema_backup_index_url
                .as_ref()
                .expect("previous schema history index")
                .as_str(),
            "https://dav.example.com/remote.php/dav/files/me/cc-switch-web/prod/v2/db-v6/main-profile/history/index.json"
        );
        assert_eq!(
            target
                .previous_schema_backup_segments
                .as_ref()
                .expect("previous schema history segments"),
            &[
                "cc-switch-web".to_string(),
                "prod".to_string(),
                "v2".to_string(),
                "db-v6".to_string(),
                "main-profile".to_string(),
                "history".to_string(),
            ]
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

        let err = backup_snapshot_segments(&target, "../bad").unwrap_err();

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
    fn legacy_config_snapshot_without_schema_remains_compatible() {
        let value = serde_json::to_value(MultiAppConfig::default()).expect("serialize config");

        let parsed = parse_snapshot_value(&value).expect("parse legacy snapshot");
        let preview = build_preview("legacy.json", Some(1), None, &parsed);

        assert!(preview.compatible);
        assert_eq!(preview.schema_version, None);
    }

    #[test]
    fn v2_snapshot_id_is_deterministic_for_artifact_hashes() {
        let mut first = BTreeMap::new();
        first.insert(
            REMOTE_DB_SQL.to_string(),
            ArtifactMeta {
                sha256: "db-hash".to_string(),
                size: 10,
            },
        );
        first.insert(
            REMOTE_SKILLS_ZIP.to_string(),
            ArtifactMeta {
                sha256: "skills-hash".to_string(),
                size: 20,
            },
        );
        let second = first.clone();

        assert_eq!(compute_snapshot_id(&first), compute_snapshot_id(&second));

        first.get_mut(REMOTE_DB_SQL).expect("db artifact").sha256 = "changed".to_string();
        assert_ne!(compute_snapshot_id(&first), compute_snapshot_id(&second));
    }

    #[test]
    fn v2_preview_rejects_protocol_database_and_artifact_mismatches() {
        let mut artifacts = BTreeMap::new();
        artifacts.insert(
            REMOTE_DB_SQL.to_string(),
            ArtifactMeta {
                sha256: "db".to_string(),
                size: 10,
            },
        );
        artifacts.insert(
            REMOTE_SKILLS_ZIP.to_string(),
            ArtifactMeta {
                sha256: "skills".to_string(),
                size: 20,
            },
        );
        let mut manifest = SyncManifest {
            format: PROTOCOL_FORMAT.to_string(),
            version: PROTOCOL_VERSION,
            db_compat_version: DB_COMPAT_VERSION,
            device_name: "test".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            snapshot_id: compute_snapshot_id(&artifacts),
            artifacts,
        };
        assert!(preview_from_manifest("manifest.json", None, None, &manifest).compatible);

        manifest.version += 1;
        assert!(!preview_from_manifest("manifest.json", None, None, &manifest).compatible);
        manifest.version = PROTOCOL_VERSION;
        manifest.db_compat_version += 1;
        assert!(!preview_from_manifest("manifest.json", None, None, &manifest).compatible);
        manifest.db_compat_version = DB_COMPAT_VERSION;

        manifest.db_compat_version = DB_COMPAT_VERSION.saturating_sub(1);
        assert!(preview_from_manifest("manifest.json", None, None, &manifest).compatible);
        manifest.db_compat_version = DB_COMPAT_VERSION;

        manifest.snapshot_id = "mismatched-snapshot-id".to_string();
        assert!(!preview_from_manifest("manifest.json", None, None, &manifest).compatible);
        manifest.snapshot_id = compute_snapshot_id(&manifest.artifacts);

        manifest.artifacts.remove(REMOTE_SKILLS_ZIP);
        assert!(!preview_from_manifest("manifest.json", None, None, &manifest).compatible);
    }

    #[tokio::test]
    async fn preview_falls_back_from_current_schema_to_v6_manifest() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind WebDAV mock");
        let address = listener.local_addr().expect("mock address");
        let mut artifacts = BTreeMap::new();
        artifacts.insert(
            REMOTE_DB_SQL.to_string(),
            ArtifactMeta {
                sha256: "db-v6-hash".to_string(),
                size: 10,
            },
        );
        artifacts.insert(
            REMOTE_SKILLS_ZIP.to_string(),
            ArtifactMeta {
                sha256: "skills-v6-hash".to_string(),
                size: 20,
            },
        );
        let previous_schema = DB_COMPAT_VERSION.saturating_sub(1);
        let manifest = SyncManifest {
            format: PROTOCOL_FORMAT.to_string(),
            version: PROTOCOL_VERSION,
            db_compat_version: previous_schema,
            device_name: "v6-device".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            snapshot_id: compute_snapshot_id(&artifacts),
            artifacts,
        };
        let manifest_body = serde_json::to_vec(&manifest).expect("serialize v6 manifest");
        let previous_path = format!("/db-v{previous_schema}/");
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut socket, _) = listener.accept().await.expect("accept WebDAV request");
                let mut request = [0u8; 4096];
                let length = socket.read(&mut request).await.expect("read request");
                let request = String::from_utf8_lossy(&request[..length]);
                let (status, body) = if request.contains(&previous_path) {
                    ("200 OK", manifest_body.as_slice())
                } else {
                    ("404 Not Found", &[][..])
                };
                let headers = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                socket
                    .write_all(headers.as_bytes())
                    .await
                    .expect("write response headers");
                socket.write_all(body).await.expect("write response body");
            }
        });
        let settings = WebDavSettings {
            enabled: true,
            base_url: format!("http://{address}"),
            remote_dir: "sync".to_string(),
            profile: "default".to_string(),
            ..WebDavSettings::default()
        };

        let preview = preview_snapshot(&settings)
            .await
            .expect("preview v6 fallback");

        assert!(preview.exists);
        assert!(preview.compatible);
        assert_eq!(preview.schema_version, i32::try_from(previous_schema).ok());
        assert!(preview
            .remote_path
            .contains(&format!("db-v{previous_schema}")));
        server.await.expect("WebDAV mock join");
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
