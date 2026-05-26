#![cfg(feature = "web-server")]

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    app_config::AppType,
    database::ModelPricingRecord,
    proxy::{self, ProxyRecentLog, ProxyService, ProxyStatus, ProxyTestResult},
    settings::ProxySettings,
    store::AppState,
};

use super::{parse_app_type, ApiError, ApiResult};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxySettingsPayload {
    pub settings: ProxySettings,
}

pub async fn get_status(State(state): State<Arc<AppState>>) -> ApiResult<ProxyStatus> {
    Ok(Json(proxy::status_for_state(&state).await))
}

pub async fn get_config(State(state): State<Arc<AppState>>) -> ApiResult<ProxySettings> {
    Ok(Json(ProxyService::config(&state).map_err(ApiError::from)?))
}

pub async fn save_config(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ProxySettingsPayload>,
) -> ApiResult<ProxySettings> {
    let config = ProxyService::save_config(&state, payload.settings).map_err(ApiError::from)?;
    if !config.enable_logging {
        proxy::clear_recent_logs().await;
    }
    Ok(Json(config))
}

pub async fn save_settings(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ProxySettingsPayload>,
) -> ApiResult<bool> {
    let config = ProxyService::save_config(&state, payload.settings).map_err(ApiError::from)?;
    if !config.enable_logging {
        proxy::clear_recent_logs().await;
    }
    Ok(Json(true))
}

pub async fn start(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ProxySettingsPayload>,
) -> ApiResult<ProxyStatus> {
    let status = ProxyService::start(state, payload.settings)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(status))
}

pub async fn stop(State(state): State<Arc<AppState>>) -> ApiResult<ProxyStatus> {
    let status = ProxyService::stop(state).await.map_err(ApiError::from)?;
    Ok(Json(status))
}

pub async fn test(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ProxySettingsPayload>,
) -> ApiResult<ProxyTestResult> {
    let result = proxy::test_settings(state, payload.settings)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(result))
}

pub async fn get_takeover(State(state): State<Arc<AppState>>) -> ApiResult<ProxyStatus> {
    Ok(Json(proxy::status_for_state(&state).await))
}

pub async fn set_takeover(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(app): axum::extract::Path<String>,
    Json(payload): Json<TakeoverPayload>,
) -> ApiResult<proxy::ProxyTakeoverResult> {
    let app_type = AppType::parse_supported(&app).map_err(ApiError::from)?;
    let result = ProxyService::set_takeover(state, app_type, payload.enabled)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(result))
}

pub async fn restore(State(state): State<Arc<AppState>>) -> ApiResult<ProxyStatus> {
    let status = ProxyService::restore(state).await.map_err(ApiError::from)?;
    Ok(Json(status))
}

pub async fn recover_stale_takeover(State(state): State<Arc<AppState>>) -> ApiResult<ProxyStatus> {
    let status = ProxyService::recover_stale_takeover(state)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(status))
}

pub async fn recent_logs(State(state): State<Arc<AppState>>) -> ApiResult<Vec<ProxyRecentLog>> {
    Ok(Json(proxy::recent_logs_for_state(&state).await))
}

pub async fn list_model_pricing(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Vec<ModelPricingRecord>> {
    Ok(Json(
        state.db.list_model_pricing().map_err(ApiError::from)?,
    ))
}

pub async fn upsert_model_pricing(
    State(state): State<Arc<AppState>>,
    Path(model_id): Path<String>,
    Json(mut payload): Json<ModelPricingRecord>,
) -> ApiResult<bool> {
    if payload.model_id.trim().is_empty() {
        payload.model_id = model_id;
    } else if payload.model_id != model_id {
        return Err(ApiError::bad_request("model pricing id mismatch"));
    }
    state
        .db
        .upsert_model_pricing(&payload)
        .map_err(ApiError::from)?;
    Ok(Json(true))
}

pub async fn delete_model_pricing(
    State(state): State<Arc<AppState>>,
    Path(model_id): Path<String>,
) -> ApiResult<bool> {
    let deleted = state
        .db
        .delete_model_pricing(&model_id)
        .map_err(ApiError::from)?;
    Ok(Json(deleted))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailoverQueueResponseItem {
    pub provider_id: String,
    pub provider_name: String,
    pub position: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailoverQueuePayload {
    pub provider_ids: Vec<String>,
}

pub async fn get_failover_queue(
    State(state): State<Arc<AppState>>,
    Path(app): Path<String>,
) -> ApiResult<Vec<FailoverQueueResponseItem>> {
    let app_type = parse_app_type(&app)?;
    let items = failover_queue_items(&state, app_type.as_str()).map_err(ApiError::from)?;
    Ok(Json(items))
}

pub async fn replace_failover_queue(
    State(state): State<Arc<AppState>>,
    Path(app): Path<String>,
    Json(payload): Json<FailoverQueuePayload>,
) -> ApiResult<Vec<FailoverQueueResponseItem>> {
    let app_type = parse_app_type(&app)?;
    validate_failover_provider_ids(&state, app_type.as_str(), &payload.provider_ids)
        .map_err(ApiError::from)?;
    state
        .db
        .replace_failover_queue(app_type.as_str(), &payload.provider_ids)
        .map_err(ApiError::from)?;
    let items = failover_queue_items(&state, app_type.as_str()).map_err(ApiError::from)?;
    Ok(Json(items))
}

pub async fn add_failover_provider(
    State(state): State<Arc<AppState>>,
    Path((app, id)): Path<(String, String)>,
) -> ApiResult<Vec<FailoverQueueResponseItem>> {
    let app_type = parse_app_type(&app)?;
    validate_failover_provider_ids(&state, app_type.as_str(), std::slice::from_ref(&id))
        .map_err(ApiError::from)?;
    state
        .db
        .add_failover_provider(app_type.as_str(), &id)
        .map_err(ApiError::from)?;
    let items = failover_queue_items(&state, app_type.as_str()).map_err(ApiError::from)?;
    Ok(Json(items))
}

pub async fn remove_failover_provider(
    State(state): State<Arc<AppState>>,
    Path((app, id)): Path<(String, String)>,
) -> ApiResult<Vec<FailoverQueueResponseItem>> {
    let app_type = parse_app_type(&app)?;
    state
        .db
        .remove_failover_provider(app_type.as_str(), &id)
        .map_err(ApiError::from)?;
    let items = failover_queue_items(&state, app_type.as_str()).map_err(ApiError::from)?;
    Ok(Json(items))
}

pub async fn clear_failover_queue(
    State(state): State<Arc<AppState>>,
    Path(app): Path<String>,
) -> ApiResult<Vec<FailoverQueueResponseItem>> {
    let app_type = parse_app_type(&app)?;
    state
        .db
        .clear_failover_queue(app_type.as_str())
        .map_err(ApiError::from)?;
    Ok(Json(Vec::new()))
}

fn validate_failover_provider_ids(
    state: &AppState,
    app_type: &str,
    provider_ids: &[String],
) -> Result<(), crate::AppError> {
    let config = state.load_config()?;
    let app = crate::AppType::parse_supported(app_type)?;
    let Some(manager) = config.get_manager(&app) else {
        return Err(crate::AppError::InvalidInput(format!(
            "No provider manager found for app '{app_type}'"
        )));
    };
    for provider_id in provider_ids {
        if provider_id.trim().is_empty() {
            return Err(crate::AppError::InvalidInput(
                "Failover provider id cannot be empty".to_string(),
            ));
        }
        if !manager.providers.contains_key(provider_id) {
            return Err(crate::AppError::InvalidInput(format!(
                "Provider '{provider_id}' does not exist for app '{app_type}'"
            )));
        }
    }
    Ok(())
}

fn failover_queue_items(
    state: &AppState,
    app_type: &str,
) -> Result<Vec<FailoverQueueResponseItem>, crate::AppError> {
    let config = state.load_config()?;
    let app = crate::AppType::parse_supported(app_type)?;
    let manager = config.get_manager(&app);
    state
        .db
        .list_failover_queue(app_type)?
        .into_iter()
        .map(|item| {
            let provider_name = manager
                .and_then(|manager| manager.providers.get(&item.provider_id))
                .map(|provider| provider.name.clone())
                .unwrap_or_else(|| item.provider_id.clone());
            Ok(FailoverQueueResponseItem {
                provider_id: item.provider_id,
                provider_name,
                position: item.position,
            })
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TakeoverPayload {
    pub enabled: bool,
}
