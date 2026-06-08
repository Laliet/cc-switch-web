use axum::http::Uri;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION};

use crate::{app_config::AppType, error::AppError, provider::Provider};

pub mod claude;
pub mod codex;
pub mod gemini;
pub mod opencode;

#[derive(Debug, Clone)]
pub struct AuthInfo {
    pub api_key: String,
}

pub trait ProviderAdapter: Send + Sync {
    fn extract_base_url(&self, provider: &Provider) -> Result<String, AppError>;
    fn extract_auth(&self, provider: &Provider) -> Result<Option<AuthInfo>, AppError>;
    fn build_url(&self, base_url: &str, uri: &Uri) -> Result<String, AppError>;
    fn auth_headers(&self, auth: &AuthInfo) -> Vec<(HeaderName, HeaderValue)>;
}

pub fn adapter_for(app: &AppType) -> &'static dyn ProviderAdapter {
    match app {
        AppType::Claude | AppType::ClaudeDesktop => &claude::CLAUDE_ADAPTER,
        AppType::Codex => &codex::CODEX_ADAPTER,
        AppType::Gemini => &gemini::GEMINI_ADAPTER,
        AppType::Opencode | AppType::Omo | AppType::OmoSlim => &opencode::OPENCODE_ADAPTER,
    }
}

pub fn validate_base_url(base_url: &str) -> Result<&str, AppError> {
    let base = base_url.trim().trim_end_matches('/');
    if !(base.starts_with("http://") || base.starts_with("https://")) {
        return Err(AppError::InvalidInput(
            "Provider base URL must be HTTP(S)".into(),
        ));
    }
    Ok(base)
}

pub fn append_path(base_url: &str, uri: &Uri) -> Result<String, AppError> {
    let base = validate_base_url(base_url)?;
    let path_and_query = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");
    let suffix = if path_and_query.starts_with('/') {
        path_and_query.to_string()
    } else {
        format!("/{path_and_query}")
    };
    Ok(format!("{base}{suffix}"))
}

pub fn full_endpoint_url(endpoint_url: &str, uri: &Uri) -> Result<String, AppError> {
    let endpoint = validate_base_url(endpoint_url)?;
    let Some(query) = uri.query() else {
        return Ok(endpoint.to_string());
    };
    if query.is_empty() {
        return Ok(endpoint.to_string());
    }
    let separator = if endpoint.contains('?') { '&' } else { '?' };
    Ok(format!("{endpoint}{separator}{query}"))
}

pub fn bearer_headers(api_key: &str) -> Vec<(HeaderName, HeaderValue)> {
    let Ok(value) = HeaderValue::from_str(&format!("Bearer {api_key}")) else {
        return Vec::new();
    };
    vec![(AUTHORIZATION, value)]
}

pub fn insert_auth_headers(
    headers: &mut HeaderMap,
    adapter: &dyn ProviderAdapter,
    auth: &AuthInfo,
) {
    for (name, value) in adapter.auth_headers(auth) {
        headers.insert(name, value);
    }
}

#[cfg(test)]
mod tests {
    use axum::http::Uri;

    use super::full_endpoint_url;

    #[test]
    fn full_endpoint_url_uses_endpoint_without_appending_request_path() {
        let uri: Uri = "/v1/messages".parse().expect("valid uri");

        let url = full_endpoint_url(
            "https://vertex.example.com/v1/projects/p/models/m:rawPredict",
            &uri,
        )
        .expect("full endpoint url");

        assert_eq!(
            url,
            "https://vertex.example.com/v1/projects/p/models/m:rawPredict"
        );
    }

    #[test]
    fn full_endpoint_url_preserves_request_query() {
        let uri: Uri = "/v1/messages?stream=true".parse().expect("valid uri");

        let url = full_endpoint_url("https://api.example.com/v1/responses?preview=1", &uri)
            .expect("full endpoint url");

        assert_eq!(
            url,
            "https://api.example.com/v1/responses?preview=1&stream=true"
        );
    }
}
