#![cfg(feature = "web-server")]

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    app_config::{AppType, MultiAppConfig},
    codex_config,
    config::{
        atomic_write, get_app_config_dir, get_app_config_path as resolve_app_config_path,
        get_claude_settings_path,
    },
    error::AppError,
    gemini_config,
    services::ConfigService,
    store::AppState,
};

use super::{parse_app_type, ApiError, ApiResult};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResponse {
    pub backup_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigTransferResult {
    pub success: bool,
    pub message: String,
    pub file_path: Option<String>,
    pub backup_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePathPayload {
    #[serde(default, rename = "filePath")]
    pub file_path: Option<String>,
    /// Web 模式下可直接传入配置内容
    pub content: Option<String>,
}

pub async fn export_config(
    State(state): State<Arc<AppState>>,
    payload: Option<Json<FilePathPayload>>,
) -> ApiResult<Value> {
    // 当未提供 body 时，直接返回 config 快照，兼容 bash 测试和备份逻辑。
    if payload.is_none() {
        let cfg = state
            .config
            .read()
            .map_err(AppError::from)
            .map_err(ApiError::from)?
            .clone();
        let value = serde_json::to_value(cfg)
            .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok(Json(value));
    }

    // 提供了 body：走文件导出分支
    let Json(payload) = payload.unwrap();
    let file_path = payload
        .file_path
        .ok_or_else(|| ApiError::bad_request("filePath is required"))?;
    let target_path = ConfigService::sanitize_transfer_path(&file_path).map_err(ApiError::from)?;
    ConfigService::export_config_to_path(&target_path).map_err(ApiError::from)?;

    Ok(Json(serde_json::json!(ConfigTransferResult {
        success: true,
        message: "Configuration exported successfully".into(),
        file_path: Some(file_path),
        backup_id: None,
    })))
}

pub async fn import_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> ApiResult<ConfigTransferResult> {
    // 三种输入形态：
    // 1) { filePath, content? } 与桌面端兼容
    // 2) { content } 直接传配置文本（Web 手动粘贴）
    // 3) 直接传 MultiAppConfig JSON（bash 测试）

    // 3) 纯配置 JSON
    let is_plain_config = body.get("providers").is_some() || body.get("mcp").is_some();
    if is_plain_config {
        let content = serde_json::to_string(&body)
            .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, e.to_string()))?;
        let config_path = resolve_app_config_path().map_err(ApiError::from)?;
        let backup_id = ConfigService::create_backup(&config_path).map_err(ApiError::from)?;
        let parsed: MultiAppConfig =
            serde_json::from_value(body).map_err(|e| ApiError::bad_request(e.to_string()))?;
        atomic_write(&config_path, content.as_bytes()).map_err(ApiError::from)?;

        {
            let mut guard = state
                .config
                .write()
                .map_err(AppError::from)
                .map_err(ApiError::from)?;
            *guard = parsed;
        }

        return Ok(Json(ConfigTransferResult {
            success: true,
            message: "Configuration imported successfully".into(),
            file_path: Some(config_path.to_string_lossy().to_string()),
            backup_id: Some(backup_id),
        }));
    }

    // 1/2) 兼容旧形态
    let payload: FilePathPayload = serde_json::from_value(body.clone())
        .map_err(|e| ApiError::bad_request(format!("invalid payload: {e}")))?;
    let mut file_path_ret = payload.file_path.clone();

    let mut updated_state = false;
    let (new_config, backup_id) = if let Some(content) = payload.content {
        let config_path = resolve_app_config_path().map_err(ApiError::from)?;
        let backup_id = ConfigService::create_backup(&config_path).map_err(ApiError::from)?;
        let parsed: MultiAppConfig =
            serde_json::from_str(&content).map_err(|e| ApiError::bad_request(e.to_string()))?;
        atomic_write(&config_path, content.as_bytes()).map_err(ApiError::from)?;
        (parsed, backup_id)
    } else if let Some(file_path) = &payload.file_path {
        let path_buf = ConfigService::sanitize_transfer_path(file_path).map_err(ApiError::from)?;
        let parsed = ConfigService::load_config_for_import(&path_buf).map_err(ApiError::from)?;
        let backup_id = ConfigService::apply_import_config(parsed.clone(), state.as_ref())
            .map_err(ApiError::from)?;
        updated_state = true;
        (parsed, backup_id)
    } else {
        return Err(ApiError::bad_request("filePath or content is required"));
    };

    if !updated_state {
        let mut guard = state
            .config
            .write()
            .map_err(AppError::from)
            .map_err(ApiError::from)?;
        *guard = new_config;
    }

    Ok(Json(ConfigTransferResult {
        success: true,
        message: "Configuration imported successfully".into(),
        file_path: file_path_ret.take(),
        backup_id: Some(backup_id),
    }))
}

/// GET 导出：直接返回当前配置内容，便于 Web 端下载。
pub async fn export_config_snapshot(
    State(state): State<Arc<AppState>>,
) -> ApiResult<MultiAppConfig> {
    let config = state
        .config
        .read()
        .map_err(AppError::from)
        .map_err(ApiError::from)?
        .clone();
    Ok(Json(config))
}

pub async fn get_config_dir(Path(app): Path<String>) -> ApiResult<String> {
    let app_type = parse_app_type(&app)?;
    let dir = get_supported_config_dir(app_type)?;
    Ok(Json(dir.to_string_lossy().to_string()))
}

pub async fn open_config_folder(Path(app): Path<String>) -> ApiResult<bool> {
    let app_type = parse_app_type(&app)?;
    let dir = get_supported_config_dir(app_type)?;

    std::fs::create_dir_all(&dir).map_err(|e| ApiError::from(AppError::io(&dir, e)))?;
    Ok(Json(true))
}

fn get_supported_config_dir(app_type: AppType) -> Result<std::path::PathBuf, ApiError> {
    match app_type {
        AppType::Claude => crate::config::get_claude_config_dir().map_err(ApiError::from),
        AppType::Codex => codex_config::get_codex_config_dir().map_err(ApiError::from),
        AppType::Gemini => gemini_config::get_gemini_dir().map_err(ApiError::from),
        AppType::Opencode | AppType::Omo => Err(ApiError::bad_request(format!(
            "应用 '{}' 暂未支持，敬请期待。",
            app_type.as_str()
        ))),
    }
}

pub async fn pick_directory() -> ApiResult<Option<String>> {
    Err(ApiError::new(
        StatusCode::NOT_IMPLEMENTED,
        "Directory picker is not available in web server mode",
    ))
}

pub async fn get_claude_code_config_path() -> ApiResult<String> {
    let path = get_claude_settings_path().map_err(ApiError::from)?;
    Ok(Json(path.to_string_lossy().to_string()))
}

pub async fn get_app_config_path() -> ApiResult<String> {
    let path = resolve_app_config_path().map_err(ApiError::from)?;
    Ok(Json(path.to_string_lossy().to_string()))
}

pub async fn open_app_config_folder() -> ApiResult<bool> {
    let dir = get_app_config_dir().map_err(ApiError::from)?;
    std::fs::create_dir_all(&dir).map_err(|e| ApiError::from(AppError::io(&dir, e)))?;
    Ok(Json(true))
}

pub async fn get_app_config_dir_override() -> ApiResult<Option<String>> {
    // Web server mode does not support overriding the app config directory.
    Ok(Json(None))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverridePayload {
    pub path: Option<String>,
}

pub async fn set_app_config_dir_override(Json(payload): Json<OverridePayload>) -> ApiResult<bool> {
    let _ = payload;
    // No-op in web server mode; desktop handles persistence.
    Ok(Json(true))
}

#[derive(Deserialize)]
pub struct ClaudePluginPayload {
    pub official: bool,
}

/// Web Server 模式下仅做占位，避免 404；实际写入由桌面端处理。
pub async fn apply_claude_plugin_config(
    Json(_payload): Json<ClaudePluginPayload>,
) -> ApiResult<bool> {
    Ok(Json(true))
}

pub async fn get_common_config_snippet(
    State(state): State<Arc<AppState>>,
    Path(app): Path<String>,
) -> ApiResult<Option<String>> {
    let app_type = parse_app_type(&app)?;
    let cfg = state
        .config
        .read()
        .map_err(AppError::from)
        .map_err(ApiError::from)?;
    Ok(Json(cfg.common_config_snippets.get(&app_type).cloned()))
}

#[derive(Deserialize)]
pub struct SnippetPayload {
    pub snippet: String,
}

pub async fn set_common_config_snippet(
    State(state): State<Arc<AppState>>,
    Path(app): Path<String>,
    Json(payload): Json<SnippetPayload>,
) -> ApiResult<bool> {
    let app_type =
        AppType::parse_supported(&app).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let mut guard = state
        .config
        .write()
        .map_err(AppError::from)
        .map_err(ApiError::from)?;

    if !payload.snippet.trim().is_empty() {
        match app_type {
            AppType::Claude | AppType::Gemini => {
                serde_json::from_str::<serde_json::Value>(&payload.snippet)
                    .map_err(|e| ApiError::bad_request(format!("无效的 JSON 格式: {e}")))?;
            }
            AppType::Codex => { /* 不验证 TOML */ }
            AppType::Opencode | AppType::Omo => {
                return Err(ApiError::bad_request(format!(
                    "应用 '{}' 暂未支持，敬请期待。",
                    app_type.as_str()
                )));
            }
        }
    }

    guard.common_config_snippets.set(
        &app_type,
        if payload.snippet.trim().is_empty() {
            None
        } else {
            Some(payload.snippet)
        },
    );
    guard.save().map_err(ApiError::from)?;
    Ok(Json(true))
}

pub async fn get_claude_common_config_snippet(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Option<String>> {
    let guard = state
        .config
        .read()
        .map_err(AppError::from)
        .map_err(ApiError::from)?;
    Ok(Json(guard.common_config_snippets.claude.clone()))
}

pub async fn set_claude_common_config_snippet(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SnippetPayload>,
) -> ApiResult<bool> {
    let mut guard = state
        .config
        .write()
        .map_err(AppError::from)
        .map_err(ApiError::from)?;

    if !payload.snippet.trim().is_empty() {
        serde_json::from_str::<serde_json::Value>(&payload.snippet)
            .map_err(|e| ApiError::bad_request(format!("无效的 JSON 格式: {e}")))?;
    }

    guard.common_config_snippets.claude = if payload.snippet.trim().is_empty() {
        None
    } else {
        Some(payload.snippet)
    };
    guard.save().map_err(ApiError::from)?;
    Ok(Json(true))
}

pub async fn save_file_dialog() -> ApiResult<Option<String>> {
    Err(ApiError::new(
        StatusCode::NOT_IMPLEMENTED,
        "File save dialog is not available in web server mode",
    ))
}

pub async fn open_file_dialog() -> ApiResult<Option<String>> {
    Err(ApiError::new(
        StatusCode::NOT_IMPLEMENTED,
        "File open dialog is not available in web server mode",
    ))
}
