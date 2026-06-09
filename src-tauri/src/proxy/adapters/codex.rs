use axum::http::Uri;
use regex::Regex;
use reqwest::header::{HeaderName, HeaderValue};

use crate::{error::AppError, provider::Provider};

use super::{append_path, bearer_headers, validate_base_url, AuthInfo, ProviderAdapter};

pub struct CodexAdapter;

pub static CODEX_ADAPTER: CodexAdapter = CodexAdapter;

impl ProviderAdapter for CodexAdapter {
    fn extract_base_url(&self, provider: &Provider) -> Result<String, AppError> {
        let config_toml = provider
            .settings_config
            .get("config")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let re = Regex::new(r#"base_url\s*=\s*["']([^"']+)["']"#).map_err(|e| {
            AppError::localized(
                "provider.regex_init_failed",
                format!("正则初始化失败: {e}"),
                format!("Failed to initialize regex: {e}"),
            )
        })?;
        re.captures(config_toml)
            .and_then(|caps| caps.get(1))
            .map(|m| normalize_openai_base(m.as_str()))
            .ok_or_else(|| {
                AppError::localized(
                    "provider.codex.base_url.missing",
                    "config.toml 中缺少 base_url 配置",
                    "base_url is missing from config.toml",
                )
            })
    }

    fn extract_auth(&self, provider: &Provider) -> Result<Option<AuthInfo>, AppError> {
        let Some(auth) = provider
            .settings_config
            .get("auth")
            .and_then(|v| v.as_object())
        else {
            return Err(AppError::localized(
                "provider.codex.auth.missing",
                "配置格式错误: 缺少 auth",
                "Invalid configuration: missing auth section",
            ));
        };
        let api_key = auth
            .get("OPENAI_API_KEY")
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
        let base = validate_base_url(base_url)?;
        let path = uri.path();
        if base.ends_with("/v1") && (path == "/v1" || path.starts_with("/v1/")) {
            let trimmed = path.trim_start_matches("/v1");
            let query = uri.query().map(|q| format!("?{q}")).unwrap_or_default();
            return Ok(format!("{base}{}{query}", normalize_path(trimmed)));
        }
        append_path(base, uri)
    }

    fn auth_headers(&self, auth: &AuthInfo) -> Vec<(HeaderName, HeaderValue)> {
        bearer_headers(&auth.api_key)
    }
}

pub fn normalize_openai_base(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/v1")
    }
}

fn normalize_path(path: &str) -> String {
    if path.is_empty() {
        String::new()
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

#[cfg(test)]
mod tests {
    use axum::http::Uri;

    use super::{normalize_openai_base, CodexAdapter, ProviderAdapter};

    #[test]
    fn normalizes_openai_base_with_single_v1_suffix() {
        assert_eq!(
            normalize_openai_base("https://api.example.com"),
            "https://api.example.com/v1"
        );
        assert_eq!(
            normalize_openai_base("https://api.example.com/v1"),
            "https://api.example.com/v1"
        );
        assert_eq!(
            normalize_openai_base("https://api.example.com/v1/"),
            "https://api.example.com/v1"
        );
    }

    #[test]
    fn build_url_deduplicates_v1_path() {
        let adapter = CodexAdapter;
        let uri: Uri = "/v1/responses?stream=true".parse().expect("valid uri");
        let url = adapter
            .build_url("http://127.0.0.1:3456/v1", &uri)
            .expect("build url");
        assert_eq!(url, "http://127.0.0.1:3456/v1/responses?stream=true");
    }
}
