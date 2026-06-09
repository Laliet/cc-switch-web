use axum::http::Uri;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION};

use std::sync::Arc;

use crate::{
    app_config::AppType,
    auth::ManagedAuthProvider,
    error::AppError,
    provider::{Provider, ProviderType},
    services::{
        auth::{
            GITHUB_COPILOT_API_VERSION, GITHUB_COPILOT_EDITOR_VERSION,
            GITHUB_COPILOT_INTEGRATION_ID, GITHUB_COPILOT_PLUGIN_VERSION,
            GITHUB_COPILOT_USER_AGENT,
        },
        AuthService,
    },
    store::AppState,
};

pub mod claude;
pub mod codex;
pub mod gemini;
pub mod opencode;

#[derive(Debug, Clone)]
pub struct AuthInfo {
    pub api_key: String,
    pub provider_type: Option<String>,
    pub api_format: Option<String>,
    pub account_id: Option<String>,
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
    let auth_headers = if matches!(
        auth.provider_type.as_deref(),
        Some("github_copilot" | "codex_oauth")
    ) || matches!(
        auth.api_format.as_deref(),
        Some("openai_chat" | "openai_responses")
    ) {
        bearer_headers(&auth.api_key)
    } else {
        adapter.auth_headers(auth)
    };
    for (name, value) in auth_headers {
        headers.insert(name, value);
    }
    if auth.provider_type.as_deref() == Some(ManagedAuthProvider::GithubCopilot.as_str()) {
        insert_static_header(headers, "editor-version", GITHUB_COPILOT_EDITOR_VERSION);
        insert_static_header(
            headers,
            "editor-plugin-version",
            GITHUB_COPILOT_PLUGIN_VERSION,
        );
        insert_static_header(
            headers,
            "copilot-integration-id",
            GITHUB_COPILOT_INTEGRATION_ID,
        );
        insert_static_header(headers, "user-agent", GITHUB_COPILOT_USER_AGENT);
        insert_static_header(headers, "x-github-api-version", GITHUB_COPILOT_API_VERSION);
    }
    if auth.provider_type.as_deref() == Some(ManagedAuthProvider::CodexOauth.as_str()) {
        insert_static_header(headers, "originator", "cc-switch");
        if let Some(account_id) = auth
            .account_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            insert_header_if_valid(headers, "chatgpt-account-id", account_id);
        }
    }
}

fn insert_static_header(headers: &mut HeaderMap, name: &'static str, value: &'static str) {
    headers.insert(
        HeaderName::from_static(name),
        HeaderValue::from_static(value),
    );
}

fn insert_header_if_valid(headers: &mut HeaderMap, name: &'static str, value: &str) {
    if let Ok(value) = HeaderValue::from_str(value) {
        headers.insert(HeaderName::from_static(name), value);
    }
}

pub async fn resolve_auth_for_provider(
    state: &Arc<AppState>,
    app: &AppType,
    provider: &Provider,
    adapter: &dyn ProviderAdapter,
) -> Result<Option<AuthInfo>, AppError> {
    if let Some((managed_provider, account_id)) = managed_auth_binding(app, provider)? {
        let (account, tokens) =
            AuthService::resolve_token(state, managed_provider, account_id.as_deref()).await?;
        return Ok(Some(AuthInfo {
            api_key: tokens.access_token,
            provider_type: Some(managed_provider.as_str().to_string()),
            api_format: provider_api_format(provider),
            account_id: Some(account.id),
        }));
    }
    let mut auth = adapter.extract_auth(provider)?;
    if let Some(auth) = auth.as_mut() {
        auth.provider_type = provider_type(provider);
        auth.api_format = provider_api_format(provider);
        auth.account_id = None;
    }
    Ok(auth)
}

pub fn provider_type(provider: &Provider) -> Option<String> {
    provider
        .meta
        .as_ref()
        .and_then(|meta| meta.provider_type())
        .map(|provider_type| provider_type.as_str().to_string())
}

fn provider_api_format(provider: &Provider) -> Option<String> {
    provider
        .meta
        .as_ref()
        .and_then(|meta| meta.api_format())
        .map(|api_format| api_format.as_str())
        .map(ToString::to_string)
}

fn managed_auth_binding(
    app: &AppType,
    provider: &Provider,
) -> Result<Option<(ManagedAuthProvider, Option<String>)>, AppError> {
    let Some(meta) = provider.meta.as_ref() else {
        return Ok(None);
    };

    if let Some(binding) = meta.auth_binding.as_ref() {
        if !auth_binding_mode_is(&binding.mode, "managed") {
            return Ok(None);
        }
        let provider_type = binding
            .provider_type
            .as_deref()
            .or(meta.provider_type.as_deref())
            .ok_or_else(|| {
                AppError::InvalidInput("Managed auth binding is missing providerType".to_string())
            })?;
        let provider_type = ProviderType::parse(provider_type).ok_or_else(|| {
            AppError::InvalidInput(format!("Unsupported managed providerType: {provider_type}"))
        })?;
        let account_id = binding
            .account_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string);
        if account_id.is_none() && binding.use_default == Some(false) {
            return Err(AppError::InvalidInput(
                "Managed auth binding requires accountId when useDefault is false".to_string(),
            ));
        }
        return Ok(Some((provider_type.managed_auth_provider(), account_id)));
    }

    if matches!(
        app,
        AppType::Claude | AppType::ClaudeDesktop | AppType::Codex
    ) {
        match meta.provider_type() {
            Some(ProviderType::GithubCopilot) => {
                let account_id = meta
                    .github_account_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(ToString::to_string);
                if account_id.is_none() && provider_has_manual_auth_key(provider) {
                    return Ok(None);
                }
                return Ok(Some((ManagedAuthProvider::GithubCopilot, account_id)));
            }
            Some(ProviderType::CodexOauth) => {
                if provider_has_manual_auth_key(provider) {
                    return Ok(None);
                }
                return Ok(Some((ManagedAuthProvider::CodexOauth, None)));
            }
            None => {}
        }
    }

    Ok(None)
}

fn auth_binding_mode_is(actual: &str, expected: &str) -> bool {
    normalize_auth_binding_mode(actual) == normalize_auth_binding_mode(expected)
}

fn normalize_auth_binding_mode(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| {
            if ch == '-' || ch.is_ascii_whitespace() {
                '_'
            } else {
                ch.to_ascii_lowercase()
            }
        })
        .collect()
}

fn provider_has_manual_auth_key(provider: &Provider) -> bool {
    let env = provider.settings_config.get("env");
    env.and_then(|value| {
        [
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_API_KEY",
            "OPENROUTER_API_KEY",
            "OPENAI_API_KEY",
            "GEMINI_API_KEY",
        ]
        .into_iter()
        .find_map(|key| value.get(key))
    })
    .or_else(|| {
        provider
            .settings_config
            .get("auth")
            .and_then(|auth| auth.get("OPENAI_API_KEY"))
    })
    .or_else(|| provider.settings_config.get("apiKey"))
    .or_else(|| provider.settings_config.get("api_key"))
    .and_then(serde_json::Value::as_str)
    .map(str::trim)
    .is_some_and(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use axum::http::Uri;
    use reqwest::header::HeaderMap;

    use super::{codex::CODEX_ADAPTER, full_endpoint_url, insert_auth_headers, AuthInfo};

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

    #[test]
    fn codex_oauth_auth_headers_include_originator_and_account_id() {
        let mut headers = HeaderMap::new();
        insert_auth_headers(
            &mut headers,
            &CODEX_ADAPTER,
            &AuthInfo {
                api_key: "access-token".to_string(),
                provider_type: Some("codex_oauth".to_string()),
                api_format: Some("openai_responses".to_string()),
                account_id: Some("chatgpt-account".to_string()),
            },
        );

        assert_eq!(
            headers
                .get("authorization")
                .and_then(|value| value.to_str().ok()),
            Some("Bearer access-token")
        );
        assert_eq!(
            headers
                .get("originator")
                .and_then(|value| value.to_str().ok()),
            Some("cc-switch")
        );
        assert_eq!(
            headers
                .get("chatgpt-account-id")
                .and_then(|value| value.to_str().ok()),
            Some("chatgpt-account")
        );
    }
}
