use axum::http::Uri;
use reqwest::header::{HeaderName, HeaderValue};

use crate::{error::AppError, gemini_config::json_to_env, provider::Provider};

use super::{append_path, validate_base_url, AuthInfo, ProviderAdapter};

pub struct GeminiAdapter;

pub static GEMINI_ADAPTER: GeminiAdapter = GeminiAdapter;

impl ProviderAdapter for GeminiAdapter {
    fn extract_base_url(&self, provider: &Provider) -> Result<String, AppError> {
        let env = json_to_env(&provider.settings_config)?;
        Ok(normalize_gemini_base(
            env.get("GOOGLE_GEMINI_BASE_URL")
                .map(String::as_str)
                .unwrap_or("https://generativelanguage.googleapis.com"),
        ))
    }

    fn extract_auth(&self, provider: &Provider) -> Result<Option<AuthInfo>, AppError> {
        let env = json_to_env(&provider.settings_config)?;
        let api_key = env
            .get("GEMINI_API_KEY")
            .map(String::as_str)
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
        if base.ends_with("/v1beta") && (path == "/v1beta" || path.starts_with("/v1beta/")) {
            let trimmed = path.trim_start_matches("/v1beta");
            let query = uri.query().map(|q| format!("?{q}")).unwrap_or_default();
            let suffix = if trimmed.is_empty() {
                String::new()
            } else {
                trimmed.to_string()
            };
            return Ok(format!("{base}{suffix}{query}"));
        }
        append_path(base, uri)
    }

    fn auth_headers(&self, auth: &AuthInfo) -> Vec<(HeaderName, HeaderValue)> {
        let Ok(value) = HeaderValue::from_str(&auth.api_key) else {
            return Vec::new();
        };
        vec![(HeaderName::from_static("x-goog-api-key"), value)]
    }
}

pub fn normalize_gemini_base(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.ends_with("/v1beta") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/v1beta")
    }
}

#[cfg(test)]
mod tests {
    use axum::http::Uri;

    use super::{normalize_gemini_base, GeminiAdapter, ProviderAdapter};

    #[test]
    fn normalizes_gemini_base_with_single_v1beta_suffix() {
        assert_eq!(
            normalize_gemini_base("https://generativelanguage.googleapis.com"),
            "https://generativelanguage.googleapis.com/v1beta"
        );
        assert_eq!(
            normalize_gemini_base("https://generativelanguage.googleapis.com/v1beta/"),
            "https://generativelanguage.googleapis.com/v1beta"
        );
    }

    #[test]
    fn build_url_deduplicates_v1beta_path() {
        let adapter = GeminiAdapter;
        let uri: Uri = "/v1beta/models/gemini:generateContent?key=hidden"
            .parse()
            .expect("valid uri");
        let url = adapter
            .build_url("http://127.0.0.1:3456/v1beta", &uri)
            .expect("build url");
        assert_eq!(
            url,
            "http://127.0.0.1:3456/v1beta/models/gemini:generateContent?key=hidden"
        );
    }
}
