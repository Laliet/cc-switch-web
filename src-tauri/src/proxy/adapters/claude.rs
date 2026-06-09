use axum::http::Uri;
use reqwest::header::{HeaderName, HeaderValue, AUTHORIZATION};

use crate::{error::AppError, provider::Provider};

use super::{append_path, AuthInfo, ProviderAdapter};

pub struct ClaudeAdapter;

pub static CLAUDE_ADAPTER: ClaudeAdapter = ClaudeAdapter;

impl ProviderAdapter for ClaudeAdapter {
    fn extract_base_url(&self, provider: &Provider) -> Result<String, AppError> {
        provider
            .settings_config
            .get("env")
            .and_then(|v| v.as_object())
            .and_then(|env| env.get("ANTHROPIC_BASE_URL"))
            .and_then(|v| v.as_str())
            .map(|value| value.to_string())
            .ok_or_else(|| {
                AppError::localized(
                    "provider.claude.base_url.missing",
                    "缺少 ANTHROPIC_BASE_URL 配置",
                    "Missing ANTHROPIC_BASE_URL configuration",
                )
            })
    }

    fn extract_auth(&self, provider: &Provider) -> Result<Option<AuthInfo>, AppError> {
        let Some(env) = provider
            .settings_config
            .get("env")
            .and_then(|v| v.as_object())
        else {
            return Err(AppError::localized(
                "provider.claude.env.missing",
                "配置格式错误: 缺少 env",
                "Invalid configuration: missing env section",
            ));
        };
        let api_key = env
            .get("ANTHROPIC_AUTH_TOKEN")
            .or_else(|| env.get("ANTHROPIC_API_KEY"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        Ok((!api_key.is_empty()).then_some(AuthInfo {
            api_key,
            provider_type: None,
            api_format: None,
            account_id: None,
        }))
    }

    fn build_url(&self, base_url: &str, uri: &Uri) -> Result<String, AppError> {
        append_path(base_url, uri)
    }

    fn auth_headers(&self, auth: &AuthInfo) -> Vec<(HeaderName, HeaderValue)> {
        let mut headers = Vec::new();
        if let Ok(value) = HeaderValue::from_str(&auth.api_key) {
            headers.push((HeaderName::from_static("x-api-key"), value));
        }
        if let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", auth.api_key)) {
            headers.push((AUTHORIZATION, value));
        }
        headers
    }
}
