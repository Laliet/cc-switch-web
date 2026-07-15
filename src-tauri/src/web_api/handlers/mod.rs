#![cfg(feature = "web-server")]

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use std::str::FromStr;

use crate::{app_config::AppType, error::AppError};

pub mod auth;
pub mod capabilities;
pub mod config;
pub mod deeplink;
pub mod health;
pub mod mcp;
pub mod model_fetch;
pub mod openclaw;
pub mod prompts;
pub mod providers;
pub mod proxy;
pub mod sessions;
pub mod settings;
pub mod skills;
pub mod stream_check;
pub mod subscription;
pub mod system;
pub mod usage;
pub mod webdav;
pub mod workspace;

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: String,
    message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            code: default_error_code(status).to_string(),
            message: message.into(),
        }
    }

    pub fn with_code(
        status: StatusCode,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn not_implemented(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::with_code(StatusCode::NOT_IMPLEMENTED, code, message)
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    pub fn public_message(&self) -> String {
        public_error_message(self.status, &self.message)
    }
}

impl From<AppError> for ApiError {
    fn from(err: AppError) -> Self {
        let status = match &err {
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::InvalidInput(_)
            | AppError::Config(_)
            | AppError::McpValidation(_)
            | AppError::Localized { .. } => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let message = match err {
            AppError::Unauthorized(_) => "Authentication failed".to_string(),
            AppError::Config(_) => "Invalid configuration".to_string(),
            AppError::McpValidation(_) => "Invalid MCP configuration".to_string(),
            other => other.to_string(),
        };
        Self::new(status, message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let message = self.public_message();
        if is_internal_server_failure(self.status) {
            log::error!(
                "Web API request failed with status {} and code {}",
                self.status,
                self.code
            );
        }
        let body = Json(ErrorResponse {
            code: self.code,
            error: message,
        });
        (self.status, body).into_response()
    }
}

fn public_error_message(status: StatusCode, message: &str) -> String {
    if is_internal_server_failure(status) {
        return "Internal server error".to_string();
    }
    if contains_absolute_path(message) || contains_sensitive_field(message) {
        return "Request could not be processed".to_string();
    }
    message.to_string()
}

fn is_internal_server_failure(status: StatusCode) -> bool {
    status.is_server_error() && status != StatusCode::NOT_IMPLEMENTED
}

fn contains_absolute_path(message: &str) -> bool {
    let bytes = message.as_bytes();
    for index in 0..bytes.len() {
        if bytes[index] == b'/' {
            let previous = index.checked_sub(1).map(|offset| bytes[offset]);
            let starts_path = previous.map_or(true, |value| {
                !value.is_ascii_alphanumeric() && value != b':' && value != b'/'
            });
            if starts_path && bytes.get(index + 1).is_some_and(u8::is_ascii_alphanumeric) {
                return true;
            }
        }
        if index + 2 < bytes.len()
            && bytes[index].is_ascii_alphabetic()
            && bytes[index + 1] == b':'
            && matches!(bytes[index + 2], b'/' | b'\\')
        {
            return true;
        }
        if index + 1 < bytes.len() && bytes[index] == b'\\' && bytes[index + 1] == b'\\' {
            return true;
        }
    }
    false
}

fn contains_sensitive_field(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    [
        "access_token",
        "refresh_token",
        "authorization:",
        "api_key",
        "apikey",
        "secretaccesskey",
        "secret_access_key",
        "password=",
        "password:",
        "bearer ",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

#[derive(Serialize)]
struct ErrorResponse {
    code: String,
    error: String,
}

fn default_error_code(status: StatusCode) -> &'static str {
    match status {
        StatusCode::BAD_REQUEST => "bad_request",
        StatusCode::UNAUTHORIZED => "unauthorized",
        StatusCode::FORBIDDEN => "forbidden",
        StatusCode::NOT_FOUND => "not_found",
        StatusCode::NOT_IMPLEMENTED => "not_implemented",
        StatusCode::TOO_MANY_REQUESTS => "rate_limited",
        _ if status.is_server_error() => "internal_error",
        _ => "api_error",
    }
}

pub type ApiResult<T> = Result<Json<T>, ApiError>;

/// Return a stable capability error for an app that is known but does not
/// implement a particular operation yet. Unknown app names must be rejected
/// by the parser as ordinary bad requests instead.
pub fn unsupported_app_feature(app: &AppType, feature: &str) -> ApiError {
    let feature = feature.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    let app_name = app.as_str().replace('-', "_");
    ApiError::not_implemented(
        format!("{feature}_{app_name}_unavailable"),
        format!(
            "The {feature} feature is not supported yet for {}",
            app.as_str()
        ),
    )
}

/// Check the same per-app capability matrix returned by `/api/capabilities`.
pub fn ensure_app_feature(app: &AppType, feature: &str) -> Result<(), ApiError> {
    let supported = match crate::capabilities::supports_app_feature(app, feature) {
        Some(value) => value,
        None if feature == "config_snippet" => {
            matches!(app, AppType::Claude | AppType::Codex | AppType::Gemini)
        }
        None => false,
    };
    if supported {
        Ok(())
    } else {
        Err(unsupported_app_feature(app, feature))
    }
}

pub fn parse_app_feature_type(app: &str, feature: &str) -> Result<AppType, ApiError> {
    let app_type = parse_known_app_type(app)?;
    ensure_app_feature(&app_type, feature)?;
    Ok(app_type)
}

pub fn parse_app_type(app: &str) -> Result<AppType, ApiError> {
    let app_type = parse_known_app_type(app)?;
    if !app_type.is_supported() {
        return Err(unsupported_app_feature(&app_type, "app"));
    }
    Ok(app_type)
}

pub fn parse_known_app_type(app: &str) -> Result<AppType, ApiError> {
    AppType::from_str(app).map_err(|e| ApiError::bad_request(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{
        parse_app_feature_type, parse_app_type, parse_known_app_type, public_error_message,
        ApiError,
    };
    use crate::{AppError, AppType};
    use axum::http::StatusCode;

    #[test]
    fn parse_app_type_accepts_supported_apps() {
        let app = parse_app_type("gemini").expect("supported app should parse");
        assert_eq!(app, AppType::Gemini);
    }

    #[test]
    fn parse_app_type_rejects_omo() {
        let err = parse_app_type("omo").expect_err("omo should be rejected");
        assert_eq!(err.status, StatusCode::NOT_IMPLEMENTED);
        assert_eq!(err.code, "app_omo_unavailable");
        assert!(
            err.message.contains("暂未支持") || err.message.contains("not supported yet"),
            "unexpected error message: {}",
            err.message
        );
    }

    #[test]
    fn parse_known_app_type_accepts_omo() {
        let app = parse_known_app_type("omo").expect("omo should parse");
        assert_eq!(app, AppType::Omo);
    }

    #[test]
    fn feature_parser_distinguishes_known_unsupported_from_unknown_apps() {
        let err = parse_app_feature_type("openclaw", "prompts").expect_err("unsupported feature");
        assert_eq!(err.status, StatusCode::NOT_IMPLEMENTED);
        assert_eq!(err.code, "prompts_openclaw_unavailable");

        let err = parse_app_feature_type("not-an-app", "prompts").expect_err("unknown app");
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn public_errors_hide_internal_paths_secrets_and_server_failures() {
        assert_eq!(
            public_error_message(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to read /root/private/AGENTS.md: workspace contents"
            ),
            "Internal server error"
        );
        assert_eq!(
            public_error_message(
                StatusCode::BAD_REQUEST,
                r#"upstream returned {"access_token":"secret-token"}"#
            ),
            "Request could not be processed"
        );
        assert_eq!(
            public_error_message(
                StatusCode::BAD_REQUEST,
                r"failed at C:\Users\Admin\config.json"
            ),
            "Request could not be processed"
        );
        assert_eq!(
            public_error_message(StatusCode::BAD_REQUEST, "provider id mismatch"),
            "provider id mismatch"
        );
        assert_eq!(
            public_error_message(StatusCode::NOT_IMPLEMENTED, "operation is not available"),
            "operation is not available"
        );
    }

    #[test]
    fn app_errors_do_not_expose_auth_bodies_or_configuration_details() {
        let auth = ApiError::from(AppError::Unauthorized(
            r#"HTTP 401: {"access_token":"secret-token"}"#.to_string(),
        ));
        assert_eq!(auth.status, StatusCode::UNAUTHORIZED);
        assert_eq!(auth.message, "Authentication failed");

        let config = ApiError::from(AppError::Config(
            "failed to parse /root/private/config.json: secret workspace content".to_string(),
        ));
        assert_eq!(config.status, StatusCode::BAD_REQUEST);
        assert_eq!(config.message, "Invalid configuration");
    }
}
