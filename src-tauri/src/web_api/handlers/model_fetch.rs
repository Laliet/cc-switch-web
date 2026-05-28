#![cfg(feature = "web-server")]

use axum::Json;
use serde::Deserialize;

use crate::services::model_fetch::{self, FetchedModel};

use super::{ApiError, ApiResult};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchModelsPayload {
    pub base_url: String,
    pub api_key: String,
    pub npm: Option<String>,
    pub is_full_url: Option<bool>,
    pub models_url: Option<String>,
}

pub async fn fetch_models_for_config(
    Json(payload): Json<FetchModelsPayload>,
) -> ApiResult<Vec<FetchedModel>> {
    let models = model_fetch::fetch_models(
        &payload.base_url,
        &payload.api_key,
        payload.npm.as_deref(),
        payload.is_full_url.unwrap_or(false),
        payload.models_url.as_deref(),
    )
    .await
    .map_err(ApiError::bad_request)?;
    Ok(Json(models))
}
