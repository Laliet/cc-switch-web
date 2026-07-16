#![cfg(feature = "web-server")]

use axum::{
    extract::{Path, Query},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::workspace::{
    self, DailyMemoryDeleteOutcome, DailyMemoryInfo, DailyMemorySearchResult, WorkspaceBackupInfo,
    WorkspaceError, WorkspaceFileContent, WorkspaceFileInfo, WorkspaceWriteOutcome,
};

use super::{ApiError, ApiResult};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WritePayload {
    pub content: String,
    pub expected_etag: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePayload {
    pub backup_id: String,
    pub expected_etag: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyMemorySearchQuery {
    query: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpectedEtagQuery {
    expected_etag: Option<String>,
}

pub async fn list_files() -> ApiResult<Vec<WorkspaceFileInfo>> {
    run_blocking(workspace::list_files).await
}

pub async fn read_file(Path(name): Path<String>) -> ApiResult<WorkspaceFileContent> {
    run_blocking(move || workspace::read_file(&name)).await
}

pub async fn write_file(
    Path(name): Path<String>,
    Json(payload): Json<WritePayload>,
) -> ApiResult<WorkspaceWriteOutcome> {
    run_blocking(move || {
        workspace::write_file(&name, &payload.content, payload.expected_etag.as_deref())
    })
    .await
}

pub async fn list_backups(Path(name): Path<String>) -> ApiResult<Vec<WorkspaceBackupInfo>> {
    run_blocking(move || workspace::list_backups(&name)).await
}

pub async fn restore_backup(
    Path(name): Path<String>,
    Json(payload): Json<RestorePayload>,
) -> ApiResult<WorkspaceWriteOutcome> {
    run_blocking(move || {
        workspace::restore_backup(&name, &payload.backup_id, payload.expected_etag.as_deref())
    })
    .await
}

pub async fn list_daily_memory() -> ApiResult<Vec<DailyMemoryInfo>> {
    run_blocking(workspace::list_daily_memory).await
}

pub async fn read_daily_memory(Path(date): Path<String>) -> ApiResult<WorkspaceFileContent> {
    run_blocking(move || workspace::read_daily_memory(&date)).await
}

pub async fn write_daily_memory(
    Path(date): Path<String>,
    Json(payload): Json<WritePayload>,
) -> ApiResult<WorkspaceWriteOutcome> {
    run_blocking(move || {
        workspace::write_daily_memory(&date, &payload.content, payload.expected_etag.as_deref())
    })
    .await
}

pub async fn search_daily_memory(
    Query(query): Query<DailyMemorySearchQuery>,
) -> ApiResult<Vec<DailyMemorySearchResult>> {
    run_blocking(move || workspace::search_daily_memory(&query.query)).await
}

pub async fn delete_daily_memory(
    Path(date): Path<String>,
    Query(query): Query<ExpectedEtagQuery>,
) -> ApiResult<DailyMemoryDeleteOutcome> {
    run_blocking(move || workspace::delete_daily_memory(&date, query.expected_etag.as_deref()))
        .await
}

async fn run_blocking<T, F>(operation: F) -> ApiResult<T>
where
    T: serde::Serialize + Send + 'static,
    F: FnOnce() -> Result<T, WorkspaceError> + Send + 'static,
{
    let result = tokio::task::spawn_blocking(operation).await.map_err(|e| {
        ApiError::with_code(
            StatusCode::INTERNAL_SERVER_ERROR,
            "workspace_task_failed",
            format!("Workspace task failed: {e}"),
        )
    })?;
    result.map(Json).map_err(map_workspace_error)
}

fn map_workspace_error(error: WorkspaceError) -> ApiError {
    match error {
        WorkspaceError::InvalidInput(message) => {
            ApiError::with_code(StatusCode::BAD_REQUEST, "workspace_invalid_input", message)
        }
        WorkspaceError::NotFound(message) => {
            ApiError::with_code(StatusCode::NOT_FOUND, "workspace_not_found", message)
        }
        WorkspaceError::Conflict(message) => {
            ApiError::with_code(StatusCode::CONFLICT, "workspace_etag_conflict", message)
        }
        WorkspaceError::TooLarge(message) => ApiError::with_code(
            StatusCode::PAYLOAD_TOO_LARGE,
            "workspace_file_too_large",
            message,
        ),
        WorkspaceError::Io(message) => ApiError::with_code(
            StatusCode::INTERNAL_SERVER_ERROR,
            "workspace_io_error",
            message,
        ),
    }
}
