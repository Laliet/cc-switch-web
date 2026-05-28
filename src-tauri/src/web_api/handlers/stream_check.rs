#![cfg(feature = "web-server")]

use std::{str::FromStr, sync::Arc};

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;

use crate::{
    app_config::AppType,
    services::{
        stream_check::{StreamCheckConfig, StreamCheckResult, StreamCheckService},
        ProviderService,
    },
    store::AppState,
};

use super::{ApiError, ApiResult};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamCheckAllPayload {
    pub app_type: String,
    pub proxy_targets_only: Option<bool>,
}

pub async fn stream_check_provider(
    State(state): State<Arc<AppState>>,
    Path((app, id)): Path<(String, String)>,
) -> ApiResult<StreamCheckResult> {
    let app_type = AppType::from_str(&app).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let providers = ProviderService::list(&state, app_type.clone()).map_err(ApiError::from)?;
    let provider = providers
        .get(&id)
        .ok_or_else(|| ApiError::bad_request(format!("Provider {id} not found")))?;
    let config = StreamCheckService::get_config(&state).map_err(ApiError::from)?;
    Ok(Json(
        StreamCheckService::check_with_retry(&app_type, provider, &config).await,
    ))
}

pub async fn stream_check_all_providers(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<StreamCheckAllPayload>,
) -> ApiResult<Vec<(String, StreamCheckResult)>> {
    let app_type =
        AppType::from_str(&payload.app_type).map_err(|e| ApiError::bad_request(e.to_string()))?;
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
