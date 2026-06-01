#![cfg(feature = "web-server")]

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use std::str::FromStr;

use crate::{app_config::AppType, error::AppError};

pub mod config;
pub mod health;
pub mod mcp;
pub mod model_fetch;
pub mod prompts;
pub mod providers;
pub mod proxy;
pub mod settings;
pub mod skills;
pub mod stream_check;
pub mod system;
pub mod usage;
pub mod webdav;

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }
}

impl From<AppError> for ApiError {
    fn from(err: AppError) -> Self {
        let status = match err {
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::InvalidInput(_)
            | AppError::Config(_)
            | AppError::McpValidation(_)
            | AppError::Localized { .. } => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self::new(status, err.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(ErrorResponse {
            error: self.message,
        });
        (self.status, body).into_response()
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

pub type ApiResult<T> = Result<Json<T>, ApiError>;

pub fn parse_app_type(app: &str) -> Result<AppType, ApiError> {
    AppType::parse_supported(app).map_err(|e| ApiError::bad_request(e.to_string()))
}

pub fn parse_known_app_type(app: &str) -> Result<AppType, ApiError> {
    AppType::from_str(app).map_err(|e| ApiError::bad_request(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{parse_app_type, parse_known_app_type};
    use crate::AppType;
    use axum::http::StatusCode;

    #[test]
    fn parse_app_type_accepts_supported_apps() {
        let app = parse_app_type("gemini").expect("supported app should parse");
        assert_eq!(app, AppType::Gemini);
    }

    #[test]
    fn parse_app_type_rejects_omo() {
        let err = parse_app_type("omo").expect_err("omo should be rejected");
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
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
}
