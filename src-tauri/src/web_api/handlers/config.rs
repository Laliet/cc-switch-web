#![cfg(feature = "web-server")]

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{parse_app_type, parse_known_app_type, ApiError, ApiResult};
use crate::{
    app_config::{AppType, MultiAppConfig},
    codex_config,
    config::{
        atomic_write, get_app_config_dir, get_app_config_path as resolve_app_config_path,
        get_claude_settings_path,
    },
    database::{BackupEntry, Database},
    error::AppError,
    gemini_config,
    services::ConfigService,
    store::AppState,
};

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreBackupPayload {
    pub filename: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameBackupPayload {
    pub old_filename: String,
    pub new_name: String,
}

pub async fn export_config(
    State(state): State<Arc<AppState>>,
    payload: Option<Json<FilePathPayload>>,
) -> ApiResult<Value> {
    let Json(payload) = payload.ok_or_else(|| ApiError::bad_request("filePath is required"))?;
    let file_path = payload
        .file_path
        .ok_or_else(|| ApiError::bad_request("filePath is required"))?;
    let target_path = ConfigService::sanitize_transfer_path(&file_path).map_err(ApiError::from)?;
    state.db.export_sql(&target_path).map_err(ApiError::from)?;

    Ok(Json(serde_json::json!(ConfigTransferResult {
        success: true,
        message: "SQL backup exported successfully".into(),
        file_path: Some(file_path),
        backup_id: None,
    })))
}

pub async fn import_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> ApiResult<ConfigTransferResult> {
    // Legacy JSON remains readable for 0.18.x migration. New exports are SQL.

    // 3) 纯配置 JSON
    let is_plain_config = body.get("providers").is_some() || body.get("mcp").is_some();
    if is_plain_config {
        let content = serde_json::to_string(&body)
            .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, e.to_string()))?;
        let config_path = resolve_app_config_path().map_err(ApiError::from)?;
        let backup_id = ConfigService::create_backup(&config_path).map_err(ApiError::from)?;
        let parsed: MultiAppConfig =
            serde_json::from_value(body).map_err(|e| ApiError::bad_request(e.to_string()))?;
        state.replace_config(&parsed).map_err(ApiError::from)?;
        atomic_write(&config_path, content.as_bytes()).map_err(ApiError::from)?;

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

    let backup_id = if let Some(content) = payload.content {
        if content
            .trim_start_matches('\u{feff}')
            .trim_start()
            .starts_with("-- CC Switch SQLite export")
        {
            state
                .db
                .import_sql_string(&content)
                .map_err(ApiError::from)?
        } else {
            let config_path = resolve_app_config_path().map_err(ApiError::from)?;
            let backup_id = ConfigService::create_backup(&config_path).map_err(ApiError::from)?;
            let parsed: MultiAppConfig =
                serde_json::from_str(&content).map_err(|e| ApiError::bad_request(e.to_string()))?;
            state.replace_config(&parsed).map_err(ApiError::from)?;
            atomic_write(&config_path, content.as_bytes()).map_err(ApiError::from)?;
            backup_id
        }
    } else if let Some(file_path) = &payload.file_path {
        let path_buf = ConfigService::sanitize_transfer_path(file_path).map_err(ApiError::from)?;
        if path_buf
            .extension()
            .is_some_and(|extension| extension == "sql")
        {
            state.db.import_sql(&path_buf).map_err(ApiError::from)?
        } else {
            let parsed =
                ConfigService::load_config_for_import(&path_buf).map_err(ApiError::from)?;
            ConfigService::apply_import_config(parsed, state.as_ref()).map_err(ApiError::from)?
        }
    } else {
        return Err(ApiError::bad_request("filePath or content is required"));
    };

    state
        .update_config(ConfigService::sync_current_providers_to_live)
        .map_err(ApiError::from)?;

    Ok(Json(ConfigTransferResult {
        success: true,
        message: "Configuration imported successfully".into(),
        file_path: file_path_ret.take(),
        backup_id: Some(backup_id),
    }))
}

/// GET export returns a complete SQL backup for browser download.
pub async fn export_config_snapshot(
    State(state): State<Arc<AppState>>,
) -> Result<Response, ApiError> {
    let sql = state.db.export_sql_string().map_err(ApiError::from)?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/sql; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=cc-switch-backup.sql",
            ),
        ],
        sql,
    )
        .into_response())
}

pub async fn create_db_backup(State(state): State<Arc<AppState>>) -> ApiResult<String> {
    let filename = state
        .db
        .backup_database_file()
        .map_err(ApiError::from)?
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .ok_or_else(|| ApiError::bad_request("Database file not found, backup skipped"))?;
    Ok(Json(filename))
}

pub async fn list_db_backups() -> ApiResult<Vec<BackupEntry>> {
    Ok(Json(Database::list_backups().map_err(ApiError::from)?))
}

pub async fn restore_db_backup(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RestoreBackupPayload>,
) -> ApiResult<String> {
    let safety_id = state
        .db
        .restore_from_backup(&payload.filename)
        .map_err(ApiError::from)?;
    state
        .update_config(ConfigService::sync_current_providers_to_live)
        .map_err(ApiError::from)?;
    Ok(Json(safety_id))
}

pub async fn rename_db_backup(Json(payload): Json<RenameBackupPayload>) -> ApiResult<String> {
    Ok(Json(
        Database::rename_backup(&payload.old_filename, &payload.new_name)
            .map_err(ApiError::from)?,
    ))
}

pub async fn delete_db_backup(Path(filename): Path<String>) -> ApiResult<bool> {
    Database::delete_backup(&filename).map_err(ApiError::from)?;
    Ok(Json(true))
}

pub async fn get_config_dir(Path(app): Path<String>) -> ApiResult<String> {
    let app_type = parse_config_app_type(&app)?;
    let dir = get_supported_config_dir(app_type)?;
    Ok(Json(dir.to_string_lossy().to_string()))
}

pub async fn get_config_dir_info(
    Path(app): Path<String>,
) -> ApiResult<crate::config::ConfigDirInfo> {
    let app_type = parse_config_app_type(&app)?;
    let info = match app_type {
        AppType::Claude | AppType::ClaudeDesktop => {
            crate::config::get_claude_config_dir_info().map_err(ApiError::from)?
        }
        AppType::Codex => codex_config::get_codex_config_dir_info().map_err(ApiError::from)?,
        AppType::Gemini => crate::gemini_config::get_gemini_dir_info().map_err(ApiError::from)?,
        AppType::Opencode => {
            crate::opencode_config::get_opencode_dir_info().map_err(ApiError::from)?
        }
        AppType::Omo | AppType::OmoSlim => {
            crate::opencode_config::get_opencode_dir_info().map_err(ApiError::from)?
        }
    };

    Ok(Json(info))
}

pub async fn open_config_folder(Path(app): Path<String>) -> ApiResult<bool> {
    let app_type = parse_config_app_type(&app)?;
    let dir = get_supported_config_dir(app_type)?;

    std::fs::create_dir_all(&dir).map_err(|e| ApiError::from(AppError::io(&dir, e)))?;
    Ok(Json(true))
}

fn get_supported_config_dir(app_type: AppType) -> Result<std::path::PathBuf, ApiError> {
    match app_type {
        AppType::Claude | AppType::ClaudeDesktop => {
            crate::config::get_claude_config_dir().map_err(ApiError::from)
        }
        AppType::Codex => codex_config::get_codex_config_dir().map_err(ApiError::from),
        AppType::Gemini => gemini_config::get_gemini_dir().map_err(ApiError::from),
        AppType::Opencode => Ok(crate::opencode_config::get_opencode_dir()),
        AppType::Omo | AppType::OmoSlim => Ok(crate::omo_config::get_omo_dir()),
    }
}

fn parse_config_app_type(app: &str) -> Result<AppType, ApiError> {
    parse_known_app_type(app)
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
    let cfg = state.load_config().map_err(ApiError::from)?;
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

    if !payload.snippet.trim().is_empty() {
        match app_type {
            AppType::Claude | AppType::Gemini => {
                serde_json::from_str::<serde_json::Value>(&payload.snippet)
                    .map_err(|e| ApiError::bad_request(format!("无效的 JSON 格式: {e}")))?;
            }
            AppType::Codex => { /* 不验证 TOML */ }
            AppType::ClaudeDesktop | AppType::Opencode | AppType::Omo | AppType::OmoSlim => {
                return Err(ApiError::bad_request(format!(
                    "应用 '{}' 暂未支持，敬请期待。",
                    app_type.as_str()
                )));
            }
        }
    }

    state
        .update_config(|guard| {
            guard.common_config_snippets.set(
                &app_type,
                if payload.snippet.trim().is_empty() {
                    None
                } else {
                    Some(payload.snippet)
                },
            );
            Ok(())
        })
        .map_err(ApiError::from)?;
    Ok(Json(true))
}

pub async fn get_claude_common_config_snippet(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Option<String>> {
    let guard = state.load_config().map_err(ApiError::from)?;
    Ok(Json(guard.common_config_snippets.claude.clone()))
}

pub async fn set_claude_common_config_snippet(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SnippetPayload>,
) -> ApiResult<bool> {
    if !payload.snippet.trim().is_empty() {
        serde_json::from_str::<serde_json::Value>(&payload.snippet)
            .map_err(|e| ApiError::bad_request(format!("无效的 JSON 格式: {e}")))?;
    }

    state
        .update_config(|guard| {
            guard.common_config_snippets.claude = if payload.snippet.trim().is_empty() {
                None
            } else {
                Some(payload.snippet)
            };
            Ok(())
        })
        .map_err(ApiError::from)?;
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
