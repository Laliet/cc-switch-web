#![cfg(feature = "web-server")]

use std::sync::Arc;

use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use super::{ApiError, ApiResult};
use crate::{
    error::AppError,
    web_api::{persist_web_credentials, SharedWebAuth},
};

const MIN_WEB_PASSWORD_LEN: usize = 8;

/// Stub handler for tray updates in web mode.
pub async fn update_tray() -> ApiResult<bool> {
    Err(super::ApiError::not_implemented(
        "tray_unavailable",
        "Tray integration is not available in web server mode",
    ))
}

pub async fn unsupported_operation(Path(operation): Path<String>) -> ApiResult<bool> {
    if operation.is_empty()
        || operation.len() > 80
        || !operation
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(ApiError::bad_request("Invalid unsupported operation name"));
    }
    Err(ApiError::not_implemented(
        format!("{operation}_unavailable"),
        format!("Operation '{operation}' is not available in web server mode"),
    ))
}

#[derive(Deserialize)]
pub struct UpdateCredentialsPayload {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct OpenExternalPayload {
    pub url: String,
}

/// Validate and acknowledge external URL open request in web mode.
/// 实际浏览器打开操作应由前端完成，这里仅作校验避免 404。
pub async fn open_external(Json(payload): Json<OpenExternalPayload>) -> ApiResult<bool> {
    let parsed = url::Url::parse(&payload.url)
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, e.to_string()))?;
    let scheme = parsed.scheme().to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Unsupported URL scheme",
        ));
    }
    Ok(Json(true))
}

/// Update web basic auth credentials (username + password).
pub(crate) async fn update_credentials(
    Extension(auth_state): Extension<SharedWebAuth>,
    Json(payload): Json<UpdateCredentialsPayload>,
) -> ApiResult<bool> {
    let username = payload.username.trim();
    if username.is_empty() {
        return Err(ApiError::bad_request("Username is required"));
    }
    if username.contains(':') {
        return Err(ApiError::bad_request("Username cannot contain ':'"));
    }
    let password = payload.password.trim();
    if password.is_empty() {
        return Err(ApiError::bad_request("Password is required"));
    }
    if password.chars().count() < MIN_WEB_PASSWORD_LEN {
        return Err(ApiError::bad_request(format!(
            "Password must be at least {MIN_WEB_PASSWORD_LEN} characters"
        )));
    }

    persist_web_credentials(username, password)?;

    let mut guard = auth_state.write().map_err(AppError::from)?;
    guard.username = username.to_string();
    guard.password = password.to_string();

    Ok(Json(true))
}

/// Return the current CSRF token for the session.
/// This endpoint requires Basic Auth but does NOT require CSRF token (it's a GET request).
pub async fn get_csrf_token(Extension(csrf): Extension<Option<Arc<String>>>) -> impl IntoResponse {
    match csrf {
        Some(token) => Json(serde_json::json!({ "csrfToken": token.as_str() })),
        None => Json(serde_json::json!({ "csrfToken": null })),
    }
}
