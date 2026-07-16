#![allow(non_snake_case)]

use crate::session_manager;

#[tauri::command]
pub async fn list_sessions(
    refresh: Option<bool>,
) -> Result<Vec<session_manager::SessionMeta>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        session_manager::scan_sessions_with_refresh(refresh.unwrap_or(false))
    })
    .await
    .map_err(|e| format!("Failed to scan sessions: {e}"))
}

#[tauri::command]
pub async fn list_sessions_page(
    cursor: Option<String>,
    limit: Option<usize>,
    providerId: Option<String>,
    query: Option<String>,
    refresh: Option<bool>,
) -> Result<session_manager::SessionPage, String> {
    tauri::async_runtime::spawn_blocking(move || {
        session_manager::scan_sessions_page_with_query(
            cursor.as_deref(),
            limit.unwrap_or(100),
            providerId.as_deref(),
            query.as_deref(),
            refresh.unwrap_or(false),
        )
    })
    .await
    .map_err(|e| format!("Failed to scan sessions: {e}"))?
}

#[tauri::command]
pub async fn get_session_messages(
    providerId: String,
    sourcePath: String,
) -> Result<Vec<session_manager::SessionMessage>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        session_manager::load_messages(&providerId, &sourcePath)
    })
    .await
    .map_err(|e| format!("Failed to load session messages: {e}"))?
}

#[tauri::command]
pub async fn delete_session(
    providerId: String,
    sessionId: String,
    sourcePath: String,
) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        session_manager::delete_session(&providerId, &sessionId, &sourcePath)
    })
    .await
    .map_err(|e| format!("Failed to delete session: {e}"))?
}

#[tauri::command]
pub async fn delete_sessions(
    items: Vec<session_manager::DeleteSessionRequest>,
) -> Result<Vec<session_manager::DeleteSessionOutcome>, String> {
    tauri::async_runtime::spawn_blocking(move || session_manager::delete_sessions(&items))
        .await
        .map_err(|e| format!("Failed to delete sessions: {e}"))
}
