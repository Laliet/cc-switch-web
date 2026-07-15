#![cfg(feature = "web-server")]

use axum::{extract::Path, Json};

use super::{ApiError, ApiResult};
use crate::openclaw_config::{
    self, OpenClawDefaultModel, OpenClawHealthWarning, OpenClawLiveProviderSummary,
    OpenClawLiveStatus, OpenClawWriteOutcome,
};

pub async fn get_status() -> ApiResult<OpenClawLiveStatus> {
    Ok(Json(
        openclaw_config::get_live_status().map_err(ApiError::from)?,
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

pub async fn get_default_model() -> ApiResult<Option<OpenClawDefaultModel>> {
    Ok(Json(
        openclaw_config::get_default_model().map_err(ApiError::from)?,
    ))
}

pub async fn set_default_model(
    Json(model): Json<OpenClawDefaultModel>,
) -> ApiResult<OpenClawWriteOutcome> {
    Ok(Json(
        openclaw_config::set_default_model(&model).map_err(ApiError::from)?,
    ))
}

pub async fn clear_default_model() -> ApiResult<OpenClawWriteOutcome> {
    Ok(Json(
        openclaw_config::clear_default_model().map_err(ApiError::from)?,
    ))
}

pub async fn get_health() -> ApiResult<Vec<OpenClawHealthWarning>> {
    Ok(Json(
        openclaw_config::scan_openclaw_config_health().map_err(ApiError::from)?,
    ))
}
