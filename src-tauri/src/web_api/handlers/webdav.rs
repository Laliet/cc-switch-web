#![cfg(feature = "web-server")]

use std::sync::Arc;

use axum::{extract::State, Json};
use serde::Deserialize;

use crate::{
    settings::{self, WebDavSettings},
    store::AppState,
    webdav_sync::{
        WebDavAutoSyncResult, WebDavBackupEntry, WebDavSnapshotPreview, WebDavSyncResult,
    },
};

use super::{ApiError, ApiResult};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavSettingsPayload {
    pub settings: Option<WebDavSettings>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavRestorePayload {
    pub settings: Option<WebDavSettings>,
    pub backup_id: String,
}

fn resolve_settings(payload: Option<Json<WebDavSettingsPayload>>) -> WebDavSettings {
    payload
        .and_then(|Json(payload)| payload.settings)
        .unwrap_or_else(|| settings::get_settings().webdav)
}

pub async fn upload_snapshot(
    State(state): State<Arc<AppState>>,
    payload: Option<Json<WebDavSettingsPayload>>,
) -> ApiResult<WebDavSyncResult> {
    let settings = resolve_settings(payload);
    let result = crate::webdav_sync::upload_snapshot(&state, &settings)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(result))
}

pub async fn preview_snapshot(
    State(_state): State<Arc<AppState>>,
    payload: Option<Json<WebDavSettingsPayload>>,
) -> ApiResult<WebDavSnapshotPreview> {
    let settings = resolve_settings(payload);
    let preview = crate::webdav_sync::preview_snapshot(&settings)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(preview))
}

pub async fn download_snapshot(
    State(state): State<Arc<AppState>>,
    payload: Option<Json<WebDavSettingsPayload>>,
) -> ApiResult<WebDavSyncResult> {
    let settings = resolve_settings(payload);
    let result = crate::webdav_sync::download_snapshot(&state, &settings)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(result))
}

pub async fn sync_snapshot(
    State(state): State<Arc<AppState>>,
    payload: Option<Json<WebDavSettingsPayload>>,
) -> ApiResult<WebDavAutoSyncResult> {
    let settings = resolve_settings(payload);
    let result = crate::webdav_sync::sync_snapshot(&state, &settings)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(result))
}

pub async fn list_backups(
    State(_state): State<Arc<AppState>>,
    payload: Option<Json<WebDavSettingsPayload>>,
) -> ApiResult<Vec<WebDavBackupEntry>> {
    let settings = resolve_settings(payload);
    let backups = crate::webdav_sync::list_backups(&settings)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(backups))
}

pub async fn restore_backup(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<WebDavRestorePayload>,
) -> ApiResult<WebDavSyncResult> {
    let settings = resolve_settings(Some(Json(WebDavSettingsPayload {
        settings: payload.settings,
    })));
    let result = crate::webdav_sync::restore_backup(&state, &settings, &payload.backup_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(result))
}
