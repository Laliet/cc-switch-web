#![cfg(feature = "web-server")]

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};

use crate::{prompt::Prompt, services::PromptService, store::AppState};

use super::{parse_app_feature_type, ApiError, ApiResult};

pub async fn list_prompts(
    State(state): State<Arc<AppState>>,
    Path(app): Path<String>,
) -> ApiResult<HashMap<String, Prompt>> {
    let app_type = parse_app_feature_type(&app, "prompts")?;
    let prompts = PromptService::get_prompts(&state, app_type).map_err(ApiError::from)?;
    Ok(Json(prompts))
}

pub async fn upsert_prompt(
    State(state): State<Arc<AppState>>,
    Path((app, id)): Path<(String, String)>,
    Json(mut prompt): Json<Prompt>,
) -> ApiResult<bool> {
    let app_type = parse_app_feature_type(&app, "prompts")?;
    // 如果未携带 id，回填路径中的 id，保持与 Tauri 行为一致
    if prompt.id.is_empty() {
        prompt.id = id.clone();
    } else if prompt.id != id {
        return Err(ApiError::bad_request("prompt id mismatch"));
    }

    PromptService::upsert_prompt(&state, app_type, &id, prompt).map_err(ApiError::from)?;
    Ok(Json(true))
}

pub async fn delete_prompt(
    State(state): State<Arc<AppState>>,
    Path((app, id)): Path<(String, String)>,
) -> ApiResult<bool> {
    let app_type = parse_app_feature_type(&app, "prompts")?;
    PromptService::delete_prompt(&state, app_type, &id).map_err(ApiError::from)?;
    Ok(Json(true))
}

pub async fn enable_prompt(
    State(state): State<Arc<AppState>>,
    Path((app, id)): Path<(String, String)>,
) -> ApiResult<bool> {
    let app_type = parse_app_feature_type(&app, "prompts")?;
    PromptService::enable_prompt(&state, app_type, &id).map_err(ApiError::from)?;
    Ok(Json(true))
}

pub async fn import_from_file(
    State(state): State<Arc<AppState>>,
    Path(app): Path<String>,
) -> ApiResult<String> {
    let app_type = parse_app_feature_type(&app, "prompts")?;
    let id = PromptService::import_from_file(&state, app_type).map_err(ApiError::from)?;
    Ok(Json(id))
}

pub async fn current_file_content(Path(app): Path<String>) -> ApiResult<Option<String>> {
    let app_type = parse_app_feature_type(&app, "prompts")?;
    let content = PromptService::get_current_file_content(app_type).map_err(ApiError::from)?;
    Ok(Json(content))
}
