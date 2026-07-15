#![cfg(feature = "web-server")]

use std::{str::FromStr, sync::Arc};

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::{
    app_config::AppType,
    database::StreamCheckLogFilters,
    services::{
        stream_check::{StreamCheckConfig, StreamCheckLog, StreamCheckResult, StreamCheckService},
        ProviderService,
    },
    store::AppState,
};

use super::{ApiError, ApiResult};

fn parse_stream_check_app(value: &str) -> Result<AppType, ApiError> {
    let app_type =
        AppType::from_str(value).map_err(|error| ApiError::bad_request(error.to_string()))?;
    StreamCheckService::ensure_app_supported(&app_type).map_err(|error| {
        ApiError::not_implemented(
            format!(
                "stream_check_{}_unavailable",
                app_type.as_str().replace('-', "_")
            ),
            error.to_string(),
        )
    })?;
    Ok(app_type)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamCheckAllPayload {
    pub app_type: String,
    pub proxy_targets_only: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamCheckProviderPayload {
    pub app_type: String,
}

async fn check_provider(
    state: &AppState,
    app_type: AppType,
    id: &str,
) -> Result<StreamCheckResult, ApiError> {
    let providers = ProviderService::list(state, app_type.clone()).map_err(ApiError::from)?;
    let provider = providers
        .get(id)
        .ok_or_else(|| ApiError::bad_request(format!("Provider {id} not found")))?;
    let config = StreamCheckService::get_config(state).map_err(ApiError::from)?;
    let result = StreamCheckService::check_with_retry(&app_type, provider, &config).await;
    StreamCheckService::record_result(state, &app_type, provider, &result)
        .map_err(ApiError::from)?;
    Ok(result)
}

pub async fn stream_check_provider(
    State(state): State<Arc<AppState>>,
    Path((app, id)): Path<(String, String)>,
) -> ApiResult<StreamCheckResult> {
    let app_type = parse_stream_check_app(&app)?;
    Ok(Json(check_provider(&state, app_type, &id).await?))
}

pub async fn stream_check_provider_by_id(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<StreamCheckProviderPayload>,
) -> ApiResult<StreamCheckResult> {
    let app_type = parse_stream_check_app(&payload.app_type)?;
    Ok(Json(check_provider(&state, app_type, &id).await?))
}

pub async fn stream_check_all_providers(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<StreamCheckAllPayload>,
) -> ApiResult<Vec<(String, StreamCheckResult)>> {
    let app_type = parse_stream_check_app(&payload.app_type)?;
    let providers = ProviderService::list(&state, app_type.clone()).map_err(ApiError::from)?;
    let config = StreamCheckService::get_config(&state).map_err(ApiError::from)?;
    let current = if payload.proxy_targets_only.unwrap_or(false) {
        Some(ProviderService::current(&state, app_type.clone()).map_err(ApiError::from)?)
    } else {
        None
    };

    let mut results = Vec::new();
    for (id, provider) in providers {
        if current.as_ref().is_some_and(|current_id| current_id != &id) {
            continue;
        }
        let result = StreamCheckService::check_with_retry(&app_type, &provider, &config).await;
        StreamCheckService::record_result(&state, &app_type, &provider, &result)
            .map_err(ApiError::from)?;
        results.push((id, result));
    }
    Ok(Json(results))
}

pub async fn get_stream_check_config(
    State(state): State<Arc<AppState>>,
) -> ApiResult<StreamCheckConfig> {
    Ok(Json(
        StreamCheckService::get_config(&state).map_err(ApiError::from)?,
    ))
}

pub async fn save_stream_check_config(
    State(state): State<Arc<AppState>>,
    Json(config): Json<StreamCheckConfig>,
) -> ApiResult<bool> {
    StreamCheckService::save_config(&state, &config).map_err(ApiError::from)?;
    Ok(Json(true))
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StreamCheckLogQuery {
    pub app_type: Option<String>,
    pub provider_id: Option<String>,
    pub status: Option<String>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

fn into_filters(query: StreamCheckLogQuery) -> StreamCheckLogFilters {
    StreamCheckLogFilters {
        app_type: query.app_type.filter(|value| !value.trim().is_empty()),
        provider_id: query.provider_id.filter(|value| !value.trim().is_empty()),
        status: query.status.filter(|value| !value.trim().is_empty()),
        since: query.since,
        until: query.until,
        limit: query.limit.unwrap_or(100),
        offset: query.offset.unwrap_or(0),
    }
}

pub async fn get_stream_check_logs(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StreamCheckLogQuery>,
) -> ApiResult<Vec<StreamCheckLog>> {
    Ok(Json(
        StreamCheckService::list_logs(&state, into_filters(query)).map_err(ApiError::from)?,
    ))
}

pub async fn get_latest_stream_check_logs(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LatestStreamCheckLogQuery>,
) -> ApiResult<Vec<StreamCheckLog>> {
    Ok(Json(
        StreamCheckService::latest_logs(&state, query.app_type.as_deref())
            .map_err(ApiError::from)?,
    ))
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LatestStreamCheckLogQuery {
    pub app_type: Option<String>,
}
