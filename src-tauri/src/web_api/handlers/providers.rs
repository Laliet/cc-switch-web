#![cfg(feature = "web-server")]

use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;

use super::{parse_known_app_type, ApiError, ApiResult};
use crate::{
    error::AppError,
    provider::{Provider, UsageResult},
    services::provider::ProviderSortUpdate,
    services::ConfigService,
    services::ProviderService,
    store::AppState,
};

#[derive(Debug, Deserialize)]
pub struct ProviderPath {
    pub app: String,
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum SortOrderPayload {
    Wrapped { updates: Vec<ProviderSortUpdate> },
    Direct(Vec<ProviderSortUpdate>),
}

pub async fn list_providers(
    State(state): State<Arc<AppState>>,
    Path(app): Path<String>,
) -> ApiResult<HashMap<String, Provider>> {
    let app_type = parse_known_app_type(&app)?;
    let providers = ProviderService::list(&state, app_type).map_err(ApiError::from)?;
    Ok(Json(providers))
}

pub async fn current_provider(
    State(state): State<Arc<AppState>>,
    Path(app): Path<String>,
) -> ApiResult<String> {
    let app_type = parse_known_app_type(&app)?;
    let current = ProviderService::current(&state, app_type).map_err(ApiError::from)?;
    Ok(Json(current))
}

pub async fn backup_provider(
    State(state): State<Arc<AppState>>,
    Path(app): Path<String>,
) -> ApiResult<Option<String>> {
    let app_type = parse_known_app_type(&app)?;
    let backup = ProviderService::backup(&state, app_type).map_err(ApiError::from)?;
    Ok(Json(backup))
}

#[derive(Deserialize)]
pub struct BackupPayload {
    pub id: Option<String>,
}

pub async fn set_backup_provider(
    State(state): State<Arc<AppState>>,
    Path(app): Path<String>,
    Json(payload): Json<BackupPayload>,
) -> ApiResult<bool> {
    let app_type = parse_known_app_type(&app)?;
    ProviderService::set_backup(&state, app_type, payload.id).map_err(ApiError::from)?;
    Ok(Json(true))
}

pub async fn add_provider(
    State(state): State<Arc<AppState>>,
    Path(app): Path<String>,
    Json(provider): Json<Provider>,
) -> ApiResult<bool> {
    let app_type = parse_known_app_type(&app)?;
    let created = ProviderService::add(&state, app_type, provider).map_err(ApiError::from)?;
    Ok(Json(created))
}

pub async fn update_provider(
    State(state): State<Arc<AppState>>,
    Path(path): Path<ProviderPath>,
    Json(mut provider): Json<Provider>,
) -> ApiResult<bool> {
    let app_type = parse_known_app_type(&path.app)?;
    if provider.id.is_empty() {
        provider.id = path.id.clone();
    } else if provider.id != path.id {
        return Err(ApiError::bad_request("provider id mismatch"));
    }

    let updated = ProviderService::update(&state, app_type, provider).map_err(ApiError::from)?;
    Ok(Json(updated))
}

pub async fn delete_provider(
    State(state): State<Arc<AppState>>,
    Path(path): Path<ProviderPath>,
) -> ApiResult<bool> {
    let app_type = parse_known_app_type(&path.app)?;
    ProviderService::delete(&state, app_type, &path.id).map_err(ApiError::from)?;
    Ok(Json(true))
}

pub async fn switch_provider(
    State(state): State<Arc<AppState>>,
    Path(path): Path<ProviderPath>,
) -> ApiResult<bool> {
    let app_type = parse_known_app_type(&path.app)?;
    ProviderService::switch(&state, app_type, &path.id).map_err(ApiError::from)?;
    Ok(Json(true))
}

pub async fn import_default_config(
    State(state): State<Arc<AppState>>,
    Path(app): Path<String>,
) -> ApiResult<bool> {
    let app_type = parse_known_app_type(&app)?;
    match ProviderService::import_default_config(&state, app_type) {
        Ok(_) => Ok(Json(true)),
        Err(err) => {
            log::warn!("Import default config for {app} failed: {err}");
            // 前端依赖返回值判定是否需要提示，这里返回 false 避免 400
            Ok(Json(false))
        }
    }
}

pub async fn read_live_provider_settings(
    State(state): State<Arc<AppState>>,
    Path(app): Path<String>,
) -> ApiResult<serde_json::Value> {
    let app_type = parse_known_app_type(&app)?;
    let live_settings =
        ProviderService::read_live_settings(app_type.clone()).map_err(ApiError::from)?;
    ProviderService::sync_default_provider_from_live(&state, app_type, live_settings.clone())
        .map_err(ApiError::from)?;
    Ok(Json(live_settings))
}

pub async fn update_sort_order(
    State(state): State<Arc<AppState>>,
    Path(app): Path<String>,
    Json(payload): Json<SortOrderPayload>,
) -> ApiResult<bool> {
    let app_type = parse_known_app_type(&app)?;
    let updates = match payload {
        SortOrderPayload::Wrapped { updates } => updates,
        SortOrderPayload::Direct(updates) => updates,
    };

    ProviderService::update_sort_order(&state, app_type, updates).map_err(ApiError::from)?;
    Ok(Json(true))
}

pub async fn query_provider_usage(
    State(state): State<Arc<AppState>>,
    Path((app, id)): Path<(String, String)>,
) -> ApiResult<UsageResult> {
    let app_type = parse_known_app_type(&app)?;
    let result = ProviderService::query_usage(&state, app_type, &id).await;
    match result {
        Ok(r) => Ok(Json(r)),
        Err(err) => Ok(Json(UsageResult {
            success: false,
            data: None,
            error: Some(err.to_string()),
        })),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestUsageScriptRequest {
    script_code: String,
    timeout: Option<u64>,
    api_key: Option<String>,
    base_url: Option<String>,
    access_token: Option<String>,
    user_id: Option<String>,
}

pub async fn test_usage_script(
    State(state): State<Arc<AppState>>,
    Path((app, id)): Path<(String, String)>,
    Json(req): Json<TestUsageScriptRequest>,
) -> ApiResult<UsageResult> {
    let app_type = parse_known_app_type(&app)?;
    let result = ProviderService::test_usage_script(
        &state,
        app_type,
        &id,
        &req.script_code,
        req.timeout.unwrap_or(10),
        req.api_key.as_deref(),
        req.base_url.as_deref(),
        req.access_token.as_deref(),
        req.user_id.as_deref(),
    )
    .await;
    match result {
        Ok(r) => Ok(Json(r)),
        Err(err) => Ok(Json(UsageResult {
            success: false,
            data: None,
            error: Some(err.to_string()),
        })),
    }
}

/// 将当前供应商写入对应应用的 live 配置文件。
pub async fn sync_current_providers_live(
    State(state): State<Arc<AppState>>,
) -> ApiResult<serde_json::Value> {
    {
        let mut config_guard = state
            .config
            .write()
            .map_err(AppError::from)
            .map_err(ApiError::from)?;
        ConfigService::sync_current_providers_to_live(&mut config_guard).map_err(ApiError::from)?;
    }
    state.save().map_err(ApiError::from)?;
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Live configuration synchronized"
    })))
}
