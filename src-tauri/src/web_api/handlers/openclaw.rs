#![cfg(feature = "web-server")]

use std::collections::HashMap;

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use super::{ApiError, ApiResult};
use crate::error::AppError;
use crate::openclaw_config::{
    self, OpenClawAgentsDefaults, OpenClawDefaultModel, OpenClawEnvConfig, OpenClawHealthWarning,
    OpenClawLiveProviderSummary, OpenClawLiveStatus, OpenClawModelCatalogEntry, OpenClawSection,
    OpenClawToolsConfig, OpenClawWriteOutcome,
};
use crate::services::provider::{
    OpenClawReconciliationOutcome, OpenClawReconciliationPreview, ProviderService,
};
use crate::store::AppState;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpectedEtagQuery {
    expected_etag: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionWrite<T> {
    value: T,
    expected_etag: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultModelWrite {
    model: OpenClawDefaultModel,
    expected_etag: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum DefaultModelPayload {
    Wrapped(DefaultModelWrite),
    Legacy(OpenClawDefaultModel),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciliationWrite {
    provider_ids: Vec<String>,
    #[serde(default)]
    update_existing: bool,
    expected_etag: Option<String>,
}

pub async fn get_status() -> ApiResult<OpenClawLiveStatus> {
    Ok(Json(
        openclaw_config::get_live_status().map_err(ApiError::from)?,
    ))
}

pub async fn get_raw_config() -> ApiResult<OpenClawSection<String>> {
    Ok(Json(
        openclaw_config::get_raw_config().map_err(ApiError::from)?,
    ))
}

pub async fn set_raw_config(
    Json(payload): Json<SectionWrite<String>>,
) -> ApiResult<OpenClawWriteOutcome> {
    Ok(Json(
        openclaw_config::set_raw_config(&payload.value, payload.expected_etag.as_deref())
            .map_err(map_openclaw_error)?,
    ))
}

pub async fn get_providers() -> ApiResult<Vec<OpenClawLiveProviderSummary>> {
    Ok(Json(
        openclaw_config::get_live_provider_summaries().map_err(ApiError::from)?,
    ))
}

pub async fn get_provider(
    Path(provider_id): Path<String>,
) -> ApiResult<Option<OpenClawLiveProviderSummary>> {
    Ok(Json(
        openclaw_config::get_live_provider_summary(&provider_id).map_err(ApiError::from)?,
    ))
}

pub async fn preview_reconciliation(
    State(state): State<Arc<AppState>>,
) -> ApiResult<OpenClawReconciliationPreview> {
    Ok(Json(
        ProviderService::preview_openclaw_provider_reconciliation(&state)
            .map_err(map_openclaw_error)?,
    ))
}

pub async fn apply_reconciliation(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ReconciliationWrite>,
) -> ApiResult<OpenClawReconciliationOutcome> {
    Ok(Json(
        ProviderService::apply_openclaw_provider_reconciliation(
            &state,
            &payload.provider_ids,
            payload.update_existing,
            payload.expected_etag.as_deref(),
        )
        .map_err(map_openclaw_error)?,
    ))
}

pub async fn import_live_providers(State(state): State<Arc<AppState>>) -> ApiResult<usize> {
    Ok(Json(
        ProviderService::import_openclaw_providers_from_live(&state).map_err(map_openclaw_error)?,
    ))
}

pub async fn get_default_model() -> ApiResult<Option<OpenClawDefaultModel>> {
    Ok(Json(
        openclaw_config::get_default_model().map_err(ApiError::from)?,
    ))
}

pub async fn set_default_model(
    Json(payload): Json<DefaultModelPayload>,
) -> ApiResult<OpenClawWriteOutcome> {
    let (model, expected_etag) = match payload {
        DefaultModelPayload::Wrapped(payload) => (payload.model, payload.expected_etag),
        DefaultModelPayload::Legacy(model) => (model, None),
    };
    Ok(Json(
        openclaw_config::set_default_model_with_etag(&model, expected_etag.as_deref())
            .map_err(map_openclaw_error)?,
    ))
}

pub async fn clear_default_model(
    Query(query): Query<ExpectedEtagQuery>,
) -> ApiResult<OpenClawWriteOutcome> {
    Ok(Json(
        openclaw_config::clear_default_model_with_etag(query.expected_etag.as_deref())
            .map_err(map_openclaw_error)?,
    ))
}

pub async fn get_model_catalog(
) -> ApiResult<OpenClawSection<Option<HashMap<String, OpenClawModelCatalogEntry>>>> {
    Ok(Json(
        openclaw_config::get_model_catalog().map_err(ApiError::from)?,
    ))
}

pub async fn set_model_catalog(
    Json(payload): Json<SectionWrite<HashMap<String, OpenClawModelCatalogEntry>>>,
) -> ApiResult<OpenClawWriteOutcome> {
    Ok(Json(
        openclaw_config::set_model_catalog(&payload.value, payload.expected_etag.as_deref())
            .map_err(map_openclaw_error)?,
    ))
}

pub async fn get_agents_defaults() -> ApiResult<OpenClawSection<Option<OpenClawAgentsDefaults>>> {
    Ok(Json(
        openclaw_config::get_agents_defaults().map_err(ApiError::from)?,
    ))
}

pub async fn set_agents_defaults(
    Json(payload): Json<SectionWrite<OpenClawAgentsDefaults>>,
) -> ApiResult<OpenClawWriteOutcome> {
    Ok(Json(
        openclaw_config::set_agents_defaults(&payload.value, payload.expected_etag.as_deref())
            .map_err(map_openclaw_error)?,
    ))
}

pub async fn get_env() -> ApiResult<OpenClawSection<OpenClawEnvConfig>> {
    Ok(Json(
        openclaw_config::get_env_config().map_err(ApiError::from)?,
    ))
}

pub async fn set_env(
    Json(payload): Json<SectionWrite<OpenClawEnvConfig>>,
) -> ApiResult<OpenClawWriteOutcome> {
    Ok(Json(
        openclaw_config::set_env_config(&payload.value, payload.expected_etag.as_deref())
            .map_err(map_openclaw_error)?,
    ))
}

pub async fn get_tools() -> ApiResult<OpenClawSection<OpenClawToolsConfig>> {
    Ok(Json(
        openclaw_config::get_tools_config().map_err(ApiError::from)?,
    ))
}

pub async fn set_tools(
    Json(payload): Json<SectionWrite<OpenClawToolsConfig>>,
) -> ApiResult<OpenClawWriteOutcome> {
    Ok(Json(
        openclaw_config::set_tools_config(&payload.value, payload.expected_etag.as_deref())
            .map_err(map_openclaw_error)?,
    ))
}

pub async fn get_health() -> ApiResult<Vec<OpenClawHealthWarning>> {
    Ok(Json(
        openclaw_config::scan_openclaw_config_health().map_err(ApiError::from)?,
    ))
}

fn map_openclaw_error(error: AppError) -> ApiError {
    match error {
        AppError::Conflict(message) => {
            ApiError::with_code(StatusCode::CONFLICT, "openclaw_etag_conflict", message)
        }
        other => ApiError::from(other),
    }
}
