#![allow(non_snake_case)]

use tauri::State;

use crate::{settings::WebDavSettings, store::AppState};

fn resolve_settings(settings: Option<WebDavSettings>) -> WebDavSettings {
    settings.unwrap_or_else(|| crate::settings::get_settings().webdav)
}

#[tauri::command]
pub async fn upload_webdav_snapshot(
    settings: Option<WebDavSettings>,
    state: State<'_, AppState>,
) -> Result<crate::webdav_sync::WebDavSyncResult, String> {
    let settings = resolve_settings(settings);
    crate::webdav_sync::upload_snapshot(state.inner(), &settings)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn preview_webdav_snapshot(
    settings: Option<WebDavSettings>,
) -> Result<crate::webdav_sync::WebDavSnapshotPreview, String> {
    let settings = resolve_settings(settings);
    crate::webdav_sync::preview_snapshot(&settings)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn download_webdav_snapshot(
    settings: Option<WebDavSettings>,
    state: State<'_, AppState>,
) -> Result<crate::webdav_sync::WebDavSyncResult, String> {
    let settings = resolve_settings(settings);
    crate::webdav_sync::download_snapshot(state.inner(), &settings)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn sync_webdav_snapshot(
    settings: Option<WebDavSettings>,
    state: State<'_, AppState>,
) -> Result<crate::webdav_sync::WebDavAutoSyncResult, String> {
    let settings = resolve_settings(settings);
    crate::webdav_sync::sync_snapshot(state.inner(), &settings)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_webdav_backups(
    settings: Option<WebDavSettings>,
) -> Result<Vec<crate::webdav_sync::WebDavBackupEntry>, String> {
    let settings = resolve_settings(settings);
    crate::webdav_sync::list_backups(&settings)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_webdav_backup(
    settings: Option<WebDavSettings>,
    #[allow(non_snake_case)] backupId: String,
    state: State<'_, AppState>,
) -> Result<crate::webdav_sync::WebDavSyncResult, String> {
    let settings = resolve_settings(settings);
    crate::webdav_sync::restore_backup(state.inner(), &settings, &backupId)
        .await
        .map_err(|e| e.to_string())
}
