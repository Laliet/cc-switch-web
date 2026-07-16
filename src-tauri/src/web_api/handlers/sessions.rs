#![cfg(feature = "web-server")]

use axum::{extract::Query, Json};
use serde::Deserialize;

use crate::session_manager::{
    self, DeleteSessionOutcome, DeleteSessionRequest, SessionMessage, SessionMeta, SessionPage,
};

use super::{ApiError, ApiResult};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessagesRequest {
    provider_id: String,
    source_path: String,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionListQuery {
    refresh: Option<bool>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPageQuery {
    cursor: Option<String>,
    limit: Option<usize>,
    provider_id: Option<String>,
    query: Option<String>,
    refresh: Option<bool>,
}

pub async fn list_sessions(Query(query): Query<SessionListQuery>) -> ApiResult<Vec<SessionMeta>> {
    let sessions = tokio::task::spawn_blocking(move || {
        session_manager::scan_sessions_with_refresh(query.refresh.unwrap_or(false))
    })
    .await
    .map_err(|e| {
        ApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to scan sessions: {e}"),
        )
    })?;
    Ok(Json(sessions))
}

pub async fn list_sessions_page(Query(query): Query<SessionPageQuery>) -> ApiResult<SessionPage> {
    let page = tokio::task::spawn_blocking(move || {
        session_manager::scan_sessions_page_with_query(
            query.cursor.as_deref(),
            query.limit.unwrap_or(100),
            query.provider_id.as_deref(),
            query.query.as_deref(),
            query.refresh.unwrap_or(false),
        )
    })
    .await
    .map_err(|e| {
        ApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to scan sessions: {e}"),
        )
    })?
    .map_err(ApiError::bad_request)?;
    Ok(Json(page))
}

pub async fn get_messages(Json(request): Json<MessagesRequest>) -> ApiResult<Vec<SessionMessage>> {
    let messages = tokio::task::spawn_blocking(move || {
        session_manager::load_messages(&request.provider_id, &request.source_path)
    })
    .await
    .map_err(|e| {
        ApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to load session messages: {e}"),
        )
    })?
    .map_err(ApiError::bad_request)?;
    Ok(Json(messages))
}

pub async fn delete_session(Json(request): Json<DeleteSessionRequest>) -> ApiResult<bool> {
    let deleted = tokio::task::spawn_blocking(move || {
        session_manager::delete_session(
            &request.provider_id,
            &request.session_id,
            &request.source_path,
        )
    })
    .await
    .map_err(|e| {
        ApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to delete session: {e}"),
        )
    })?
    .map_err(ApiError::bad_request)?;
    Ok(Json(deleted))
}

pub async fn delete_sessions(
    Json(items): Json<Vec<DeleteSessionRequest>>,
) -> ApiResult<Vec<DeleteSessionOutcome>> {
    let outcomes = tokio::task::spawn_blocking(move || session_manager::delete_sessions(&items))
        .await
        .map_err(|e| {
            ApiError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to delete sessions: {e}"),
            )
        })?;
    Ok(Json(outcomes))
}
