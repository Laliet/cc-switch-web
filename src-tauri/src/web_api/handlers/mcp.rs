#![cfg(feature = "web-server")]

use std::{collections::HashMap, sync::Arc};

use axum::http::StatusCode;
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    app_config::{AppType, McpServer},
    claude_mcp,
    error::AppError,
    services::McpService,
    store::AppState,
};

use super::{ApiError, ApiResult};

pub async fn list_servers(
    State(state): State<Arc<AppState>>,
) -> ApiResult<HashMap<String, McpServer>> {
    let servers = McpService::get_all_servers(&state).map_err(internal_error)?;
    Ok(Json(servers))
}

pub async fn upsert_server(
    State(state): State<Arc<AppState>>,
    Json(server): Json<McpServer>,
) -> ApiResult<bool> {
    McpService::upsert_server(&state, server).map_err(internal_error)?;
    Ok(Json(true))
}

pub async fn update_server(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(mut server): Json<McpServer>,
) -> ApiResult<bool> {
    if server.id.is_empty() {
        server.id = id.clone();
    } else if server.id != id {
        return Err(ApiError::bad_request("server id mismatch"));
    }

    McpService::upsert_server(&state, server).map_err(internal_error)?;
    Ok(Json(true))
}

pub async fn delete_server(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<bool> {
    let deleted = McpService::delete_server(&state, &id).map_err(internal_error)?;
    Ok(Json(deleted))
}

#[derive(Deserialize)]
pub struct ToggleAppPayload {
    pub enabled: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertConfigPayload {
    pub spec: serde_json::Value,
    pub sync_other_side: Option<bool>,
}

/// 切换 MCP 服务器在指定客户端的启用状态
pub async fn toggle_app(
    State(state): State<Arc<AppState>>,
    Path((id, app)): Path<(String, String)>,
    Json(payload): Json<ToggleAppPayload>,
) -> ApiResult<bool> {
    let app_ty = super::parse_app_type(&app)?;
    McpService::toggle_app(&state, &id, app_ty, payload.enabled).map_err(internal_error)?;
    Ok(Json(true))
}

/// 获取 Claude MCP 状态
pub async fn get_status() -> ApiResult<claude_mcp::McpStatus> {
    let status = claude_mcp::get_mcp_status().map_err(internal_error)?;
    Ok(Json(status))
}

/// 读取 mcp.json 文本内容
pub async fn read_config() -> ApiResult<Option<String>> {
    let content = claude_mcp::read_mcp_json().map_err(internal_error)?;
    Ok(Json(content))
}

/// 追加或更新 Claude MCP 服务器条目
pub async fn upsert_claude_server(
    Path(id): Path<String>,
    Json(spec): Json<serde_json::Value>,
) -> ApiResult<bool> {
    claude_mcp::upsert_mcp_server(&id, spec).map_err(internal_error)?;
    Ok(Json(true))
}

pub async fn delete_claude_server(Path(id): Path<String>) -> ApiResult<bool> {
    claude_mcp::delete_mcp_server(&id).map_err(internal_error)?;
    Ok(Json(true))
}

#[derive(Deserialize)]
pub struct ValidatePayload {
    pub cmd: String,
}

pub async fn validate_command(Json(payload): Json<ValidatePayload>) -> ApiResult<bool> {
    claude_mcp::validate_command_in_path(&payload.cmd).map_err(internal_error)?;
    Ok(Json(true))
}

/// 兼容旧版：返回指定应用下的 MCP servers（来自统一配置）
pub async fn get_config(
    State(state): State<Arc<AppState>>,
    Path(app): Path<String>,
) -> ApiResult<McpConfigResponse> {
    let app_ty =
        AppType::parse_supported(&app).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let config_path = crate::config::get_app_config_path()
        .map_err(internal_error)?
        .to_string_lossy()
        .to_string();
    let servers = McpService::get_all_servers(&state)
        .map_err(internal_error)?
        .into_iter()
        .filter(|(_, server)| server.apps.is_enabled_for(&app_ty))
        .map(|(id, server)| (id, server.server))
        .collect();
    Ok(Json(McpConfigResponse {
        config_path,
        servers,
    }))
}

/// 兼容旧版：在 config.json 中新增或更新服务器（按应用）
pub async fn upsert_server_in_config(
    State(state): State<Arc<AppState>>,
    Path((app, id)): Path<(String, String)>,
    Json(payload): Json<UpsertConfigPayload>,
) -> ApiResult<bool> {
    use crate::app_config::McpApps;

    let app_ty = super::parse_app_type(&app)?;
    let spec = payload.spec;

    // 尝试读取现有服务器
    let existing = {
        let cfg = state
            .config
            .read()
            .map_err(AppError::from)
            .map_err(ApiError::from)?;
        cfg.mcp
            .servers
            .as_ref()
            .and_then(|servers| servers.get(&id).cloned())
    };

    let mut server = if let Some(mut s) = existing {
        s.server = spec.clone();
        s.apps.set_enabled_for(&app_ty, true);
        s
    } else {
        let mut apps = McpApps::default();
        apps.set_enabled_for(&app_ty, true);
        let name = spec
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&id)
            .to_string();
        McpServer {
            id: id.clone(),
            name,
            server: spec,
            apps,
            description: None,
            homepage: None,
            docs: None,
            tags: Vec::new(),
        }
    };

    // 兼容参数：如果 syncOtherSide=true，则全部启用
    if let Some(sync_other_side) = payload.sync_other_side {
        if sync_other_side {
            server.apps.claude = true;
            server.apps.codex = true;
            server.apps.gemini = true;
        }
    }

    McpService::upsert_server(&state, server).map_err(internal_error)?;
    Ok(Json(true))
}

pub async fn delete_server_in_config(
    State(state): State<Arc<AppState>>,
    Path((app, id)): Path<(String, String)>,
    payload: Option<Json<serde_json::Value>>,
) -> ApiResult<bool> {
    let app_ty = super::parse_app_type(&app)?;
    let sync_other_side = payload
        .as_ref()
        .and_then(|p| p.get("syncOtherSide"))
        .and_then(|v| v.as_bool());

    // 删除统一服务器
    let deleted = McpService::delete_server(&state, &id).map_err(internal_error)?;

    if deleted && !sync_other_side.unwrap_or(false) {
        McpService::toggle_app(&state, &id, app_ty, false).map_err(internal_error)?;
    }

    Ok(Json(deleted))
}

pub async fn set_enabled(
    State(state): State<Arc<AppState>>,
    Path((app, id)): Path<(String, String)>,
    Json(payload): Json<ToggleAppPayload>,
) -> ApiResult<bool> {
    let app_ty = super::parse_app_type(&app)?;
    McpService::toggle_app(&state, &id, app_ty, payload.enabled).map_err(internal_error)?;
    Ok(Json(true))
}

#[derive(Serialize)]
pub struct McpConfigResponse {
    pub config_path: String,
    pub servers: HashMap<String, serde_json::Value>,
}

fn internal_error(err: impl ToString) -> ApiError {
    ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
