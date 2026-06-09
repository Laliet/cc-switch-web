use axum::http::Uri;
use reqwest::header::{HeaderName, HeaderValue};

use crate::{error::AppError, provider::Provider};

use super::{append_path, bearer_headers, AuthInfo, ProviderAdapter};

pub struct OpencodeAdapter;

pub static OPENCODE_ADAPTER: OpencodeAdapter = OpencodeAdapter;

impl ProviderAdapter for OpencodeAdapter {
    fn extract_base_url(&self, provider: &Provider) -> Result<String, AppError> {
        provider
            .settings_config
            .get("options")
            .and_then(|v| v.as_object())
            .and_then(|options| {
                options
                    .get("baseURL")
                    .or_else(|| options.get("baseUrl"))
                    .or_else(|| options.get("base_url"))
            })
            .and_then(|v| v.as_str())
            .map(|value| value.to_string())
            .ok_or_else(|| {
                AppError::localized(
                    "provider.opencode.base_url.missing",
                    "OpenCode 配置缺少 options.baseURL",
                    "OpenCode configuration is missing options.baseURL",
                )
            })
    }

    fn extract_auth(&self, provider: &Provider) -> Result<Option<AuthInfo>, AppError> {
        let Some(options) = provider
            .settings_config
            .get("options")
            .and_then(|v| v.as_object())
        else {
            return Err(AppError::localized(
                "provider.opencode.options.missing",
                "OpenCode 配置缺少 options 字段",
                "OpenCode configuration is missing options",
            ));
        };
        let api_key = options
            .get("apiKey")
            .or_else(|| options.get("api_key"))
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
        bearer_headers(&auth.api_key)
    }
}
