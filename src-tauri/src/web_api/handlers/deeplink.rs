#![cfg(feature = "web-server")]

use std::sync::Arc;

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{ApiError, ApiResult};
use crate::{
    deeplink::{
        import_mcp_from_deeplink, import_prompt_from_deeplink, import_provider_from_deeplink,
        import_skill_from_deeplink, parse_and_merge_config, parse_deeplink_url,
        DeepLinkImportRequest,
    },
    store::AppState,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseDeepLinkPayload {
    pub url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepLinkImportResult {
    #[serde(rename = "type")]
    pub resource_type: String,
    pub id: Option<String>,
    pub result: serde_json::Value,
}

pub async fn parse_deeplink(
    Json(payload): Json<ParseDeepLinkPayload>,
) -> ApiResult<DeepLinkImportRequest> {
    let parsed = parse_deeplink_url(&payload.url)
        .and_then(|request| parse_and_merge_config(&request))
        .map_err(ApiError::from)?;
    Ok(Json(parsed))
}

pub async fn merge_deeplink_config(
    Json(request): Json<DeepLinkImportRequest>,
) -> ApiResult<DeepLinkImportRequest> {
    let merged = parse_and_merge_config(&request).map_err(ApiError::from)?;
    Ok(Json(merged))
}

pub async fn import_from_deeplink(
    State(state): State<Arc<AppState>>,
    Json(request): Json<DeepLinkImportRequest>,
) -> ApiResult<DeepLinkImportResult> {
    match request.resource.as_str() {
        "provider" => {
            let id = import_provider_from_deeplink(&state, request).map_err(ApiError::from)?;
            Ok(Json(DeepLinkImportResult {
                resource_type: "provider".to_string(),
                id: Some(id.clone()),
                result: json!({ "id": id }),
            }))
        }
        "prompt" => {
            let id = import_prompt_from_deeplink(&state, request).map_err(ApiError::from)?;
            Ok(Json(DeepLinkImportResult {
                resource_type: "prompt".to_string(),
                id: Some(id.clone()),
                result: json!({ "id": id }),
            }))
        }
        "mcp" => {
            let result = import_mcp_from_deeplink(&state, request).map_err(ApiError::from)?;
            Ok(Json(DeepLinkImportResult {
                resource_type: "mcp".to_string(),
                id: None,
                result: serde_json::to_value(result).map_err(|err| {
                    ApiError::new(
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        err.to_string(),
                    )
                })?,
            }))
        }
        "skill" => {
            let key = import_skill_from_deeplink(&state, request).map_err(ApiError::from)?;
            Ok(Json(DeepLinkImportResult {
                resource_type: "skill".to_string(),
                id: Some(key.clone()),
                result: json!({ "key": key }),
            }))
        }
        _ => Err(ApiError::bad_request(format!(
            "Unsupported resource type: {}",
            request.resource
        ))),
    }
}
