#![cfg(feature = "web-server")]

use std::sync::Arc;

use axum::{extract::State, Json};
use serde::Deserialize;

use crate::{
    settings::{self, WebDavSettings},
    store::AppState,
    webdav_sync::{WebDavSnapshotPreview, WebDavSyncResult},
};

use super::{ApiError, ApiResult};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavSettingsPayload {
    pub settings: Option<WebDavSettings>,
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
