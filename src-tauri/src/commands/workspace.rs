#![allow(non_snake_case)]

use crate::workspace::{
    self, DailyMemoryDeleteOutcome, DailyMemoryInfo, DailyMemorySearchResult, WorkspaceBackupInfo,
    WorkspaceFileContent, WorkspaceFileInfo, WorkspaceWriteOutcome,
};

#[tauri::command]
pub async fn list_workspace_files() -> Result<Vec<WorkspaceFileInfo>, String> {
    tauri::async_runtime::spawn_blocking(workspace::list_files)
        .await
        .map_err(|e| format!("Failed to list workspace files: {e}"))?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn read_workspace_file(filename: String) -> Result<WorkspaceFileContent, String> {
    tauri::async_runtime::spawn_blocking(move || workspace::read_file(&filename))
        .await
        .map_err(|e| format!("Failed to read workspace file: {e}"))?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn write_workspace_file(
    filename: String,
    content: String,
    expectedEtag: Option<String>,
) -> Result<WorkspaceWriteOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || {
        workspace::write_file(&filename, &content, expectedEtag.as_deref())
    })
    .await
    .map_err(|e| format!("Failed to write workspace file: {e}"))?
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_workspace_backups(filename: String) -> Result<Vec<WorkspaceBackupInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || workspace::list_backups(&filename))
        .await
        .map_err(|e| format!("Failed to list workspace backups: {e}"))?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_workspace_backup(
    filename: String,
    backupId: String,
    expectedEtag: Option<String>,
) -> Result<WorkspaceWriteOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || {
        workspace::restore_backup(&filename, &backupId, expectedEtag.as_deref())
    })
    .await
    .map_err(|e| format!("Failed to restore workspace backup: {e}"))?
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_daily_memory_files() -> Result<Vec<DailyMemoryInfo>, String> {
    tauri::async_runtime::spawn_blocking(workspace::list_daily_memory)
        .await
        .map_err(|e| format!("Failed to list daily memory files: {e}"))?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn read_daily_memory_file(date: String) -> Result<WorkspaceFileContent, String> {
    tauri::async_runtime::spawn_blocking(move || workspace::read_daily_memory(&date))
        .await
        .map_err(|e| format!("Failed to read daily memory file: {e}"))?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn write_daily_memory_file(
    date: String,
    content: String,
    expectedEtag: Option<String>,
) -> Result<WorkspaceWriteOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || {
        workspace::write_daily_memory(&date, &content, expectedEtag.as_deref())
    })
    .await
    .map_err(|e| format!("Failed to write daily memory file: {e}"))?
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_daily_memory_files(
    query: String,
) -> Result<Vec<DailyMemorySearchResult>, String> {
    tauri::async_runtime::spawn_blocking(move || workspace::search_daily_memory(&query))
        .await
        .map_err(|e| format!("Failed to search daily memory files: {e}"))?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_daily_memory_file(
    date: String,
    expectedEtag: Option<String>,
) -> Result<DailyMemoryDeleteOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || {
        workspace::delete_daily_memory(&date, expectedEtag.as_deref())
    })
    .await
    .map_err(|e| format!("Failed to delete daily memory file: {e}"))?
    .map_err(|e| e.to_string())
}
