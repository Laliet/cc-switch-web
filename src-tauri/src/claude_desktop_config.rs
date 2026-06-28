use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(any(target_os = "macos", windows))]
use crate::config::get_home_dir;
use crate::config::{atomic_write, delete_file, read_json_file, write_json_file};
use crate::database::{Database, CLAUDE_DESKTOP_OFFICIAL_PROVIDER_ID};
use crate::error::AppError;
use crate::json_canonical::canonical_json_string;
use crate::provider::{ClaudeDesktopMode, Provider, ProviderApiFormat, ProviderMeta, ProviderType};
use crate::proxy::gemini_schema::{
    build_gemini_function_declaration, rectify_gemini_tool_call_parts, AnthropicToolSchemaHints,
};
use crate::proxy::gemini_shadow::{GeminiAssistantTurn, GeminiShadowStore, GeminiToolCallMeta};
use crate::store::AppState;
use crate::ManagedAuthProvider;

pub const PROFILE_ID: &str = "00000000-0000-4000-8000-000000157210";
pub const PROFILE_NAME: &str = "CC Switch";
pub const CLAUDE_ROUTE_PREFIX: &str = "claude-";
pub const ANTHROPIC_CLAUDE_ROUTE_PREFIX: &str = "anthropic/claude-";
pub const ONE_M_CONTEXT_MARKER: &str = "[1m]";

#[cfg(any(target_os = "macos", windows))]
const CONFIG_FILE: &str = "claude_desktop_config.json";
#[cfg(any(target_os = "macos", windows))]
const CONFIG_LIBRARY_DIR: &str = "configLibrary";
const GATEWAY_TOKEN_SETTING_KEY: &str = "claude_desktop_gateway_token";
const CLAUDE_DESKTOP_PROXY_PREFIX: &str = "/claude-desktop";
const DEFAULT_CREATED_AT: &str = "2024-01-01T00:00:00Z";
const GEMINI_SYNTHESIZED_ID_PREFIX: &str = "gemini_synth_";
const ANTHROPIC_BILLING_HEADER_PREFIX: &str = "x-anthropic-billing-header:";

const NON_ANTHROPIC_ROUTE_MARKERS: &[&str] = &[
    "ark-code",
    "astron",
    "command-r",
    "deepseek",
    "doubao",
    "gemini",
    "gemma",
    "glm",
    "gpt",
    "grok",
    "hermes",
    "kimi",
    "lfm",
    "llama",
    "longcat",
    "mimo",
    "minimax",
    "mistral",
    "mixtral",
    "moonshot",
    "nemotron",
    "openai",
    "qianfan",
    "qwen",
    "stepfun",
    "seed-",
    "hunyuan",
    "nova-",
    "ernie",
    "codex",
    "abab",
    "jamba",
    "arctic",
    "solar",
    "mercury",
];

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDesktopDefaultRoute {
    pub route_id: &'static str,
    pub env_key: &'static str,
    #[serde(rename = "supports1m")]
    pub supports_1m: bool,
}

pub const DEFAULT_PROXY_ROUTES: &[ClaudeDesktopDefaultRoute] = &[
    ClaudeDesktopDefaultRoute {
        route_id: "claude-sonnet-4-6",
        env_key: "ANTHROPIC_DEFAULT_SONNET_MODEL",
        supports_1m: true,
    },
    ClaudeDesktopDefaultRoute {
        route_id: "claude-opus-4-7",
        env_key: "ANTHROPIC_DEFAULT_OPUS_MODEL",
        supports_1m: true,
    },
    ClaudeDesktopDefaultRoute {
        route_id: "claude-haiku-4-5",
        env_key: "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        supports_1m: true,
    },
];

#[derive(Debug, Clone)]
struct ClaudeDesktopPaths {
    normal_config_path: PathBuf,
    threep_config_path: PathBuf,
    config_library_path: PathBuf,
    profile_path: PathBuf,
    meta_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectGatewayCredentials {
    pub base_url: String,
    pub api_key: String,
}

#[derive(Debug, Clone)]
struct FileSnapshot {
    path: PathBuf,
    content: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDesktopStatus {
    pub supported: bool,
    pub configured: bool,
    pub applied_id: Option<String>,
    pub profile_path: Option<String>,
    pub config_library_path: Option<String>,
    pub mode: Option<ClaudeDesktopMode>,
    pub expected_base_url: Option<String>,
    pub actual_base_url: Option<String>,
    pub proxy_running: bool,
    pub stale_raw_models: bool,
    pub missing_route_mappings: bool,
    pub gateway_token_configured: bool,
    pub needs_restart: bool,
    pub restart_hint: Option<String>,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedModelRoute {
    pub route_id: String,
    pub upstream_model: String,
    pub label_override: Option<String>,
    pub supports_1m: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InferenceModelSpec {
    name: String,
    label_override: Option<String>,
    supports_1m: bool,
}

pub fn apply_provider(db: &Database, provider: &Provider) -> Result<(), AppError> {
    let paths = current_platform_paths()?;
    apply_provider_to_paths(db, provider, &paths)
}

pub fn get_status(db: &Database, proxy_running: bool) -> Result<ClaudeDesktopStatus, AppError> {
    if !is_supported_platform() {
        return Ok(ClaudeDesktopStatus {
            supported: false,
            configured: false,
            applied_id: None,
            profile_path: None,
            config_library_path: None,
            mode: None,
            expected_base_url: None,
            actual_base_url: None,
            proxy_running,
            stale_raw_models: false,
            missing_route_mappings: false,
            gateway_token_configured: false,
            needs_restart: false,
            restart_hint: None,
            issues: vec![
                "Claude Desktop 3P profile management is only supported on macOS and Windows."
                    .to_string(),
            ],
        });
    }

    let paths = current_platform_paths()?;
    let applied_id = read_applied_id(&paths.meta_path);
    let configured = paths.profile_path.exists() || meta_has_profile_entry(&paths.meta_path);
    let profile = read_json_or_empty(&paths.profile_path).unwrap_or_else(|_| json!({}));
    let actual_base_url = profile
        .get("inferenceGatewayBaseUrl")
        .and_then(Value::as_str)
        .map(str::to_string);
    let stale_raw_models = profile
        .get("inferenceModels")
        .and_then(Value::as_array)
        .map(|models| {
            models.iter().any(|item| {
                item.as_str()
                    .or_else(|| item.get("name").and_then(Value::as_str))
                    .is_some_and(|model| !is_claude_safe_model_id(model))
            })
        })
        .unwrap_or(false);
    let gateway_token_configured = db
        .get_setting(GATEWAY_TOKEN_SETTING_KEY)
        .ok()
        .flatten()
        .is_some_and(|token| !token.trim().is_empty());
    let current_provider = db.load_config().ok().and_then(|config| {
        let manager = config.get_manager(&crate::app_config::AppType::ClaudeDesktop)?;
        manager.providers.get(&manager.current).cloned()
    });
    let mode = current_provider.as_ref().map(provider_mode);
    let expected_base_url = match mode {
        Some(ClaudeDesktopMode::Proxy) => proxy_gateway_base_url_from_db(db).ok(),
        Some(ClaudeDesktopMode::Direct) => current_provider
            .as_ref()
            .and_then(|provider| direct_gateway_credentials(provider).ok())
            .map(|credentials| credentials.base_url),
        None => None,
    };
    let missing_route_mappings = current_provider.as_ref().is_some_and(|provider| {
        matches!(provider_mode(provider), ClaudeDesktopMode::Proxy)
            && proxy_model_routes(provider).is_err()
    });
    let needs_restart = configured;
    let mut issues = Vec::new();
    if !configured {
        issues.push("CC Switch profile has not been applied to Claude Desktop yet.".to_string());
    }
    if expected_base_url.is_some()
        && actual_base_url.is_some()
        && expected_base_url != actual_base_url
    {
        issues.push(
            "Claude Desktop profile base URL does not match the selected provider.".to_string(),
        );
    }
    if matches!(mode, Some(ClaudeDesktopMode::Proxy)) && !proxy_running {
        issues.push(
            "Local proxy is not running, so proxy-mode Desktop routes will fail.".to_string(),
        );
    }
    if stale_raw_models {
        issues.push(
            "Profile contains raw upstream model IDs; reapply the provider profile.".to_string(),
        );
    }
    if missing_route_mappings {
        issues.push("Current provider is missing Claude Desktop model route mappings.".to_string());
    }
    if matches!(mode, Some(ClaudeDesktopMode::Proxy)) && !gateway_token_configured {
        issues.push(
            "Gateway token is not configured for the local Claude Desktop route.".to_string(),
        );
    }
    if let Some(provider) = current_provider.as_ref() {
        issues.extend(provider_status_issues(db, provider, proxy_running));
    }
    let restart_hint = needs_restart.then(|| {
        "Restart Claude Desktop after applying or switching a 3P provider so it reloads the CC Switch profile.".to_string()
    });

    Ok(ClaudeDesktopStatus {
        supported: true,
        configured,
        applied_id,
        profile_path: Some(paths.profile_path.display().to_string()),
        config_library_path: Some(paths.config_library_path.display().to_string()),
        mode,
        expected_base_url,
        actual_base_url,
        proxy_running,
        stale_raw_models,
        missing_route_mappings,
        gateway_token_configured,
        needs_restart,
        restart_hint,
        issues,
    })
}

fn provider_status_issues(db: &Database, provider: &Provider, proxy_running: bool) -> Vec<String> {
    if is_official_provider(provider) {
        return Vec::new();
    }

    let mut issues = Vec::new();
    if let Err(err) = validate_provider(provider) {
        issues.push(format!(
            "Current Claude Desktop provider is not compatible: {err}"
        ));
    }

    if matches!(provider_mode(provider), ClaudeDesktopMode::Proxy)
        && provider
            .meta
            .as_ref()
            .and_then(|meta| meta.provider_type())
            .is_some_and(|provider_type| {
                matches!(
                    provider_type,
                    ProviderType::GithubCopilot | ProviderType::CodexOauth
                )
            })
        && !proxy_running
    {
        issues.push(
            "OAuth-backed Claude Desktop providers require the local proxy to be running."
                .to_string(),
        );
    }

    if let Some(issue) = managed_auth_binding_issue(provider) {
        issues.push(issue);
    } else if let Some((managed_provider, account_id)) = managed_auth_requirement(provider) {
        let found = if let Some(account_id) = account_id.as_deref() {
            db.get_managed_auth_account(managed_provider, account_id)
        } else {
            db.get_default_managed_auth_account(managed_provider)
        }
        .ok()
        .flatten()
        .is_some_and(managed_auth_secret_is_usable);
        if !found {
            let account_hint = account_id
                .as_deref()
                .map(|id| format!(" account '{id}'"))
                .unwrap_or_else(|| " default account".to_string());
            issues.push(format!(
                "Missing {} managed auth{}; sign in from Auth Center or choose another account.",
                managed_provider.as_str(),
                account_hint,
            ));
        }
    }

    issues
}

fn managed_auth_secret_is_usable(secret: crate::auth::ManagedAuthAccountSecret) -> bool {
    let logged_out = secret
        .account
        .status
        .as_deref()
        .map(str::trim)
        .is_some_and(|status| status.eq_ignore_ascii_case("logged_out"));
    !logged_out && !secret.tokens.access_token.trim().is_empty()
}

fn managed_auth_binding_issue(provider: &Provider) -> Option<String> {
    let meta = provider.meta.as_ref()?;
    let provider_type = meta.provider_type()?;
    let binding = meta.auth_binding.as_ref()?;
    if !auth_binding_mode_is(&binding.mode, "managed") {
        return None;
    }
    let account_id = binding
        .account_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if account_id.is_none() && binding.use_default == Some(false) {
        return Some(format!(
            "Managed {} auth binding requires an accountId when useDefault is false.",
            provider_type.as_str()
        ));
    }
    None
}

fn managed_auth_requirement(provider: &Provider) -> Option<(ManagedAuthProvider, Option<String>)> {
    let meta = provider.meta.as_ref()?;
    let provider_type = meta.provider_type()?;
    if !matches!(
        provider_type,
        ProviderType::GithubCopilot | ProviderType::CodexOauth
    ) {
        return None;
    }
    let binding = meta.auth_binding.as_ref();
    if binding.is_some_and(|binding| auth_binding_mode_is(&binding.mode, "api_key")) {
        return None;
    }
    if binding.is_none()
        && meta
            .github_account_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
        && has_manual_auth_key(provider)
    {
        return None;
    }
    Some((
        provider_type.managed_auth_provider(),
        binding
            .and_then(|binding| binding.account_id.as_deref())
            .or(meta.github_account_id.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
    ))
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

fn has_manual_auth_key(provider: &Provider) -> bool {
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
    .and_then(Value::as_str)
    .map(str::trim)
    .is_some_and(|value| !value.is_empty())
}

pub fn default_proxy_routes() -> Vec<ClaudeDesktopDefaultRoute> {
    DEFAULT_PROXY_ROUTES.to_vec()
}

pub fn import_providers_from_claude(state: &AppState) -> Result<usize, AppError> {
    let mut imported = 0usize;
    state.update_config(|config| {
        let claude_providers = config
            .get_manager(&crate::app_config::AppType::Claude)
            .map(|manager| manager.providers.clone())
            .unwrap_or_default();
        let desktop_manager = config
            .get_manager_mut(&crate::app_config::AppType::ClaudeDesktop)
            .ok_or_else(|| {
                AppError::localized(
                    "provider.app.not_found",
                    "应用配置不存在: claude-desktop",
                    "App configuration not found: claude-desktop",
                )
            })?;

        ensure_official_provider(desktop_manager);
        for provider in claude_providers.values() {
            if desktop_manager.providers.contains_key(&provider.id) {
                continue;
            }

            let mut desktop_provider = provider.clone();
            let meta = desktop_provider
                .meta
                .get_or_insert_with(ProviderMeta::default);
            if is_compatible_direct_provider(provider)
                && claude_provider_models_are_claude_safe(provider)
            {
                meta.claude_desktop_mode = Some(ClaudeDesktopMode::Direct);
            } else if let Some(routes) = suggested_routes_from_claude_provider(provider) {
                meta.claude_desktop_mode = Some(ClaudeDesktopMode::Proxy);
                meta.claude_desktop_model_routes = routes;
            } else {
                continue;
            }

            desktop_manager
                .providers
                .insert(desktop_provider.id.clone(), desktop_provider);
            imported += 1;
        }

        if desktop_manager.current.is_empty() {
            desktop_manager.current = CLAUDE_DESKTOP_OFFICIAL_PROVIDER_ID.to_string();
        }
        Ok(())
    })?;
    Ok(imported)
}

fn ensure_official_provider(manager: &mut crate::provider::ProviderManager) {
    manager
        .providers
        .entry(CLAUDE_DESKTOP_OFFICIAL_PROVIDER_ID.to_string())
        .or_insert_with(|| {
            let mut provider = Provider::with_id(
                CLAUDE_DESKTOP_OFFICIAL_PROVIDER_ID.to_string(),
                "Claude Desktop Official".to_string(),
                json!({"env": {}}),
                Some("https://claude.ai/download".to_string()),
            );
            provider.category = Some("official".to_string());
            provider
        });
}

fn claude_provider_models_are_claude_safe(provider: &Provider) -> bool {
    let Some(env) = provider
        .settings_config
        .get("env")
        .and_then(Value::as_object)
    else {
        return true;
    };

    [
        "ANTHROPIC_MODEL",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        "ANTHROPIC_DEFAULT_SONNET_MODEL",
        "ANTHROPIC_DEFAULT_OPUS_MODEL",
    ]
    .into_iter()
    .filter_map(|key| env.get(key).and_then(Value::as_str))
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .all(is_claude_safe_model_id)
}

pub fn is_compatible_direct_provider(provider: &Provider) -> bool {
    validate_direct_provider(provider).is_ok()
}

pub fn is_official_provider(provider: &Provider) -> bool {
    provider.id == CLAUDE_DESKTOP_OFFICIAL_PROVIDER_ID
}

pub fn provider_mode(provider: &Provider) -> ClaudeDesktopMode {
    provider
        .meta
        .as_ref()
        .and_then(|meta| meta.claude_desktop_mode.clone())
        .unwrap_or(ClaudeDesktopMode::Direct)
}

pub fn is_claude_safe_model_id(model: &str) -> bool {
    let normalized = model.trim().to_ascii_lowercase();
    let has_allowed_shape = (normalized.starts_with(CLAUDE_ROUTE_PREFIX)
        && normalized.len() > CLAUDE_ROUTE_PREFIX.len())
        || (normalized.starts_with(ANTHROPIC_CLAUDE_ROUTE_PREFIX)
            && normalized.len() > ANTHROPIC_CLAUDE_ROUTE_PREFIX.len())
        || matches!(normalized.as_str(), "sonnet" | "opus" | "haiku")
        || (normalized.starts_with("sonnet-") && normalized.len() > "sonnet-".len())
        || (normalized.starts_with("opus-") && normalized.len() > "opus-".len())
        || (normalized.starts_with("haiku-") && normalized.len() > "haiku-".len());

    has_allowed_shape
        && !normalized.contains(ONE_M_CONTEXT_MARKER)
        && !NON_ANTHROPIC_ROUTE_MARKERS
            .iter()
            .any(|marker| normalized.contains(marker))
}

pub fn direct_gateway_credentials(
    provider: &Provider,
) -> Result<DirectGatewayCredentials, AppError> {
    let env = provider
        .settings_config
        .get("env")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            AppError::localized(
                "claude_desktop.provider.env_missing",
                "Claude Desktop 直连供应商缺少 env 配置",
                "Claude Desktop direct provider is missing env configuration",
            )
        })?;

    let base_url = required_env(env, "ANTHROPIC_BASE_URL", "ANTHROPIC_BASE_URL")?;
    let preferred_key = match provider
        .meta
        .as_ref()
        .and_then(|meta| meta.api_key_field.as_deref())
        .map(str::trim)
    {
        Some("ANTHROPIC_API_KEY") => "ANTHROPIC_API_KEY",
        _ => "ANTHROPIC_AUTH_TOKEN",
    };
    let fallback_key = if preferred_key == "ANTHROPIC_API_KEY" {
        "ANTHROPIC_AUTH_TOKEN"
    } else {
        "ANTHROPIC_API_KEY"
    };
    let api_key = required_env_any(
        env,
        &[preferred_key, fallback_key],
        "ANTHROPIC_AUTH_TOKEN 或 ANTHROPIC_API_KEY",
    )?;
    Ok(DirectGatewayCredentials { base_url, api_key })
}

fn required_env(
    env: &serde_json::Map<String, Value>,
    key: &str,
    label: &str,
) -> Result<String, AppError> {
    required_env_any(env, &[key], label)
}

fn required_env_any(
    env: &serde_json::Map<String, Value>,
    keys: &[&str],
    label: &str,
) -> Result<String, AppError> {
    for key in keys {
        if let Some(value) = env
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Ok(value.to_string());
        }
    }

    Err(AppError::localized(
        "claude_desktop.provider.env_key_missing",
        format!("Claude Desktop 供应商缺少 {label}"),
        format!("Claude Desktop provider is missing {label}"),
    ))
}

pub fn validate_provider(provider: &Provider) -> Result<(), AppError> {
    if is_official_provider(provider) {
        return Ok(());
    }

    match provider_mode(provider) {
        ClaudeDesktopMode::Direct => validate_direct_provider(provider),
        ClaudeDesktopMode::Proxy => validate_proxy_provider(provider),
    }
}

pub fn validate_direct_provider(provider: &Provider) -> Result<(), AppError> {
    if is_official_provider(provider) {
        return Ok(());
    }
    if !provider.settings_config.is_object() {
        return Err(AppError::localized(
            "claude_desktop.provider.settings_not_object",
            "Claude Desktop 直连供应商配置必须是 JSON 对象",
            "Claude Desktop direct provider configuration must be a JSON object",
        ));
    }

    if let Some(meta) = provider.meta.as_ref() {
        if meta.api_format_raw().is_some()
            && meta.api_format() != Some(ProviderApiFormat::Anthropic)
        {
            return Err(AppError::localized(
                "claude_desktop.provider.api_format_unsupported",
                "Claude Desktop 直连模式只支持原生 Anthropic Messages API",
                "Claude Desktop direct mode only supports native Anthropic Messages API",
            ));
        }
        if matches!(meta.claude_desktop_mode, Some(ClaudeDesktopMode::Proxy)) {
            return Err(AppError::localized(
                "claude_desktop.provider.mode_unsupported",
                "该供应商是 Claude Desktop 本地路由模式，不能按直连模式写入",
                "This provider uses Claude Desktop proxy mode and cannot be written as direct mode",
            ));
        }
        if matches!(
            meta.provider_type(),
            Some(ProviderType::GithubCopilot | ProviderType::CodexOauth)
        ) {
            return Err(AppError::localized(
                "claude_desktop.provider.type_unsupported",
                "Claude Desktop 直连模式不支持需要本地代理转换的供应商",
                "Claude Desktop direct mode does not support providers that need local proxy conversion",
            ));
        }
        if meta.is_full_url == Some(true) {
            return Err(AppError::localized(
                "claude_desktop.provider.full_url_unsupported",
                "Claude Desktop 直连模式不支持完整 URL 端点配置",
                "Claude Desktop direct mode does not support full URL endpoint configuration",
            ));
        }
    }

    direct_inference_model_specs(provider)?;
    direct_gateway_credentials(provider)?;
    Ok(())
}

pub fn validate_proxy_provider(provider: &Provider) -> Result<(), AppError> {
    if is_official_provider(provider) {
        return Ok(());
    }
    if !provider.settings_config.is_object() {
        return Err(AppError::localized(
            "claude_desktop.provider.settings_not_object",
            "Claude Desktop 本地路由供应商配置必须是 JSON 对象",
            "Claude Desktop proxy provider configuration must be a JSON object",
        ));
    }
    if let Some(meta) = provider.meta.as_ref() {
        if let Some(api_format) = meta.api_format_raw() {
            if meta.api_format().is_none() {
                return Err(AppError::localized(
                    "claude_desktop.provider.api_format_unsupported",
                    format!("Claude Desktop 本地路由模式不支持 API 格式: {api_format}"),
                    format!("Claude Desktop proxy mode does not support API format: {api_format}"),
                ));
            }
        }
    }
    proxy_model_routes(provider)?;
    if !has_proxy_base_url_and_key(provider) {
        return Err(AppError::localized(
            "claude_desktop.provider.credentials_missing",
            "Claude Desktop 本地路由供应商缺少 Base URL 或 API Key",
            "Claude Desktop proxy provider is missing Base URL or API key",
        ));
    }
    Ok(())
}

fn has_proxy_base_url_and_key(provider: &Provider) -> bool {
    let env = provider.settings_config.get("env");
    let has_base_url = env
        .and_then(|value| value.get("ANTHROPIC_BASE_URL"))
        .or_else(|| provider.settings_config.get("base_url"))
        .or_else(|| provider.settings_config.get("baseURL"))
        .or_else(|| provider.settings_config.get("apiEndpoint"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());

    if managed_auth_binding_issue(provider).is_none()
        && managed_auth_requirement(provider).is_some()
    {
        return has_base_url;
    }

    let has_key = env
        .and_then(|value| {
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
        .or_else(|| provider.settings_config.get("apiKey"))
        .or_else(|| provider.settings_config.get("api_key"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());

    has_base_url && has_key
}

fn direct_inference_model_specs(provider: &Provider) -> Result<Vec<InferenceModelSpec>, AppError> {
    let Some(routes) = provider
        .meta
        .as_ref()
        .map(|meta| &meta.claude_desktop_model_routes)
    else {
        return Ok(Vec::new());
    };

    let mut result = Vec::new();
    for (route_id, route) in routes {
        let route_id = route_id.trim();
        if route_id.is_empty() {
            continue;
        }
        if !is_claude_safe_model_id(route_id) {
            return Err(AppError::localized(
                "claude_desktop.provider.route_invalid",
                format!("Claude Desktop 直连模型必须使用 claude-* 或 anthropic/claude-* 名称: {route_id}"),
                format!("Claude Desktop direct model must use a claude-* or anthropic/claude-* name: {route_id}"),
            ));
        }
        result.push(InferenceModelSpec {
            name: route_id.to_string(),
            label_override: route
                .label_override
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            supports_1m: route.supports_1m.unwrap_or(false),
        });
    }

    result.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| b.supports_1m.cmp(&a.supports_1m))
    });
    result.dedup_by(|a, b| a.name == b.name);
    Ok(result)
}

pub fn proxy_model_routes(provider: &Provider) -> Result<Vec<ResolvedModelRoute>, AppError> {
    let routes = provider
        .meta
        .as_ref()
        .map(|meta| &meta.claude_desktop_model_routes)
        .ok_or_else(|| {
            AppError::localized(
                "claude_desktop.provider.routes_missing",
                "Claude Desktop 本地路由模式缺少模型路由映射",
                "Claude Desktop proxy mode is missing model route mappings",
            )
        })?;

    let reserved_route_ids = routes
        .keys()
        .map(|route_id| route_id.trim())
        .filter(|route_id| is_claude_safe_model_id(route_id))
        .map(str::to_string)
        .collect::<HashSet<_>>();
    let mut result = Vec::new();
    let mut entries = routes.iter().collect::<Vec<_>>();
    entries.sort_by_key(|(route_id, _)| *route_id);
    for (route_id, route) in entries {
        let route_id = route_id.trim();
        let upstream_model = route.model.trim();
        if route_id.is_empty() || upstream_model.is_empty() {
            continue;
        }
        let repaired_route_id = if is_claude_safe_model_id(route_id) {
            route_id.to_string()
        } else {
            next_catalog_safe_route_id(&result, &reserved_route_ids)
        };
        result.push(ResolvedModelRoute {
            route_id: repaired_route_id,
            upstream_model: upstream_model.to_string(),
            label_override: route
                .label_override
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| {
                    (!is_claude_safe_model_id(route_id)).then(|| upstream_model.to_string())
                }),
            supports_1m: route.supports_1m.unwrap_or(false),
        });
    }

    result.sort_by(|a, b| a.route_id.cmp(&b.route_id));
    result.dedup_by(|a, b| a.route_id == b.route_id);
    if result.is_empty() {
        return Err(AppError::localized(
            "claude_desktop.provider.routes_missing",
            "Claude Desktop 本地路由模式至少需要一个模型路由映射",
            "Claude Desktop proxy mode requires at least one model route mapping",
        ));
    }
    Ok(result)
}

fn next_catalog_safe_route_id(
    existing: &[ResolvedModelRoute],
    reserved: &HashSet<String>,
) -> String {
    if let Some(default_route) = DEFAULT_PROXY_ROUTES
        .iter()
        .map(|route| route.route_id)
        .find(|route_id| {
            !reserved.contains(*route_id)
                && !existing.iter().any(|route| route.route_id == *route_id)
        })
    {
        return default_route.to_string();
    }

    let mut index = 2usize;
    loop {
        let route_id = format!("{}-r{index}", DEFAULT_PROXY_ROUTES[0].route_id);
        if !reserved.contains(&route_id) && !existing.iter().any(|route| route.route_id == route_id)
        {
            return route_id;
        }
        index += 1;
    }
}

pub fn model_list_response(provider: &Provider) -> Result<Value, AppError> {
    let routes = proxy_model_routes(provider)?;
    let data: Vec<Value> = routes
        .iter()
        .map(|route| {
            let mut item = json!({
                "type": "model",
                "id": route.route_id,
                "created_at": DEFAULT_CREATED_AT,
            });
            if route.supports_1m {
                item["supports1m"] = json!(true);
            }
            item
        })
        .collect();
    let first_id = data
        .first()
        .and_then(|item| item.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let last_id = data
        .last()
        .and_then(|item| item.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string);

    Ok(json!({
        "data": data,
        "has_more": false,
        "first_id": first_id,
        "last_id": last_id,
    }))
}

pub fn map_proxy_request_model(mut body: Value, provider: &Provider) -> Result<Value, AppError> {
    let route = resolve_proxy_request_route(&body, provider)?;
    body["model"] = json!(route.upstream_model);
    if provider
        .meta
        .as_ref()
        .is_some_and(|meta| meta.provider_type() == Some(ProviderType::GithubCopilot))
    {
        body = crate::proxy::copilot_optimizer::prepare_body_for_copilot(body);
    }
    match proxy_api_format(provider) {
        Some("openai_chat") => {
            map_openai_compatible_tool_choice(&mut body)?;
            body = map_anthropic_messages_to_openai_chat(body)?;
        }
        Some("openai_responses") => {
            map_openai_responses_tool_choice(&mut body)?;
            body = map_anthropic_messages_to_openai_responses(body)?;
            apply_openai_responses_provider_meta(&mut body, provider);
        }
        Some("gemini_native") => {
            body = map_anthropic_messages_to_gemini_native(body)?;
        }
        _ => {}
    }
    Ok(body)
}

pub fn map_proxy_request_model_with_gemini_shadow(
    mut body: Value,
    provider: &Provider,
    shadow_store: &GeminiShadowStore,
    session_id: Option<&str>,
) -> Result<Value, AppError> {
    let route = resolve_proxy_request_route(&body, provider)?;
    body["model"] = json!(route.upstream_model);
    match proxy_api_format(provider) {
        Some("gemini_native") => {
            let shadow_turns = session_id
                .map(|session_id| shadow_store.get_session_turns(&provider.id, session_id))
                .unwrap_or_default();
            body = map_anthropic_messages_to_gemini_native_with_shadow(body, &shadow_turns)?;
        }
        _ => return map_proxy_request_model(body, provider),
    }
    Ok(body)
}

pub fn resolve_proxy_request_route(
    body: &Value,
    provider: &Provider,
) -> Result<ResolvedModelRoute, AppError> {
    let requested = body
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::localized(
                "claude_desktop.provider.model_missing",
                "Claude Desktop 请求缺少 model 字段",
                "Claude Desktop request is missing model field",
            )
        })?;
    proxy_model_routes(provider)?
        .into_iter()
        .find(|route| route.route_id == requested)
        .ok_or_else(|| {
            AppError::localized(
                "claude_desktop.provider.route_unknown",
                format!("Claude Desktop 模型路由未配置: {requested}"),
                format!("Claude Desktop model route is not configured: {requested}"),
            )
        })
}

pub fn proxy_api_format(provider: &Provider) -> Option<&'static str> {
    provider
        .meta
        .as_ref()
        .and_then(|meta| meta.api_format())
        .map(ProviderApiFormat::as_str)
}

fn map_openai_compatible_tool_choice(body: &mut Value) -> Result<(), AppError> {
    let Some(tool_choice) = body.get_mut("tool_choice") else {
        return Ok(());
    };

    match tool_choice {
        Value::String(choice) if choice == "any" => {
            *tool_choice = json!("required");
            Ok(())
        }
        Value::String(choice) if matches!(choice.as_str(), "auto" | "none" | "required") => Ok(()),
        Value::String(choice) if choice == "required_auto" => {
            *tool_choice = json!("required");
            Ok(())
        }
        Value::Object(choice) => {
            let choice_type = choice
                .get("type")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            match choice_type {
                "auto" => {
                    *tool_choice = json!("auto");
                    Ok(())
                }
                "none" => {
                    *tool_choice = json!("none");
                    Ok(())
                }
                "any" => {
                    *tool_choice = json!("required");
                    Ok(())
                }
                "tool" | "function" => {
                    let name = choice
                        .get("name")
                        .or_else(|| {
                            choice
                                .get("function")
                                .and_then(|function| function.get("name"))
                        })
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| {
                            AppError::localized(
                                "claude_desktop.provider.tool_choice_name_missing",
                                "Claude Desktop tool_choice 指定工具时缺少 name",
                                "Claude Desktop tool_choice is missing name for a forced tool",
                            )
                        })?;
                    *tool_choice = json!({
                        "type": "function",
                        "function": {
                            "name": name,
                        },
                    });
                    Ok(())
                }
                _ => Err(AppError::localized(
                    "claude_desktop.provider.tool_choice_unsupported",
                    format!("Claude Desktop 本地路由暂不支持 tool_choice 类型: {choice_type}"),
                    format!("Claude Desktop proxy mode does not support tool_choice type: {choice_type}"),
                )),
            }
        }
        _ => Err(AppError::localized(
            "claude_desktop.provider.tool_choice_invalid",
            "Claude Desktop tool_choice 格式无效",
            "Claude Desktop tool_choice has an invalid shape",
        )),
    }
}

fn map_openai_responses_tool_choice(body: &mut Value) -> Result<(), AppError> {
    let Some(tool_choice) = body.get_mut("tool_choice") else {
        return Ok(());
    };

    match tool_choice {
        Value::String(choice) if choice == "any" || choice == "required_auto" => {
            *tool_choice = json!("required");
            Ok(())
        }
        Value::String(choice) if matches!(choice.as_str(), "auto" | "none" | "required") => Ok(()),
        Value::Object(choice) => {
            let choice_type = choice
                .get("type")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            match choice_type {
                "auto" => {
                    *tool_choice = json!("auto");
                    Ok(())
                }
                "none" => {
                    *tool_choice = json!("none");
                    Ok(())
                }
                "any" => {
                    *tool_choice = json!("required");
                    Ok(())
                }
                "tool" | "function" => {
                    let name = choice
                        .get("name")
                        .or_else(|| {
                            choice
                                .get("function")
                                .and_then(|function| function.get("name"))
                        })
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| {
                            AppError::localized(
                                "claude_desktop.provider.tool_choice_name_missing",
                                "Claude Desktop tool_choice 指定工具时缺少 name",
                                "Claude Desktop tool_choice is missing name for a forced tool",
                            )
                        })?;
                    *tool_choice = json!({
                        "type": "function",
                        "name": name,
                    });
                    Ok(())
                }
                _ => Err(AppError::localized(
                    "claude_desktop.provider.tool_choice_unsupported",
                    format!("Claude Desktop 本地路由暂不支持 tool_choice 类型: {choice_type}"),
                    format!("Claude Desktop proxy mode does not support tool_choice type: {choice_type}"),
                )),
            }
        }
        _ => Err(AppError::localized(
            "claude_desktop.provider.tool_choice_invalid",
            "Claude Desktop tool_choice 格式无效",
            "Claude Desktop tool_choice has an invalid shape",
        )),
    }
}

fn map_anthropic_messages_to_openai_chat(body: Value) -> Result<Value, AppError> {
    let mut obj = body.as_object().cloned().ok_or_else(|| {
        AppError::localized(
            "claude_desktop.provider.body_invalid",
            "Claude Desktop 请求体必须是 JSON 对象",
            "Claude Desktop request body must be a JSON object",
        )
    })?;

    if let Some(system) = obj.remove("system") {
        if let Some(content) = anthropic_content_to_text(&system) {
            let content = strip_leading_anthropic_billing_header(&content).to_string();
            let message = json!({
                "role": "system",
                "content": content,
            });
            match obj.get_mut("messages") {
                Some(Value::Array(messages)) => messages.insert(0, message),
                _ => {
                    obj.insert("messages".to_string(), json!([message]));
                }
            }
        }
    }
    if let Some(messages) = obj.remove("messages") {
        obj.insert(
            "messages".to_string(),
            map_anthropic_messages_to_openai_chat_messages(messages),
        );
    }
    if let Some(tools) = obj.get_mut("tools") {
        *tools = map_anthropic_tools_to_openai_chat(tools);
    }
    rename_field(&mut obj, "stop_sequences", "stop");
    apply_openai_chat_model_parameters(&mut obj);

    Ok(Value::Object(obj))
}

fn map_anthropic_messages_to_openai_responses(body: Value) -> Result<Value, AppError> {
    let mut obj = body.as_object().cloned().ok_or_else(|| {
        AppError::localized(
            "claude_desktop.provider.body_invalid",
            "Claude Desktop 请求体必须是 JSON 对象",
            "Claude Desktop request body must be a JSON object",
        )
    })?;

    if let Some(system) = obj.remove("system") {
        if let Some(instructions) = anthropic_content_to_text(&system) {
            obj.insert("instructions".to_string(), Value::String(instructions));
        }
    }
    if let Some(messages) = obj.remove("messages") {
        obj.insert(
            "input".to_string(),
            map_anthropic_messages_to_responses_input(messages),
        );
    }
    if let Some(max_tokens) = obj.remove("max_tokens") {
        obj.insert("max_output_tokens".to_string(), max_tokens);
    }
    apply_openai_responses_reasoning_parameters(&mut obj);
    if let Some(tools) = obj.get_mut("tools") {
        *tools = map_anthropic_tools_to_openai_responses(tools);
    }
    rename_field(&mut obj, "stop_sequences", "stop");

    Ok(Value::Object(obj))
}

fn apply_openai_responses_provider_meta(body: &mut Value, provider: &Provider) {
    let Some(obj) = body.as_object_mut() else {
        return;
    };
    let Some(meta) = provider.meta.as_ref() else {
        return;
    };

    if let Some(cache_key) = meta
        .prompt_cache_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        obj.insert(
            "prompt_cache_key".to_string(),
            Value::String(cache_key.to_string()),
        );
    }

    if meta.provider_type() != Some(ProviderType::CodexOauth) {
        return;
    }

    obj.insert("store".to_string(), Value::Bool(false));
    if meta.codex_fast_mode.unwrap_or(false) {
        obj.insert(
            "service_tier".to_string(),
            Value::String("priority".to_string()),
        );
    } else {
        obj.remove("service_tier");
    }

    const REASONING_MARKER: &str = "reasoning.encrypted_content";
    let mut includes = obj
        .remove("include")
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();
    if !includes
        .iter()
        .any(|value| value.as_str() == Some(REASONING_MARKER))
    {
        includes.push(Value::String(REASONING_MARKER.to_string()));
    }
    obj.insert("include".to_string(), Value::Array(includes));

    // Keep this aligned with upstream cc-switch's Codex OAuth Responses contract.
    // ChatGPT's codex backend rejects these normal OpenAI/Anthropic parameters.
    obj.remove("max_output_tokens");
    obj.remove("max_tokens");
    obj.remove("temperature");
    obj.remove("top_p");
    obj.remove("stop");
    obj.remove("stop_sequences");
    obj.remove("thinking");

    obj.entry("instructions".to_string())
        .or_insert_with(|| Value::String(String::new()));
    obj.entry("tools".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    obj.entry("parallel_tool_calls".to_string())
        .or_insert(Value::Bool(false));
    obj.insert("stream".to_string(), Value::Bool(true));
}

fn rename_field(obj: &mut serde_json::Map<String, Value>, from: &str, to: &str) {
    if obj.contains_key(to) {
        obj.remove(from);
        return;
    }
    if let Some(value) = obj.remove(from) {
        obj.insert(to.to_string(), value);
    }
}

fn apply_openai_chat_model_parameters(obj: &mut Map<String, Value>) {
    let model = obj
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if let Some(max_tokens) = obj.remove("max_tokens") {
        if is_openai_o_series(&model) {
            obj.insert("max_completion_tokens".to_string(), max_tokens);
        } else {
            obj.insert("max_tokens".to_string(), max_tokens);
        }
    }
    if supports_reasoning_effort(&model) {
        if let Some(effort) = resolve_openai_reasoning_effort(obj) {
            obj.insert(
                "reasoning_effort".to_string(),
                Value::String(effort.to_string()),
            );
        }
    }
    obj.remove("thinking");
    obj.remove("output_config");
}

fn apply_openai_responses_reasoning_parameters(obj: &mut Map<String, Value>) {
    let model = obj
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if supports_reasoning_effort(&model) {
        if let Some(effort) = resolve_openai_reasoning_effort(obj) {
            obj.insert("reasoning".to_string(), json!({ "effort": effort }));
        }
    }
    obj.remove("thinking");
    obj.remove("output_config");
}

fn is_openai_o_series(model: &str) -> bool {
    model.len() > 1
        && model.starts_with('o')
        && model.as_bytes().get(1).is_some_and(u8::is_ascii_digit)
}

fn supports_reasoning_effort(model: &str) -> bool {
    is_openai_o_series(model)
        || model
            .to_ascii_lowercase()
            .strip_prefix("gpt-")
            .and_then(|rest| rest.chars().next())
            .is_some_and(|value| value.is_ascii_digit() && value >= '5')
}

fn resolve_openai_reasoning_effort(obj: &Map<String, Value>) -> Option<&'static str> {
    if let Some(effort) = obj
        .get("output_config")
        .and_then(|value| value.get("effort"))
        .and_then(Value::as_str)
    {
        return match effort {
            "low" => Some("low"),
            "medium" => Some("medium"),
            "high" => Some("high"),
            "max" => Some("xhigh"),
            _ => None,
        };
    }

    let thinking = obj.get("thinking")?;
    match thinking.get("type").and_then(Value::as_str) {
        Some("adaptive") => Some("xhigh"),
        Some("enabled") => match thinking.get("budget_tokens").and_then(Value::as_u64) {
            Some(budget) if budget < 4_000 => Some("low"),
            Some(budget) if budget < 16_000 => Some("medium"),
            Some(_) => Some("high"),
            None => Some("high"),
        },
        _ => None,
    }
}

fn strip_leading_anthropic_billing_header(text: &str) -> &str {
    if !text.starts_with(ANTHROPIC_BILLING_HEADER_PREFIX) {
        return text;
    }
    let Some(line_end) = text
        .as_bytes()
        .iter()
        .position(|byte| *byte == b'\n' || *byte == b'\r')
    else {
        return "";
    };
    let bytes = text.as_bytes();
    let mut rest_start = line_end + 1;
    if bytes[line_end] == b'\r' && bytes.get(line_end + 1) == Some(&b'\n') {
        rest_start += 1;
    }
    let rest = &text[rest_start..];
    rest.strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))
        .or_else(|| rest.strip_prefix('\r'))
        .unwrap_or(rest)
}

fn map_anthropic_messages_to_openai_chat_messages(messages: Value) -> Value {
    let Value::Array(items) = messages else {
        return messages;
    };

    Value::Array(
        items
            .into_iter()
            .flat_map(map_anthropic_message_to_openai_chat_messages)
            .collect(),
    )
}

fn map_anthropic_message_to_openai_chat_messages(message: Value) -> Vec<Value> {
    let Some(mut obj) = message.as_object().cloned() else {
        return vec![message];
    };
    let role = obj
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("user")
        .to_string();
    let Some(content) = obj.remove("content") else {
        return vec![Value::Object(obj)];
    };

    match content {
        Value::String(text) => {
            obj.insert("content".to_string(), Value::String(text));
            vec![Value::Object(obj)]
        }
        Value::Array(blocks) => openai_chat_messages_from_content_blocks(obj, &role, blocks),
        other => {
            obj.insert("content".to_string(), other);
            vec![Value::Object(obj)]
        }
    }
}

fn openai_chat_messages_from_content_blocks(
    mut obj: serde_json::Map<String, Value>,
    role: &str,
    blocks: Vec<Value>,
) -> Vec<Value> {
    let mut result = Vec::new();
    let mut content_parts = Vec::new();
    let mut tool_calls = Vec::new();

    for block in blocks {
        match block
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "text" => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    let mut part = json!({"type": "text", "text": text});
                    if let Some(cache_control) = block.get("cache_control") {
                        part["cache_control"] = cache_control.clone();
                    }
                    content_parts.push(part);
                }
            }
            "image" => {
                if let Some(source) = block.get("source") {
                    let media_type = source
                        .get("media_type")
                        .and_then(Value::as_str)
                        .unwrap_or("image/png");
                    let data = source.get("data").and_then(Value::as_str).unwrap_or("");
                    content_parts.push(json!({
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:{media_type};base64,{data}")
                        }
                    }));
                }
            }
            "tool_use" => {
                let id = block.get("id").and_then(Value::as_str).unwrap_or("");
                let name = block.get("name").and_then(Value::as_str).unwrap_or("");
                let input = block.get("input").cloned().unwrap_or_else(|| json!({}));
                tool_calls.push(json!({
                    "id": id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": canonical_json_string(&input)
                    }
                }));
            }
            "tool_result" => {
                let tool_use_id = block
                    .get("tool_use_id")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let content = match block.get("content") {
                    Some(Value::String(text)) => text.clone(),
                    Some(value) => canonical_json_string(value),
                    None => String::new(),
                };
                result.push(json!({
                    "role": "tool",
                    "tool_call_id": tool_use_id,
                    "content": content
                }));
            }
            _ => {}
        }
    }

    if !content_parts.is_empty() || !tool_calls.is_empty() {
        obj.insert("role".to_string(), Value::String(role.to_string()));
        obj.insert(
            "content".to_string(),
            openai_chat_content_from_parts(content_parts),
        );
        if !tool_calls.is_empty() {
            obj.insert("tool_calls".to_string(), Value::Array(tool_calls));
        }
        result.insert(0, Value::Object(obj));
    }

    result
}

fn openai_chat_content_from_parts(content_parts: Vec<Value>) -> Value {
    if content_parts.is_empty() {
        return Value::Null;
    }
    if content_parts.len() == 1 && content_parts[0].get("cache_control").is_none() {
        if let Some(text) = content_parts[0].get("text") {
            return text.clone();
        }
    }
    Value::Array(content_parts)
}

fn map_anthropic_messages_to_responses_input(messages: Value) -> Value {
    let Value::Array(items) = messages else {
        return messages;
    };

    Value::Array(
        items
            .into_iter()
            .flat_map(map_anthropic_message_to_responses_input)
            .collect(),
    )
}

fn map_anthropic_message_to_responses_input(message: Value) -> Vec<Value> {
    let Some(obj) = message.as_object() else {
        return vec![message];
    };
    let role = obj.get("role").and_then(Value::as_str).unwrap_or("user");
    match obj.get("content") {
        Some(Value::String(text)) => {
            let content_type = if role == "assistant" {
                "output_text"
            } else {
                "input_text"
            };
            vec![json!({
                "role": role,
                "content": [{ "type": content_type, "text": text }]
            })]
        }
        Some(Value::Array(blocks)) => responses_input_from_content_blocks(role, blocks),
        _ => vec![message],
    }
}

fn responses_input_from_content_blocks(role: &str, blocks: &[Value]) -> Vec<Value> {
    let mut input = Vec::new();
    let mut message_content = Vec::new();

    for block in blocks {
        match block
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "text" => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    let content_type = if role == "assistant" {
                        "output_text"
                    } else {
                        "input_text"
                    };
                    message_content.push(json!({ "type": content_type, "text": text }));
                }
            }
            "image" => {
                if let Some(source) = block.get("source") {
                    let media_type = source
                        .get("media_type")
                        .and_then(Value::as_str)
                        .unwrap_or("image/png");
                    let data = source.get("data").and_then(Value::as_str).unwrap_or("");
                    message_content.push(json!({
                        "type": "input_image",
                        "image_url": format!("data:{media_type};base64,{data}")
                    }));
                }
            }
            "tool_use" => {
                flush_responses_message_content(&mut input, role, &mut message_content);
                let id = block.get("id").and_then(Value::as_str).unwrap_or("");
                let name = block.get("name").and_then(Value::as_str).unwrap_or("");
                let arguments = block.get("input").cloned().unwrap_or_else(|| json!({}));
                input.push(json!({
                    "type": "function_call",
                    "call_id": id,
                    "name": name,
                    "arguments": canonical_json_string(&arguments)
                }));
            }
            "tool_result" => {
                flush_responses_message_content(&mut input, role, &mut message_content);
                let call_id = block
                    .get("tool_use_id")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let output = match block.get("content") {
                    Some(Value::String(text)) => text.clone(),
                    Some(value) => canonical_json_string(value),
                    None => String::new(),
                };
                input.push(json!({
                    "type": "function_call_output",
                    "call_id": call_id,
                    "output": output
                }));
            }
            _ => {}
        }
    }

    flush_responses_message_content(&mut input, role, &mut message_content);
    input
}

fn flush_responses_message_content(input: &mut Vec<Value>, role: &str, content: &mut Vec<Value>) {
    if content.is_empty() {
        return;
    }
    input.push(json!({
        "role": role,
        "content": std::mem::take(content)
    }));
}

fn map_anthropic_messages_to_gemini_native(body: Value) -> Result<Value, AppError> {
    map_anthropic_messages_to_gemini_native_with_shadow(body, &[])
}

fn map_anthropic_messages_to_gemini_native_with_shadow(
    body: Value,
    shadow_turns: &[GeminiAssistantTurn],
) -> Result<Value, AppError> {
    let obj = body.as_object().ok_or_else(|| {
        AppError::localized(
            "claude_desktop.provider.body_invalid",
            "Claude Desktop 请求体必须是 JSON 对象",
            "Claude Desktop request body must be a JSON object",
        )
    })?;
    let mut result = Map::new();

    if let Some(system_instruction) = gemini_system_instruction(obj.get("system"))? {
        result.insert("systemInstruction".to_string(), system_instruction);
    }
    if let Some(messages) = obj.get("messages") {
        result.insert(
            "contents".to_string(),
            map_anthropic_messages_to_gemini_contents_with_shadow(messages, shadow_turns)?,
        );
    }
    if let Some(generation_config) = gemini_generation_config(obj) {
        result.insert("generationConfig".to_string(), generation_config);
    }
    if let Some(tools) = obj.get("tools") {
        if let Some(mapped_tools) = map_anthropic_tools_to_gemini(tools) {
            result.insert("tools".to_string(), mapped_tools);
        }
    }
    if let Some(tool_config) = map_anthropic_tool_choice_to_gemini(obj.get("tool_choice"))? {
        result.insert("toolConfig".to_string(), tool_config);
    }

    Ok(Value::Object(result))
}

pub fn map_gemini_native_response_to_anthropic(body: Value) -> Result<Value, AppError> {
    map_gemini_native_response_to_anthropic_with_shadow(body, None, None, None)
}

pub fn map_gemini_native_response_to_anthropic_with_shadow(
    body: Value,
    shadow_store: Option<&GeminiShadowStore>,
    provider_id: Option<&str>,
    session_id: Option<&str>,
) -> Result<Value, AppError> {
    map_gemini_native_response_to_anthropic_with_shadow_and_hints(
        body,
        shadow_store,
        provider_id,
        session_id,
        None,
    )
}

pub fn map_gemini_native_response_to_anthropic_with_shadow_and_hints(
    body: Value,
    shadow_store: Option<&GeminiShadowStore>,
    provider_id: Option<&str>,
    session_id: Option<&str>,
    tool_schema_hints: Option<&AnthropicToolSchemaHints>,
) -> Result<Value, AppError> {
    if let Some(block_reason) = body
        .get("promptFeedback")
        .and_then(|value| value.get("blockReason"))
        .and_then(Value::as_str)
    {
        return Ok(json!({
            "id": body.get("responseId").and_then(Value::as_str).unwrap_or(""),
            "type": "message",
            "role": "assistant",
            "content": [{
                "type": "text",
                "text": format!("Request blocked by Gemini safety filters: {block_reason}")
            }],
            "model": body.get("modelVersion").and_then(Value::as_str).unwrap_or(""),
            "stop_reason": "refusal",
            "stop_sequence": Value::Null,
            "usage": gemini_usage_to_anthropic(body.get("usageMetadata"))
        }));
    }

    let candidate = body
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .ok_or_else(|| AppError::InvalidInput("No candidates in Gemini response".to_string()))?;
    let parts = candidate
        .get("content")
        .and_then(|content| content.get("parts"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut parts = parts;
    rectify_gemini_tool_call_parts(&mut parts, tool_schema_hints);
    for (index, part) in parts.iter_mut().enumerate() {
        let Some(function_call) = part.get_mut("functionCall").and_then(Value::as_object_mut)
        else {
            continue;
        };
        let needs_synthesized_id = function_call
            .get("id")
            .and_then(Value::as_str)
            .map(str::is_empty)
            .unwrap_or(true);
        if needs_synthesized_id {
            function_call.insert(
                "id".to_string(),
                json!(format!("{GEMINI_SYNTHESIZED_ID_PREFIX}{index}")),
            );
        }
    }
    let mut content = Vec::new();
    let mut has_tool_use = false;

    for (index, part) in parts.iter().enumerate() {
        if part.get("thought").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        if let Some(text) = part.get("text").and_then(Value::as_str) {
            if !text.is_empty() {
                content.push(json!({
                    "type": "text",
                    "text": text
                }));
            }
            continue;
        }
        if let Some(function_call) = part.get("functionCall") {
            has_tool_use = true;
            let id = function_call
                .get("id")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| format!("{GEMINI_SYNTHESIZED_ID_PREFIX}{index}"));
            content.push(json!({
                "type": "tool_use",
                "id": id,
                "name": function_call.get("name").and_then(Value::as_str).unwrap_or(""),
                "input": function_call.get("args").cloned().unwrap_or_else(|| json!({}))
            }));
        }
    }

    if let (Some(store), Some(provider_id), Some(session_id)) =
        (shadow_store, provider_id, session_id)
    {
        if !parts.is_empty() {
            store.record_assistant_turn(
                provider_id,
                session_id,
                json!({ "parts": parts.clone() }),
                extract_gemini_tool_call_meta(&parts),
            );
        }
    }

    Ok(json!({
        "id": body.get("responseId").and_then(Value::as_str).unwrap_or(""),
        "type": "message",
        "role": "assistant",
        "content": content,
        "model": body.get("modelVersion").and_then(Value::as_str).unwrap_or(""),
        "stop_reason": gemini_finish_reason_to_anthropic(
            candidate.get("finishReason").and_then(Value::as_str),
            has_tool_use,
        ),
        "stop_sequence": Value::Null,
        "usage": gemini_usage_to_anthropic(body.get("usageMetadata"))
    }))
}

pub fn map_openai_chat_response_to_anthropic(body: Value) -> Result<Value, AppError> {
    let choices = body
        .get("choices")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Config("No choices in OpenAI Chat response".to_string()))?;
    let choice = choices.first().ok_or_else(|| {
        AppError::Config("Empty choices array in OpenAI Chat response".to_string())
    })?;
    let message = choice
        .get("message")
        .ok_or_else(|| AppError::Config("No message in OpenAI Chat choice".to_string()))?;

    let mut content = Vec::new();
    let mut has_tool_use = false;

    if let Some(reasoning_content) = message.get("reasoning_content").and_then(Value::as_str) {
        if !reasoning_content.is_empty() {
            content.push(json!({
                "type": "thinking",
                "thinking": reasoning_content
            }));
        }
    }

    if let Some(message_content) = message.get("content") {
        if let Some(text) = message_content.as_str() {
            if !text.is_empty() {
                content.push(json!({
                    "type": "text",
                    "text": text
                }));
            }
        } else if let Some(parts) = message_content.as_array() {
            for part in parts {
                match part.get("type").and_then(Value::as_str).unwrap_or("") {
                    "text" | "output_text" => {
                        if let Some(text) = part.get("text").and_then(Value::as_str) {
                            if !text.is_empty() {
                                content.push(json!({
                                    "type": "text",
                                    "text": text
                                }));
                            }
                        }
                    }
                    "refusal" => {
                        if let Some(refusal) = part.get("refusal").and_then(Value::as_str) {
                            if !refusal.is_empty() {
                                content.push(json!({
                                    "type": "text",
                                    "text": refusal
                                }));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if let Some(refusal) = message.get("refusal").and_then(Value::as_str) {
        if !refusal.is_empty() {
            content.push(json!({
                "type": "text",
                "text": refusal
            }));
        }
    }

    if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
        if !tool_calls.is_empty() {
            has_tool_use = true;
        }
        for tool_call in tool_calls {
            let id = tool_call.get("id").and_then(Value::as_str).unwrap_or("");
            let function = tool_call.get("function").unwrap_or(&Value::Null);
            let name = function.get("name").and_then(Value::as_str).unwrap_or("");
            let arguments = function
                .get("arguments")
                .and_then(Value::as_str)
                .unwrap_or("{}");
            let input = serde_json::from_str::<Value>(arguments).unwrap_or_else(|_| json!({}));

            content.push(json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": input
            }));
        }
    }

    if !has_tool_use {
        if let Some(function_call) = message.get("function_call") {
            let id = function_call
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("");
            let name = function_call
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("");
            let has_arguments = function_call.get("arguments").is_some();
            let input = match function_call.get("arguments") {
                Some(Value::String(raw)) => {
                    serde_json::from_str::<Value>(raw).unwrap_or_else(|_| json!({}))
                }
                Some(value @ Value::Object(_)) | Some(value @ Value::Array(_)) => value.clone(),
                _ => json!({}),
            };

            if !name.is_empty() || has_arguments {
                content.push(json!({
                    "type": "tool_use",
                    "id": id,
                    "name": name,
                    "input": input
                }));
                has_tool_use = true;
            }
        }
    }

    let stop_reason = choice
        .get("finish_reason")
        .and_then(Value::as_str)
        .map(|reason| match reason {
            "stop" => "end_turn",
            "length" => "max_tokens",
            "tool_calls" | "function_call" => "tool_use",
            "content_filter" => "end_turn",
            other => {
                log::warn!(
                    "[Claude/OpenAI] Unknown finish_reason in non-streaming response: {other}"
                );
                "end_turn"
            }
        })
        .or(if has_tool_use { Some("tool_use") } else { None });

    Ok(json!({
        "id": body.get("id").and_then(Value::as_str).unwrap_or(""),
        "type": "message",
        "role": "assistant",
        "content": content,
        "model": body.get("model").and_then(Value::as_str).unwrap_or(""),
        "stop_reason": stop_reason,
        "stop_sequence": Value::Null,
        "usage": openai_chat_usage_to_anthropic(body.get("usage"))
    }))
}

pub fn map_openai_responses_response_to_anthropic(body: Value) -> Result<Value, AppError> {
    let output = body
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Config("No output in OpenAI Responses response".to_string()))?;

    let mut content = Vec::new();
    let mut has_tool_use = false;

    for item in output {
        match item.get("type").and_then(Value::as_str).unwrap_or("") {
            "message" => {
                if let Some(message_content) = item.get("content").and_then(Value::as_array) {
                    for block in message_content {
                        match block.get("type").and_then(Value::as_str).unwrap_or("") {
                            "output_text" => {
                                if let Some(text) = block.get("text").and_then(Value::as_str) {
                                    if !text.is_empty() {
                                        content.push(json!({
                                            "type": "text",
                                            "text": text
                                        }));
                                    }
                                }
                            }
                            "refusal" => {
                                if let Some(refusal) = block.get("refusal").and_then(Value::as_str)
                                {
                                    if !refusal.is_empty() {
                                        content.push(json!({
                                            "type": "text",
                                            "text": refusal
                                        }));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            "function_call" => {
                let call_id = item.get("call_id").and_then(Value::as_str).unwrap_or("");
                let name = item.get("name").and_then(Value::as_str).unwrap_or("");
                let arguments = item
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or("{}");
                let input = serde_json::from_str::<Value>(arguments).unwrap_or_else(|_| json!({}));
                content.push(json!({
                    "type": "tool_use",
                    "id": call_id,
                    "name": name,
                    "input": sanitize_anthropic_tool_use_input(name, input)
                }));
                has_tool_use = true;
            }
            "reasoning" => {
                if let Some(summary) = item.get("summary").and_then(Value::as_array) {
                    let thinking = summary
                        .iter()
                        .filter_map(|entry| {
                            (entry.get("type").and_then(Value::as_str) == Some("summary_text"))
                                .then(|| entry.get("text").and_then(Value::as_str))
                                .flatten()
                        })
                        .collect::<Vec<_>>()
                        .join("");
                    if !thinking.is_empty() {
                        content.push(json!({
                            "type": "thinking",
                            "thinking": thinking
                        }));
                    }
                }
            }
            _ => {}
        }
    }

    let stop_reason = map_responses_stop_reason(
        body.get("status").and_then(Value::as_str),
        has_tool_use,
        body.pointer("/incomplete_details/reason")
            .and_then(Value::as_str),
    );

    Ok(json!({
        "id": body.get("id").and_then(Value::as_str).unwrap_or(""),
        "type": "message",
        "role": "assistant",
        "content": content,
        "model": body.get("model").and_then(Value::as_str).unwrap_or(""),
        "stop_reason": stop_reason,
        "stop_sequence": Value::Null,
        "usage": openai_responses_usage_to_anthropic(body.get("usage"))
    }))
}

fn sanitize_anthropic_tool_use_input(name: &str, input: Value) -> Value {
    if name != "Read" {
        return input;
    }

    match input {
        Value::Object(mut object) => {
            if matches!(object.get("pages"), Some(Value::String(value)) if value.is_empty()) {
                object.remove("pages");
            }
            Value::Object(object)
        }
        other => other,
    }
}

pub(crate) fn openai_chat_usage_to_anthropic(usage: Option<&Value>) -> Value {
    let Some(usage) = usage else {
        return json!({
            "input_tokens": 0,
            "output_tokens": 0
        });
    };

    let mut result = json!({
        "input_tokens": usage
            .get("prompt_tokens")
            .and_then(Value::as_u64)
            .or_else(|| usage.get("input_tokens").and_then(Value::as_u64))
            .unwrap_or(0),
        "output_tokens": usage
            .get("completion_tokens")
            .and_then(Value::as_u64)
            .or_else(|| usage.get("output_tokens").and_then(Value::as_u64))
            .unwrap_or(0)
    });

    if let Some(cached) = usage
        .pointer("/prompt_tokens_details/cached_tokens")
        .and_then(Value::as_u64)
        .or_else(|| {
            usage
                .pointer("/input_tokens_details/cached_tokens")
                .and_then(Value::as_u64)
        })
    {
        result["cache_read_input_tokens"] = json!(cached);
    }
    if let Some(value) = usage.get("cache_read_input_tokens") {
        result["cache_read_input_tokens"] = value.clone();
    }
    if let Some(value) = usage.get("cache_creation_input_tokens") {
        result["cache_creation_input_tokens"] = value.clone();
    }

    result
}

pub(crate) fn openai_responses_usage_to_anthropic(usage: Option<&Value>) -> Value {
    let Some(usage) = usage.filter(|value| value.is_object()) else {
        return json!({
            "input_tokens": 0,
            "output_tokens": 0
        });
    };

    let mut result = json!({
        "input_tokens": usage
            .get("input_tokens")
            .and_then(Value::as_u64)
            .or_else(|| usage.get("prompt_tokens").and_then(Value::as_u64))
            .unwrap_or(0),
        "output_tokens": usage
            .get("output_tokens")
            .and_then(Value::as_u64)
            .or_else(|| usage.get("completion_tokens").and_then(Value::as_u64))
            .unwrap_or(0)
    });

    if let Some(cached) = usage
        .pointer("/input_tokens_details/cached_tokens")
        .and_then(Value::as_u64)
        .or_else(|| {
            usage
                .pointer("/prompt_tokens_details/cached_tokens")
                .and_then(Value::as_u64)
        })
    {
        result["cache_read_input_tokens"] = json!(cached);
    }
    if let Some(value) = usage.get("cache_read_input_tokens") {
        result["cache_read_input_tokens"] = value.clone();
    }
    if let Some(value) = usage.get("cache_creation_input_tokens") {
        result["cache_creation_input_tokens"] = value.clone();
    }

    result
}

pub(crate) fn map_responses_stop_reason(
    status: Option<&str>,
    has_tool_use: bool,
    incomplete_reason: Option<&str>,
) -> Option<&'static str> {
    status.map(|status| match status {
        "completed" if has_tool_use => "tool_use",
        "incomplete"
            if matches!(
                incomplete_reason,
                Some("max_output_tokens") | Some("max_tokens")
            ) || incomplete_reason.is_none() =>
        {
            "max_tokens"
        }
        "incomplete" => "end_turn",
        _ => "end_turn",
    })
}

fn gemini_system_instruction(system: Option<&Value>) -> Result<Option<Value>, AppError> {
    let Some(system) = system else {
        return Ok(None);
    };
    let text = anthropic_content_to_text(system).unwrap_or_default();
    if text.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(json!({
        "parts": [{ "text": text }]
    })))
}

fn gemini_generation_config(obj: &Map<String, Value>) -> Option<Value> {
    let mut config = Map::new();
    if let Some(value) = obj.get("max_tokens") {
        config.insert("maxOutputTokens".to_string(), value.clone());
    }
    if let Some(value) = obj.get("temperature") {
        config.insert("temperature".to_string(), value.clone());
    }
    if let Some(value) = obj.get("top_p") {
        config.insert("topP".to_string(), value.clone());
    }
    if let Some(value) = obj.get("stop_sequences") {
        config.insert("stopSequences".to_string(), value.clone());
    }
    (!config.is_empty()).then_some(Value::Object(config))
}

fn map_anthropic_messages_to_gemini_contents_with_shadow(
    messages: &Value,
    shadow_turns: &[GeminiAssistantTurn],
) -> Result<Value, AppError> {
    let Value::Array(items) = messages else {
        return Err(AppError::localized(
            "claude_desktop.provider.messages_invalid",
            "Claude Desktop messages 必须是数组",
            "Claude Desktop messages must be an array",
        ));
    };
    let mut tool_name_by_id = HashMap::new();
    for turn in shadow_turns {
        merge_gemini_tool_names_from_shadow(turn, &mut tool_name_by_id);
    }

    let total_assistant_messages = items
        .iter()
        .filter(|message| message.get("role").and_then(Value::as_str) == Some("assistant"))
        .count();
    let effective_shadow_turns = if shadow_turns.len() > total_assistant_messages {
        &shadow_turns[shadow_turns.len() - total_assistant_messages..]
    } else {
        shadow_turns
    };
    let shadow_start_index = total_assistant_messages.saturating_sub(effective_shadow_turns.len());
    let mut assistant_seen_index = 0usize;
    let mut used_shadow_indices = HashSet::new();
    let mut contents = Vec::with_capacity(items.len());

    for message in items {
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("user");
        let parts = if role == "assistant" {
            let positional_shadow_index = assistant_seen_index
                .checked_sub(shadow_start_index)
                .filter(|index| *index < effective_shadow_turns.len())
                .filter(|index| !used_shadow_indices.contains(index));
            let tool_use_match_index =
                find_matching_gemini_shadow_turn(message.get("content"), effective_shadow_turns)
                    .filter(|index| !used_shadow_indices.contains(index));
            assistant_seen_index = assistant_seen_index.saturating_add(1);

            if let Some(index) = tool_use_match_index.or(positional_shadow_index) {
                used_shadow_indices.insert(index);
                let turn = &effective_shadow_turns[index];
                merge_gemini_tool_names_from_shadow(turn, &mut tool_name_by_id);
                if let Some(parts) = shadow_gemini_parts(&turn.assistant_content) {
                    parts
                } else {
                    map_anthropic_content_to_gemini_parts(
                        message.get("content"),
                        role,
                        &mut tool_name_by_id,
                    )?
                }
            } else {
                map_anthropic_content_to_gemini_parts(
                    message.get("content"),
                    role,
                    &mut tool_name_by_id,
                )?
            }
        } else {
            map_anthropic_content_to_gemini_parts(
                message.get("content"),
                role,
                &mut tool_name_by_id,
            )?
        };

        if role == "assistant" {
            merge_gemini_tool_names_from_parts(&parts, &mut tool_name_by_id);
        }

        contents.push(json!({
            "role": if role == "assistant" { "model" } else { "user" },
            "parts": parts
        }));
    }

    Ok(Value::Array(contents))
}

fn shadow_gemini_parts(content: &Value) -> Option<Vec<Value>> {
    let mut parts = content
        .get("parts")
        .and_then(Value::as_array)
        .map(|parts| parts.to_vec())?;
    for part in &mut parts {
        let Some(function_call) = part.get_mut("functionCall").and_then(Value::as_object_mut)
        else {
            continue;
        };
        let drop_id = function_call
            .get("id")
            .and_then(Value::as_str)
            .map(|id| id.is_empty() || is_gemini_synthesized_tool_id(id))
            .unwrap_or(true);
        if drop_id {
            function_call.remove("id");
        }
    }
    Some(parts)
}

fn merge_gemini_tool_names_from_shadow(
    turn: &GeminiAssistantTurn,
    tool_name_by_id: &mut HashMap<String, String>,
) {
    for tool_call in &turn.tool_calls {
        if let Some(id) = tool_call.id.as_deref().filter(|id| !id.is_empty()) {
            if !tool_call.name.is_empty() {
                tool_name_by_id.insert(id.to_string(), tool_call.name.clone());
            }
        }
    }
    if let Some(parts) = shadow_gemini_parts(&turn.assistant_content) {
        merge_gemini_tool_names_from_parts(&parts, tool_name_by_id);
    }
}

fn merge_gemini_tool_names_from_parts(
    parts: &[Value],
    tool_name_by_id: &mut HashMap<String, String>,
) {
    for part in parts {
        let Some(function_call) = part.get("functionCall") else {
            continue;
        };
        let Some(id) = function_call.get("id").and_then(Value::as_str) else {
            continue;
        };
        let Some(name) = function_call.get("name").and_then(Value::as_str) else {
            continue;
        };
        if !id.is_empty() && !name.is_empty() {
            tool_name_by_id.insert(id.to_string(), name.to_string());
        }
    }
}

fn find_matching_gemini_shadow_turn(
    content: Option<&Value>,
    shadow_turns: &[GeminiAssistantTurn],
) -> Option<usize> {
    let (tool_use_ids, tool_use_names) = extract_gemini_tool_use_keys(content);
    if tool_use_ids.is_empty() && tool_use_names.is_empty() {
        return None;
    }

    if !tool_use_ids.is_empty() {
        if let Some(index) = shadow_turns.iter().position(|turn| {
            turn.tool_calls.iter().any(|tool_call| {
                tool_call
                    .id
                    .as_deref()
                    .is_some_and(|id| tool_use_ids.contains(id))
            })
        }) {
            return Some(index);
        }
    }

    shadow_turns.iter().enumerate().find_map(|(index, turn)| {
        turn.tool_calls
            .iter()
            .any(|tool_call| {
                tool_use_names.contains(tool_call.name.as_str())
                    || tool_use_names.contains(normalize_gemini_tool_name(&tool_call.name))
            })
            .then_some(index)
    })
}

fn extract_gemini_tool_use_keys(content: Option<&Value>) -> (HashSet<String>, HashSet<String>) {
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    let Some(blocks) = content.and_then(Value::as_array) else {
        return (ids, names);
    };

    for block in blocks {
        if block.get("type").and_then(Value::as_str) != Some("tool_use") {
            continue;
        }
        if let Some(id) = block
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
        {
            ids.insert(id.to_string());
        }
        if let Some(name) = block
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
        {
            names.insert(name.to_string());
            names.insert(normalize_gemini_tool_name(name).to_string());
        }
    }

    (ids, names)
}

fn normalize_gemini_tool_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

pub(crate) fn extract_gemini_tool_call_meta(parts: &[Value]) -> Vec<GeminiToolCallMeta> {
    parts
        .iter()
        .filter_map(|part| {
            let function_call = part.get("functionCall")?;
            Some(GeminiToolCallMeta {
                id: function_call
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|id| !id.is_empty())
                    .map(ToString::to_string),
                name: function_call
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                args: function_call
                    .get("args")
                    .cloned()
                    .unwrap_or_else(|| json!({})),
                thought_signature: part
                    .get("thoughtSignature")
                    .or_else(|| part.get("thought_signature"))
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
            })
        })
        .collect()
}

fn map_anthropic_content_to_gemini_parts(
    content: Option<&Value>,
    role: &str,
    tool_name_by_id: &mut HashMap<String, String>,
) -> Result<Vec<Value>, AppError> {
    let Some(content) = content else {
        return Ok(Vec::new());
    };
    if let Some(text) = content.as_str() {
        return Ok(vec![json!({ "text": text })]);
    }
    let Some(blocks) = content.as_array() else {
        return Err(AppError::localized(
            "claude_desktop.provider.content_invalid",
            "Claude Desktop message content 必须是字符串或数组",
            "Claude Desktop message content must be a string or array",
        ));
    };

    let mut parts = Vec::new();
    for block in blocks {
        match block
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "text" => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    parts.push(json!({ "text": text }));
                }
            }
            "image" | "document" => {
                let source = block.get("source").ok_or_else(|| {
                    AppError::InvalidInput(
                        "Gemini Native content block is missing source".to_string(),
                    )
                })?;
                let source_type = source.get("type").and_then(Value::as_str).unwrap_or("");
                if source_type != "base64" {
                    return Err(AppError::InvalidInput(format!(
                        "Gemini Native only supports base64 content sources, got `{source_type}`"
                    )));
                }
                let default_mime = if block.get("type").and_then(Value::as_str) == Some("document")
                {
                    "application/pdf"
                } else {
                    "image/png"
                };
                parts.push(json!({
                    "inlineData": {
                        "mimeType": source
                            .get("media_type")
                            .and_then(Value::as_str)
                            .unwrap_or(default_mime),
                        "data": source.get("data").and_then(Value::as_str).unwrap_or("")
                    }
                }));
            }
            "tool_use" => {
                if role != "assistant" {
                    return Err(AppError::InvalidInput(
                        "Gemini Native tool_use blocks are only valid in assistant messages"
                            .to_string(),
                    ));
                }
                let id = block.get("id").and_then(Value::as_str).unwrap_or("");
                let name = block.get("name").and_then(Value::as_str).unwrap_or("");
                if !id.is_empty() && !name.is_empty() {
                    tool_name_by_id.insert(id.to_string(), name.to_string());
                }
                let mut function_call = json!({
                    "name": name,
                    "args": block.get("input").cloned().unwrap_or_else(|| json!({}))
                });
                if !id.is_empty() && !is_gemini_synthesized_tool_id(id) {
                    function_call["id"] = json!(id);
                }
                parts.push(json!({ "functionCall": function_call }));
            }
            "tool_result" => {
                let tool_use_id = block
                    .get("tool_use_id")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let name = tool_name_by_id
                    .get(tool_use_id)
                    .cloned()
                    .ok_or_else(|| {
                        AppError::InvalidInput(format!(
                            "Unable to resolve Gemini functionResponse.name for tool_use_id `{tool_use_id}`"
                        ))
                    })?;
                let mut function_response = json!({
                    "name": name,
                    "response": normalize_gemini_tool_result_response(block.get("content"))
                });
                if !tool_use_id.is_empty() && !is_gemini_synthesized_tool_id(tool_use_id) {
                    function_response["id"] = json!(tool_use_id);
                }
                parts.push(json!({ "functionResponse": function_response }));
            }
            "thinking" | "redacted_thinking" => {}
            _ => {}
        }
    }
    Ok(parts)
}

fn is_gemini_synthesized_tool_id(id: &str) -> bool {
    id.starts_with(GEMINI_SYNTHESIZED_ID_PREFIX)
}

pub(crate) fn gemini_usage_to_anthropic(usage: Option<&Value>) -> Value {
    let Some(usage) = usage else {
        return json!({
            "input_tokens": 0,
            "output_tokens": 0
        });
    };
    let input_tokens = usage
        .get("promptTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .get("totalTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let mut result = json!({
        "input_tokens": input_tokens,
        "output_tokens": total_tokens.saturating_sub(input_tokens)
    });
    if let Some(cached) = usage.get("cachedContentTokenCount").and_then(Value::as_u64) {
        result["cache_read_input_tokens"] = json!(cached);
    }
    result
}

pub(crate) fn gemini_finish_reason_to_anthropic(reason: Option<&str>, has_tool_use: bool) -> Value {
    match reason {
        Some("MAX_TOKENS") => json!("max_tokens"),
        Some("STOP") | Some("FINISH_REASON_UNSPECIFIED") | None => {
            if has_tool_use {
                json!("tool_use")
            } else {
                json!("end_turn")
            }
        }
        Some("SAFETY")
        | Some("RECITATION")
        | Some("SPII")
        | Some("BLOCKLIST")
        | Some("PROHIBITED_CONTENT") => json!("refusal"),
        Some(_) => json!("end_turn"),
    }
}

fn normalize_gemini_tool_result_response(content: Option<&Value>) -> Value {
    match content {
        Some(Value::String(text)) => json!({ "content": text }),
        Some(Value::Array(blocks)) => {
            let texts = blocks
                .iter()
                .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>();
            if texts.is_empty() {
                json!({ "content": Value::Array(blocks.clone()) })
            } else {
                json!({ "content": texts.join("\n") })
            }
        }
        Some(value) => json!({ "content": value.clone() }),
        None => json!({ "content": "" }),
    }
}

fn map_anthropic_tools_to_gemini(tools: &Value) -> Option<Value> {
    let Value::Array(items) = tools else {
        return None;
    };
    let declarations = items
        .iter()
        .filter(|tool| tool.get("type").and_then(Value::as_str) != Some("BatchTool"))
        .filter_map(|tool| {
            let name = tool.get("name").and_then(Value::as_str)?;
            Some(build_gemini_function_declaration(
                name,
                tool.get("description").and_then(Value::as_str),
                tool.get("input_schema")
                    .or_else(|| tool.get("parameters"))
                    .cloned()
                    .unwrap_or_else(|| json!({})),
            ))
        })
        .collect::<Vec<_>>();
    (!declarations.is_empty()).then(|| {
        json!([{
            "functionDeclarations": declarations
        }])
    })
}

fn map_anthropic_tool_choice_to_gemini(
    tool_choice: Option<&Value>,
) -> Result<Option<Value>, AppError> {
    let Some(tool_choice) = tool_choice else {
        return Ok(None);
    };
    match tool_choice {
        Value::String(choice) => match choice.as_str() {
            "auto" => Ok(Some(json!({ "functionCallingConfig": { "mode": "AUTO" } }))),
            "none" => Ok(Some(json!({ "functionCallingConfig": { "mode": "NONE" } }))),
            "any" | "required" | "required_auto" => {
                Ok(Some(json!({ "functionCallingConfig": { "mode": "ANY" } })))
            }
            other => Err(AppError::InvalidInput(format!(
                "Unsupported Gemini tool_choice string: {other}"
            ))),
        },
        Value::Object(choice) => {
            let choice_type = choice.get("type").and_then(Value::as_str).unwrap_or("");
            let config = match choice_type {
                "auto" => json!({ "mode": "AUTO" }),
                "none" => json!({ "mode": "NONE" }),
                "any" => json!({ "mode": "ANY" }),
                "tool" | "function" => {
                    let name = choice
                        .get("name")
                        .or_else(|| {
                            choice
                                .get("function")
                                .and_then(|function| function.get("name"))
                        })
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| {
                            AppError::InvalidInput(
                                "Gemini tool_choice is missing forced tool name".to_string(),
                            )
                        })?;
                    json!({
                        "mode": "ANY",
                        "allowedFunctionNames": [name]
                    })
                }
                other => {
                    return Err(AppError::InvalidInput(format!(
                        "Unsupported Gemini tool_choice type: {other}"
                    )))
                }
            };
            Ok(Some(json!({ "functionCallingConfig": config })))
        }
        _ => Ok(None),
    }
}

fn anthropic_content_to_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => {
            let text = items
                .iter()
                .filter_map(|item| {
                    item.as_str()
                        .map(str::to_string)
                        .or_else(|| item.get("text").and_then(Value::as_str).map(str::to_string))
                        .or_else(|| item.get("content").and_then(anthropic_content_to_text))
                })
                .collect::<Vec<_>>()
                .join("\n");
            (!text.is_empty()).then_some(text)
        }
        _ => None,
    }
}

fn map_anthropic_tools_to_openai_chat(tools: &Value) -> Value {
    let Value::Array(items) = tools else {
        return tools.clone();
    };
    Value::Array(
        items
            .iter()
            .filter(|tool| tool.get("type").and_then(Value::as_str) != Some("BatchTool"))
            .map(|tool| {
                if tool.get("type").and_then(Value::as_str) == Some("function") {
                    return tool.clone();
                }
                let Some(name) = tool.get("name").and_then(Value::as_str) else {
                    return tool.clone();
                };
                let mut function = serde_json::Map::new();
                function.insert("name".to_string(), Value::String(name.to_string()));
                if let Some(description) = tool.get("description").and_then(Value::as_str) {
                    function.insert(
                        "description".to_string(),
                        Value::String(description.to_string()),
                    );
                }
                if let Some(parameters) =
                    tool.get("input_schema").or_else(|| tool.get("parameters"))
                {
                    function.insert("parameters".to_string(), parameters.clone());
                }
                json!({
                    "type": "function",
                    "function": Value::Object(function),
                })
            })
            .collect(),
    )
}

fn map_anthropic_tools_to_openai_responses(tools: &Value) -> Value {
    let Value::Array(items) = tools else {
        return tools.clone();
    };
    Value::Array(
        items
            .iter()
            .filter(|tool| tool.get("type").and_then(Value::as_str) != Some("BatchTool"))
            .map(|tool| {
                if tool.get("type").and_then(Value::as_str) == Some("function")
                    && tool.get("name").is_some()
                {
                    return tool.clone();
                }
                let Some(name) = tool
                    .get("name")
                    .or_else(|| {
                        tool.get("function")
                            .and_then(|function| function.get("name"))
                    })
                    .and_then(Value::as_str)
                else {
                    return tool.clone();
                };
                let mut mapped = serde_json::Map::new();
                mapped.insert("type".to_string(), Value::String("function".to_string()));
                mapped.insert("name".to_string(), Value::String(name.to_string()));
                if let Some(description) = tool
                    .get("description")
                    .or_else(|| {
                        tool.get("function")
                            .and_then(|function| function.get("description"))
                    })
                    .and_then(Value::as_str)
                {
                    mapped.insert(
                        "description".to_string(),
                        Value::String(description.to_string()),
                    );
                }
                if let Some(parameters) = tool
                    .get("input_schema")
                    .or_else(|| tool.get("parameters"))
                    .or_else(|| {
                        tool.get("function")
                            .and_then(|function| function.get("parameters"))
                    })
                {
                    mapped.insert("parameters".to_string(), parameters.clone());
                }
                Value::Object(mapped)
            })
            .collect(),
    )
}

pub fn proxy_gateway_base_url_from_db(db: &Database) -> Result<String, AppError> {
    let config = db.get_proxy_config()?;
    Ok(format!(
        "{}{}",
        proxy_origin_from_parts(&config.host, config.port),
        CLAUDE_DESKTOP_PROXY_PREFIX
    ))
}

fn apply_provider_to_paths(
    db: &Database,
    provider: &Provider,
    paths: &ClaudeDesktopPaths,
) -> Result<(), AppError> {
    if is_official_provider(provider) {
        return restore_official_at_paths(paths);
    }
    validate_provider(provider)?;
    with_rollback(paths, |paths| {
        apply_provider_to_paths_inner(db, provider, paths)
    })
}

fn with_rollback<F>(paths: &ClaudeDesktopPaths, op: F) -> Result<(), AppError>
where
    F: FnOnce(&ClaudeDesktopPaths) -> Result<(), AppError>,
{
    let snapshots = snapshot_files(paths)?;
    match op(paths) {
        Ok(()) => Ok(()),
        Err(err) => match restore_snapshots(&snapshots) {
            Ok(()) => Err(err),
            Err(rollback_err) => Err(AppError::Message(format!(
                "{err}; rollback failed: {rollback_err}"
            ))),
        },
    }
}

fn apply_provider_to_paths_inner(
    db: &Database,
    provider: &Provider,
    paths: &ClaudeDesktopPaths,
) -> Result<(), AppError> {
    let profile = match provider_mode(provider) {
        ClaudeDesktopMode::Direct => {
            let credentials = direct_gateway_credentials(provider)?;
            let model_specs = direct_inference_model_specs(provider)?;
            build_gateway_profile(
                &credentials.base_url,
                &credentials.api_key,
                (!model_specs.is_empty()).then_some(model_specs.as_slice()),
            )
        }
        ClaudeDesktopMode::Proxy => {
            let base_url = proxy_gateway_base_url_from_db(db)?;
            let api_key = get_or_create_gateway_token(db)?;
            let routes = proxy_model_routes(provider)?;
            let model_specs = routes
                .iter()
                .map(|route| InferenceModelSpec {
                    name: route.route_id.clone(),
                    label_override: route.label_override.clone(),
                    supports_1m: route.supports_1m,
                })
                .collect::<Vec<_>>();
            build_gateway_profile(&base_url, &api_key, Some(model_specs.as_slice()))
        }
    };

    write_deployment_mode(&paths.normal_config_path, "3p")?;
    write_deployment_mode(&paths.threep_config_path, "3p")?;
    write_json_file(&paths.profile_path, &profile)?;
    write_meta(&paths.meta_path, Some(PROFILE_ID))?;
    Ok(())
}

fn restore_official_at_paths(paths: &ClaudeDesktopPaths) -> Result<(), AppError> {
    with_rollback(paths, restore_official_at_paths_inner)
}

fn restore_official_at_paths_inner(paths: &ClaudeDesktopPaths) -> Result<(), AppError> {
    write_deployment_mode(&paths.normal_config_path, "1p")?;
    write_deployment_mode(&paths.threep_config_path, "1p")?;
    remove_cc_switch_enterprise_config(&paths.threep_config_path)?;
    delete_file(&paths.profile_path)?;
    write_meta(&paths.meta_path, None)?;
    Ok(())
}

fn build_gateway_profile(
    base_url: &str,
    api_key: &str,
    model_specs: Option<&[InferenceModelSpec]>,
) -> Value {
    let mut profile = json!({
        "disableDeploymentModeChooser": true,
        "inferenceGatewayApiKey": api_key,
        "inferenceGatewayAuthScheme": "bearer",
        "inferenceGatewayBaseUrl": base_url,
        "inferenceProvider": "gateway"
    });

    if let Some(model_specs) = model_specs {
        profile["inferenceModels"] = Value::Array(
            model_specs
                .iter()
                .map(|spec| {
                    if spec.supports_1m || spec.label_override.is_some() {
                        let mut item = json!({ "name": spec.name });
                        if let Some(label_override) = spec.label_override.as_deref() {
                            item["labelOverride"] = json!(label_override);
                        }
                        if spec.supports_1m {
                            item["supports1m"] = json!(true);
                        }
                        item
                    } else {
                        Value::String(spec.name.clone())
                    }
                })
                .collect(),
        );
    }
    profile
}

pub fn gateway_token_from_db(db: &Database) -> Result<Option<String>, AppError> {
    Ok(db
        .get_setting(GATEWAY_TOKEN_SETTING_KEY)?
        .and_then(|token| {
            let trimmed = token.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }))
}

pub fn validate_gateway_bearer_token(
    db: &Database,
    authorization: Option<&str>,
) -> Result<(), AppError> {
    let expected = gateway_token_from_db(db)?.ok_or_else(|| {
        AppError::Unauthorized("Claude Desktop gateway token is not configured".to_string())
    })?;
    let provided = authorization.and_then(parse_bearer_token).ok_or_else(|| {
        AppError::Unauthorized("Missing Claude Desktop gateway bearer token".to_string())
    })?;

    if constant_time_eq::constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
        Ok(())
    } else {
        Err(AppError::Unauthorized(
            "Invalid Claude Desktop gateway bearer token".to_string(),
        ))
    }
}

fn parse_bearer_token(value: &str) -> Option<&str> {
    let mut parts = value.split_whitespace();
    let scheme = parts.next()?;
    let token = parts.next()?;
    if parts.next().is_some() || !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    Some(token)
}

pub fn get_or_create_gateway_token(db: &Database) -> Result<String, AppError> {
    if let Some(token) = db.get_setting(GATEWAY_TOKEN_SETTING_KEY)? {
        let trimmed = token.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    let mut random = [0u8; 32];
    getrandom::getrandom(&mut random)
        .map_err(|e| AppError::Config(format!("Failed to generate gateway token: {e}")))?;
    let token = format!("ccs-{}", URL_SAFE_NO_PAD.encode(random));
    db.set_setting(GATEWAY_TOKEN_SETTING_KEY, &token)?;
    Ok(token)
}

fn read_json_or_empty(path: &Path) -> Result<Value, AppError> {
    let value = if path.exists() {
        read_json_file(path)?
    } else {
        json!({})
    };
    Ok(if value.is_object() { value } else { json!({}) })
}

fn snapshot_files(paths: &ClaudeDesktopPaths) -> Result<Vec<FileSnapshot>, AppError> {
    [
        &paths.normal_config_path,
        &paths.threep_config_path,
        &paths.profile_path,
        &paths.meta_path,
    ]
    .into_iter()
    .map(|path| {
        let content = if path.exists() {
            Some(fs::read(path).map_err(|e| AppError::io(path, e))?)
        } else {
            None
        };
        Ok(FileSnapshot {
            path: path.clone(),
            content,
        })
    })
    .collect()
}

fn restore_snapshots(snapshots: &[FileSnapshot]) -> Result<(), AppError> {
    for snapshot in snapshots {
        match &snapshot.content {
            Some(content) => {
                if let Some(parent) = snapshot.path.parent() {
                    fs::create_dir_all(parent).map_err(|e| AppError::io(parent, e))?;
                }
                atomic_write(&snapshot.path, content)?;
            }
            None => delete_file(&snapshot.path)?,
        }
    }
    Ok(())
}

fn write_deployment_mode(path: &Path, mode: &str) -> Result<(), AppError> {
    let mut value = read_json_or_empty(path)?;
    if let Some(obj) = value.as_object_mut() {
        obj.insert(
            "deploymentMode".to_string(),
            Value::String(mode.to_string()),
        );
    }
    write_json_file(path, &value)
}

fn remove_cc_switch_enterprise_config(path: &Path) -> Result<(), AppError> {
    if !path.exists() {
        return Ok(());
    }
    let mut value = read_json_or_empty(path)?;
    let Some(obj) = value.as_object_mut() else {
        return Ok(());
    };
    let Some(enterprise) = obj
        .get_mut("enterpriseConfig")
        .and_then(Value::as_object_mut)
    else {
        return Ok(());
    };
    for key in [
        "disableDeploymentModeChooser",
        "inferenceGatewayApiKey",
        "inferenceGatewayAuthScheme",
        "inferenceGatewayBaseUrl",
        "inferenceProvider",
    ] {
        enterprise.remove(key);
    }
    if enterprise.is_empty() {
        obj.remove("enterpriseConfig");
    }
    write_json_file(path, &value)
}

fn write_meta(path: &Path, applied_profile_id: Option<&str>) -> Result<(), AppError> {
    let mut value = read_json_or_empty(path)?;
    let obj = value
        .as_object_mut()
        .expect("read_json_or_empty returns object");
    let mut entries = obj
        .get("entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    entries.retain(|entry| entry.get("id").and_then(Value::as_str) != Some(PROFILE_ID));

    match applied_profile_id {
        Some(id) => {
            entries.push(json!({
                "id": PROFILE_ID,
                "name": PROFILE_NAME
            }));
            obj.insert("appliedId".to_string(), Value::String(id.to_string()));
        }
        None => {
            let should_clear_applied = obj
                .get("appliedId")
                .and_then(Value::as_str)
                .is_some_and(|id| id == PROFILE_ID);
            if should_clear_applied {
                if let Some(next_id) = entries
                    .iter()
                    .find_map(|entry| entry.get("id").and_then(Value::as_str))
                {
                    obj.insert("appliedId".to_string(), Value::String(next_id.to_string()));
                } else {
                    obj.remove("appliedId");
                }
            }
        }
    }

    obj.insert("entries".to_string(), Value::Array(entries));
    write_json_file(path, &value)
}

fn read_applied_id(path: &Path) -> Option<String> {
    read_json_or_empty(path).ok().and_then(|value| {
        value
            .get("appliedId")
            .and_then(Value::as_str)
            .map(str::to_string)
    })
}

fn meta_has_profile_entry(path: &Path) -> bool {
    read_json_or_empty(path)
        .ok()
        .and_then(|value| value.get("entries").and_then(Value::as_array).cloned())
        .is_some_and(|entries| {
            entries
                .iter()
                .any(|entry| entry.get("id").and_then(Value::as_str) == Some(PROFILE_ID))
        })
}

fn is_supported_platform() -> bool {
    cfg!(any(target_os = "macos", windows))
}

#[allow(clippy::needless_return)]
fn current_platform_paths() -> Result<ClaudeDesktopPaths, AppError> {
    #[cfg(target_os = "macos")]
    {
        let home =
            get_home_dir().ok_or_else(|| AppError::Config("无法获取用户主目录".to_string()))?;
        return Ok(macos_paths_from_home(&home));
    }

    #[cfg(windows)]
    {
        let local_app_data = windows_local_app_data_dir();
        return Ok(windows_paths_from_local_app_data(&local_app_data));
    }

    #[cfg(not(any(target_os = "macos", windows)))]
    {
        Err(AppError::localized(
            "claude_desktop.unsupported_platform",
            "当前平台暂不支持 Claude Desktop 3P 配置。第一阶段仅支持 macOS 和 Windows。",
            "Claude Desktop 3P configuration is not supported on this platform yet. Phase 1 only supports macOS and Windows.",
        ))
    }
}

#[cfg(target_os = "macos")]
fn macos_paths_from_home(home: &Path) -> ClaudeDesktopPaths {
    let app_support = home.join("Library").join("Application Support");
    paths_from_dirs(app_support.join("Claude"), app_support.join("Claude-3p"))
}

#[cfg(windows)]
fn windows_local_app_data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| get_home_dir().map(|home| home.join("AppData").join("Local")))
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(windows)]
fn windows_paths_from_local_app_data(local_app_data: &Path) -> ClaudeDesktopPaths {
    paths_from_dirs(
        local_app_data.join("Claude"),
        local_app_data.join("Claude-3p"),
    )
}

#[cfg(any(target_os = "macos", windows))]
fn paths_from_dirs(normal_dir: PathBuf, threep_dir: PathBuf) -> ClaudeDesktopPaths {
    let config_library_path = threep_dir.join(CONFIG_LIBRARY_DIR);
    let profile_path = config_library_path.join(format!("{PROFILE_ID}.json"));
    let meta_path = config_library_path.join("_meta.json");
    ClaudeDesktopPaths {
        normal_config_path: normal_dir.join(CONFIG_FILE),
        threep_config_path: threep_dir.join(CONFIG_FILE),
        config_library_path,
        profile_path,
        meta_path,
    }
}

fn proxy_origin_from_parts(listen_address: &str, listen_port: u16) -> String {
    let connect_host = match listen_address {
        "0.0.0.0" => "127.0.0.1",
        "::" => "::1",
        value => value,
    };
    let connect_host_for_url = if connect_host.contains(':') && !connect_host.starts_with('[') {
        format!("[{connect_host}]")
    } else {
        connect_host.to_string()
    };
    format!("http://{}:{}", connect_host_for_url, listen_port)
}

pub(crate) fn suggested_routes_from_claude_provider(
    provider: &Provider,
) -> Option<HashMap<String, crate::provider::ClaudeDesktopModelRoute>> {
    let env = provider
        .settings_config
        .get("env")
        .and_then(Value::as_object)?;
    let mut routes = HashMap::new();
    let supports_1m_default = !provider.meta.as_ref().is_some_and(|meta| {
        matches!(
            meta.provider_type(),
            Some(ProviderType::GithubCopilot | ProviderType::CodexOauth)
        )
    });

    for spec in DEFAULT_PROXY_ROUTES {
        add_suggested_route(
            &mut routes,
            env,
            spec.route_id,
            spec.env_key,
            supports_1m_default,
        );
    }
    if routes.is_empty() {
        add_suggested_route(
            &mut routes,
            env,
            DEFAULT_PROXY_ROUTES[0].route_id,
            "ANTHROPIC_MODEL",
            supports_1m_default,
        );
    }
    (!routes.is_empty()).then_some(routes)
}

fn add_suggested_route(
    routes: &mut HashMap<String, crate::provider::ClaudeDesktopModelRoute>,
    env: &serde_json::Map<String, Value>,
    route_key: &str,
    env_key: &str,
    supports_1m_default: bool,
) {
    let Some(raw_model) = env
        .get(env_key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };

    let marker = ONE_M_CONTEXT_MARKER.as_bytes();
    let raw_bytes = raw_model.as_bytes();
    let has_1m_marker = raw_bytes.len() >= marker.len()
        && raw_bytes[raw_bytes.len() - marker.len()..].eq_ignore_ascii_case(marker);
    let stripped_model = if has_1m_marker {
        raw_model[..raw_model.len() - marker.len()].trim_end()
    } else {
        raw_model
    };
    if stripped_model.is_empty() {
        return;
    }

    let explicit_label_override = env
        .get(&format!("{env_key}_NAME"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let label_override = explicit_label_override
        .clone()
        .or_else(|| (!is_claude_safe_model_id(stripped_model)).then(|| stripped_model.to_string()));
    let effective_supports_1m = supports_1m_default || has_1m_marker;

    let should_overwrite = |existing: Option<&str>| {
        existing.is_none() || explicit_label_override.is_some() || existing == Some(stripped_model)
    };
    let merge_into = |existing: &mut crate::provider::ClaudeDesktopModelRoute| {
        existing.supports_1m = Some(existing.supports_1m.unwrap_or(false) || effective_supports_1m);
        if should_overwrite(existing.label_override.as_deref()) {
            existing.label_override = label_override.clone();
        }
    };

    if let Some(existing) = routes
        .values_mut()
        .find(|existing| existing.model == stripped_model)
    {
        merge_into(existing);
        return;
    }
    routes
        .entry(route_key.to_string())
        .and_modify(merge_into)
        .or_insert_with(|| crate::provider::ClaudeDesktopModelRoute {
            model: stripped_model.to_string(),
            label_override,
            supports_1m: Some(effective_supports_1m),
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{ClaudeDesktopModelRoute, ProviderMeta};

    fn provider_with_meta(settings_config: Value, meta: ProviderMeta) -> Provider {
        Provider {
            id: "provider-1".to_string(),
            name: "Provider 1".to_string(),
            settings_config,
            website_url: None,
            category: None,
            created_at: None,
            sort_index: None,
            notes: None,
            meta: Some(meta),
        }
    }

    fn proxy_provider(routes: HashMap<String, ClaudeDesktopModelRoute>) -> Provider {
        proxy_provider_with_api_format(routes, None)
    }

    fn proxy_provider_with_api_format(
        routes: HashMap<String, ClaudeDesktopModelRoute>,
        api_format: Option<&str>,
    ) -> Provider {
        provider_with_meta(
            json!({
                "env": {
                    "ANTHROPIC_BASE_URL": "https://api.example.com",
                    "ANTHROPIC_AUTH_TOKEN": "sk-test"
                }
            }),
            ProviderMeta {
                claude_desktop_mode: Some(ClaudeDesktopMode::Proxy),
                claude_desktop_model_routes: routes,
                api_format: api_format.map(str::to_string),
                ..ProviderMeta::default()
            },
        )
    }

    #[test]
    fn unsafe_proxy_route_ids_are_repaired_to_desktop_safe_catalog_ids() {
        let provider = proxy_provider(HashMap::from([(
            "qwen3-coder".to_string(),
            ClaudeDesktopModelRoute {
                model: "qwen3-coder".to_string(),
                label_override: None,
                supports_1m: Some(true),
            },
        )]));

        let routes = proxy_model_routes(&provider).expect("proxy routes");

        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].route_id, "claude-sonnet-4-6");
        assert_eq!(routes[0].upstream_model, "qwen3-coder");
        assert_eq!(routes[0].label_override.as_deref(), Some("qwen3-coder"));
        assert!(routes[0].supports_1m);
    }

    #[test]
    fn model_list_response_only_exposes_desktop_safe_route_ids() {
        let provider = proxy_provider(HashMap::from([(
            "ark-code-latest".to_string(),
            ClaudeDesktopModelRoute {
                model: "ark-code-latest".to_string(),
                label_override: Some("火山 Agentplan".to_string()),
                supports_1m: Some(false),
            },
        )]));

        let response = model_list_response(&provider).expect("model list");
        let data = response["data"].as_array().expect("data array");

        assert_eq!(data.len(), 1);
        assert_eq!(data[0]["id"], "claude-sonnet-4-6");
        assert_eq!(response["first_id"], "claude-sonnet-4-6");
        assert_eq!(response["last_id"], "claude-sonnet-4-6");
        assert!(data[0].get("supports1m").is_none());
    }

    #[test]
    fn direct_gateway_credentials_accepts_configured_api_key_field() {
        let provider = provider_with_meta(
            json!({
                "env": {
                    "ANTHROPIC_BASE_URL": "https://api.example.com",
                    "ANTHROPIC_API_KEY": "sk-api-key"
                }
            }),
            ProviderMeta {
                claude_desktop_mode: Some(ClaudeDesktopMode::Direct),
                api_key_field: Some("ANTHROPIC_API_KEY".to_string()),
                ..ProviderMeta::default()
            },
        );

        let credentials = direct_gateway_credentials(&provider).expect("credentials");

        assert_eq!(credentials.base_url, "https://api.example.com");
        assert_eq!(credentials.api_key, "sk-api-key");
    }

    #[test]
    fn provider_status_issues_report_missing_managed_auth_account() {
        let db = Database::memory().expect("memory db");
        let provider = provider_with_meta(
            json!({
                "env": {
                    "ANTHROPIC_BASE_URL": "https://api.githubcopilot.com"
                }
            }),
            ProviderMeta {
                claude_desktop_mode: Some(ClaudeDesktopMode::Proxy),
                provider_type: Some("github_copilot".to_string()),
                auth_binding: Some(crate::provider::ProviderAuthBinding {
                    mode: "managed".to_string(),
                    provider_type: Some("github_copilot".to_string()),
                    account_id: Some("github-missing".to_string()),
                    use_default: Some(false),
                }),
                claude_desktop_model_routes: HashMap::from([(
                    "claude-sonnet-4-6".to_string(),
                    ClaudeDesktopModelRoute {
                        model: "claude-sonnet-4.6".to_string(),
                        label_override: None,
                        supports_1m: Some(true),
                    },
                )]),
                api_format: Some("openai_chat".to_string()),
                ..ProviderMeta::default()
            },
        );

        let issues = provider_status_issues(&db, &provider, true);

        assert!(issues
            .iter()
            .any(|issue| issue.contains("Missing github_copilot managed auth account")));
        assert!(issues.iter().any(|issue| issue.contains("github-missing")));
    }

    #[test]
    fn provider_status_issues_report_logged_out_managed_auth_account() {
        let db = Database::memory().expect("memory db");
        db.upsert_managed_auth_account(crate::auth::ManagedAuthAccountInput {
            provider: crate::auth::ManagedAuthProvider::GithubCopilot,
            id: Some("github-logged-out".to_string()),
            label: "GitHub Logged Out".to_string(),
            username: None,
            avatar_url: None,
            plan: None,
            make_default: true,
            tokens: crate::auth::ManagedAuthTokenSet {
                access_token: "token-before-logout".to_string(),
                refresh_token: None,
                expires_at: None,
                scope: None,
                token_type: Some("Bearer".to_string()),
            },
        })
        .expect("insert account");
        db.logout_managed_auth_account(
            crate::auth::ManagedAuthProvider::GithubCopilot,
            "github-logged-out",
        )
        .expect("logout account");
        let provider = provider_with_meta(
            json!({
                "env": {
                    "ANTHROPIC_BASE_URL": "https://api.githubcopilot.com"
                }
            }),
            ProviderMeta {
                claude_desktop_mode: Some(ClaudeDesktopMode::Proxy),
                provider_type: Some("github_copilot".to_string()),
                auth_binding: Some(crate::provider::ProviderAuthBinding {
                    mode: "managed".to_string(),
                    provider_type: Some("github_copilot".to_string()),
                    account_id: Some("github-logged-out".to_string()),
                    use_default: Some(false),
                }),
                claude_desktop_model_routes: HashMap::from([(
                    "claude-sonnet-4-6".to_string(),
                    ClaudeDesktopModelRoute {
                        model: "claude-sonnet-4.6".to_string(),
                        label_override: None,
                        supports_1m: Some(true),
                    },
                )]),
                api_format: Some("openai_chat".to_string()),
                ..ProviderMeta::default()
            },
        );

        let issues = provider_status_issues(&db, &provider, true);

        assert!(issues
            .iter()
            .any(|issue| issue.contains("Missing github_copilot managed auth account")));
    }

    #[test]
    fn provider_status_issues_report_specific_managed_binding_without_account_id() {
        let db = Database::memory().expect("memory db");
        db.upsert_managed_auth_account(crate::auth::ManagedAuthAccountInput {
            provider: crate::auth::ManagedAuthProvider::GithubCopilot,
            id: Some("github-default".to_string()),
            label: "GitHub Default".to_string(),
            username: None,
            avatar_url: None,
            plan: None,
            make_default: true,
            tokens: crate::auth::ManagedAuthTokenSet {
                access_token: "default-token".to_string(),
                refresh_token: None,
                expires_at: None,
                scope: None,
                token_type: Some("Bearer".to_string()),
            },
        })
        .expect("insert default account");
        let provider = provider_with_meta(
            json!({
                "env": {
                    "ANTHROPIC_BASE_URL": "https://api.githubcopilot.com"
                }
            }),
            ProviderMeta {
                claude_desktop_mode: Some(ClaudeDesktopMode::Proxy),
                provider_type: Some("github_copilot".to_string()),
                auth_binding: Some(crate::provider::ProviderAuthBinding {
                    mode: "managed".to_string(),
                    provider_type: Some("github_copilot".to_string()),
                    account_id: Some("  ".to_string()),
                    use_default: Some(false),
                }),
                claude_desktop_model_routes: HashMap::from([(
                    "claude-sonnet-4-6".to_string(),
                    ClaudeDesktopModelRoute {
                        model: "claude-sonnet-4.6".to_string(),
                        label_override: None,
                        supports_1m: Some(true),
                    },
                )]),
                api_format: Some("openai_chat".to_string()),
                ..ProviderMeta::default()
            },
        );

        let issues = provider_status_issues(&db, &provider, true);

        assert!(issues
            .iter()
            .any(|issue| issue.contains("requires an accountId")));
        assert!(!issues
            .iter()
            .any(|issue| issue.contains("Missing github_copilot managed auth default account")));
    }

    #[test]
    fn provider_status_issues_do_not_require_auth_center_for_manual_mode() {
        let db = Database::memory().expect("memory db");
        let provider = provider_with_meta(
            json!({
                "env": {
                    "ANTHROPIC_BASE_URL": "https://api.githubcopilot.com"
                }
            }),
            ProviderMeta {
                claude_desktop_mode: Some(ClaudeDesktopMode::Proxy),
                provider_type: Some("github_copilot".to_string()),
                auth_binding: Some(crate::provider::ProviderAuthBinding {
                    mode: " api-key ".to_string(),
                    provider_type: Some("github_copilot".to_string()),
                    account_id: None,
                    use_default: None,
                }),
                claude_desktop_model_routes: HashMap::from([(
                    "claude-sonnet-4-6".to_string(),
                    ClaudeDesktopModelRoute {
                        model: "claude-sonnet-4.6".to_string(),
                        label_override: None,
                        supports_1m: Some(true),
                    },
                )]),
                api_format: Some("openai_chat".to_string()),
                ..ProviderMeta::default()
            },
        );

        let issues = provider_status_issues(&db, &provider, true);

        assert!(!issues
            .iter()
            .any(|issue| issue.contains("Missing github_copilot managed auth")));
    }

    #[test]
    fn provider_status_issues_do_not_require_auth_center_for_legacy_manual_key() {
        let db = Database::memory().expect("memory db");
        let provider = provider_with_meta(
            json!({
                "env": {
                    "ANTHROPIC_BASE_URL": "https://api.githubcopilot.com",
                    "ANTHROPIC_AUTH_TOKEN": "manual-token"
                }
            }),
            ProviderMeta {
                claude_desktop_mode: Some(ClaudeDesktopMode::Proxy),
                provider_type: Some("github_copilot".to_string()),
                claude_desktop_model_routes: HashMap::from([(
                    "claude-sonnet-4-6".to_string(),
                    ClaudeDesktopModelRoute {
                        model: "claude-sonnet-4.6".to_string(),
                        label_override: None,
                        supports_1m: Some(true),
                    },
                )]),
                api_format: Some("openai_chat".to_string()),
                ..ProviderMeta::default()
            },
        );

        let issues = provider_status_issues(&db, &provider, true);

        assert!(!issues
            .iter()
            .any(|issue| issue.contains("Missing github_copilot managed auth")));
    }

    #[test]
    fn proxy_manual_oauth_provider_requires_manual_api_key() {
        let provider = provider_with_meta(
            json!({
                "env": {
                    "ANTHROPIC_BASE_URL": "https://api.githubcopilot.com"
                }
            }),
            ProviderMeta {
                claude_desktop_mode: Some(ClaudeDesktopMode::Proxy),
                provider_type: Some("github_copilot".to_string()),
                auth_binding: Some(crate::provider::ProviderAuthBinding {
                    mode: " API_KEY ".to_string(),
                    provider_type: Some("github_copilot".to_string()),
                    account_id: None,
                    use_default: None,
                }),
                claude_desktop_model_routes: HashMap::from([(
                    "claude-sonnet-4-6".to_string(),
                    ClaudeDesktopModelRoute {
                        model: "claude-sonnet-4.6".to_string(),
                        label_override: None,
                        supports_1m: Some(true),
                    },
                )]),
                api_format: Some("openai_chat".to_string()),
                ..ProviderMeta::default()
            },
        );

        let err = validate_proxy_provider(&provider).expect_err("missing manual key");

        assert!(err.to_string().contains("缺少 Base URL 或 API Key"));
    }

    #[test]
    fn proxy_managed_binding_without_required_account_id_does_not_skip_key_check() {
        let provider = provider_with_meta(
            json!({
                "env": {
                    "ANTHROPIC_BASE_URL": "https://api.githubcopilot.com"
                }
            }),
            ProviderMeta {
                claude_desktop_mode: Some(ClaudeDesktopMode::Proxy),
                provider_type: Some("github_copilot".to_string()),
                auth_binding: Some(crate::provider::ProviderAuthBinding {
                    mode: "managed".to_string(),
                    provider_type: Some("github_copilot".to_string()),
                    account_id: Some("   ".to_string()),
                    use_default: Some(false),
                }),
                claude_desktop_model_routes: HashMap::from([(
                    "claude-sonnet-4-6".to_string(),
                    ClaudeDesktopModelRoute {
                        model: "claude-sonnet-4.6".to_string(),
                        label_override: None,
                        supports_1m: Some(true),
                    },
                )]),
                api_format: Some("openai_chat".to_string()),
                ..ProviderMeta::default()
            },
        );

        let err = validate_proxy_provider(&provider).expect_err("invalid binding needs auth");

        assert!(err.to_string().contains("缺少 Base URL 或 API Key"));
    }

    #[test]
    fn proxy_request_model_is_remapped_to_upstream_model() {
        let provider = proxy_provider(HashMap::from([(
            "claude-haiku-4-5".to_string(),
            ClaudeDesktopModelRoute {
                model: "deepseek-v3.1".to_string(),
                label_override: Some("DeepSeek".to_string()),
                supports_1m: Some(false),
            },
        )]));

        let body = map_proxy_request_model(
            json!({
                "model": "claude-haiku-4-5",
                "messages": [{"role": "user", "content": "hi"}]
            }),
            &provider,
        )
        .expect("mapped body");

        assert_eq!(body["model"], "deepseek-v3.1");
        assert_eq!(body["messages"][0]["content"], "hi");
    }

    #[test]
    fn openai_proxy_request_maps_anthropic_tool_choice_variants() {
        let routes = HashMap::from([(
            "claude-haiku-4-5".to_string(),
            ClaudeDesktopModelRoute {
                model: "gpt-4.1".to_string(),
                label_override: None,
                supports_1m: Some(false),
            },
        )]);
        let provider = proxy_provider_with_api_format(routes, Some("openai_chat"));

        let mapped_auto = map_proxy_request_model(
            json!({
                "model": "claude-haiku-4-5",
                "tool_choice": {"type": "auto"}
            }),
            &provider,
        )
        .expect("mapped auto");
        let mapped_any = map_proxy_request_model(
            json!({
                "model": "claude-haiku-4-5",
                "tool_choice": {"type": "any"}
            }),
            &provider,
        )
        .expect("mapped any");
        let mapped_none = map_proxy_request_model(
            json!({
                "model": "claude-haiku-4-5",
                "tool_choice": {"type": "none"}
            }),
            &provider,
        )
        .expect("mapped none");
        let mapped_tool = map_proxy_request_model(
            json!({
                "model": "claude-haiku-4-5",
                "tool_choice": {"type": "tool", "name": "lookup_price"}
            }),
            &provider,
        )
        .expect("mapped forced tool");
        let mapped_function = map_proxy_request_model(
            json!({
                "model": "claude-haiku-4-5",
                "tool_choice": {
                    "type": "function",
                    "function": { "name": "lookup_price" }
                }
            }),
            &provider,
        )
        .expect("mapped forced function");
        let mapped_required_auto = map_proxy_request_model(
            json!({
                "model": "claude-haiku-4-5",
                "tool_choice": "required_auto"
            }),
            &provider,
        )
        .expect("mapped required_auto");

        assert_eq!(mapped_auto["tool_choice"], "auto");
        assert_eq!(mapped_any["tool_choice"], "required");
        assert_eq!(mapped_none["tool_choice"], "none");
        assert_eq!(mapped_required_auto["tool_choice"], "required");
        assert_eq!(
            mapped_tool["tool_choice"],
            json!({
                "type": "function",
                "function": {
                    "name": "lookup_price"
                }
            })
        );
        assert_eq!(
            mapped_function["tool_choice"],
            json!({
                "type": "function",
                "function": {
                    "name": "lookup_price"
                }
            })
        );
    }

    #[test]
    fn openai_chat_proxy_request_maps_messages_tools_and_system_prompt() {
        let provider = proxy_provider_with_api_format(
            HashMap::from([(
                "claude-sonnet-4-6".to_string(),
                ClaudeDesktopModelRoute {
                    model: "gpt-4.1".to_string(),
                    label_override: None,
                    supports_1m: Some(false),
                },
            )]),
            Some("openai_chat"),
        );

        let mapped = map_proxy_request_model(
            json!({
                "model": "claude-sonnet-4-6",
                "system": "You are concise.",
                "stop_sequences": ["END"],
                "messages": [
                    {
                        "role": "user",
                        "content": [
                            { "type": "text", "text": "hello" }
                        ]
                    }
                ],
                "tools": [
                    {
                        "name": "lookup_price",
                        "description": "Look up a price",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "symbol": { "type": "string" }
                            }
                        }
                    }
                ]
            }),
            &provider,
        )
        .expect("mapped body");

        assert_eq!(mapped["model"], "gpt-4.1");
        assert_eq!(mapped["messages"][0]["role"], "system");
        assert_eq!(mapped["messages"][0]["content"], "You are concise.");
        assert_eq!(mapped["messages"][1]["content"], "hello");
        assert_eq!(mapped["stop"], json!(["END"]));
        assert!(mapped.get("stop_sequences").is_none());
        assert_eq!(mapped["tools"][0]["type"], "function");
        assert_eq!(mapped["tools"][0]["function"]["name"], "lookup_price");
    }

    #[test]
    fn openai_chat_proxy_request_applies_model_parameter_rules() {
        let provider = proxy_provider_with_api_format(
            HashMap::from([(
                "claude-sonnet-4-6".to_string(),
                ClaudeDesktopModelRoute {
                    model: "o3-mini".to_string(),
                    label_override: None,
                    supports_1m: Some(false),
                },
            )]),
            Some("openai_chat"),
        );

        let mapped = map_proxy_request_model(
            json!({
                "model": "claude-sonnet-4-6",
                "system": "x-anthropic-billing-header: cch=rotating\n\nUse short answers.",
                "messages": [{ "role": "user", "content": "hello" }],
                "max_tokens": 2048,
                "thinking": { "type": "enabled", "budget_tokens": 12000 },
                "output_config": { "effort": "max" },
                "tools": [
                    {
                        "type": "BatchTool",
                        "name": "batch",
                        "input_schema": { "type": "object" }
                    },
                    {
                        "name": "lookup_price",
                        "input_schema": { "type": "object" }
                    }
                ]
            }),
            &provider,
        )
        .expect("mapped body");

        assert_eq!(mapped["messages"][0]["content"], "Use short answers.");
        assert_eq!(mapped["max_completion_tokens"], 2048);
        assert!(mapped.get("max_tokens").is_none());
        assert_eq!(mapped["reasoning_effort"], "xhigh");
        assert!(mapped.get("thinking").is_none());
        assert!(mapped.get("output_config").is_none());
        assert_eq!(mapped["tools"].as_array().expect("tools").len(), 1);
        assert_eq!(mapped["tools"][0]["function"]["name"], "lookup_price");
    }

    #[test]
    fn openai_responses_proxy_request_maps_input_tools_max_tokens_and_tool_choice() {
        let provider = proxy_provider_with_api_format(
            HashMap::from([(
                "claude-sonnet-4-6".to_string(),
                ClaudeDesktopModelRoute {
                    model: "gpt-5.1-codex".to_string(),
                    label_override: None,
                    supports_1m: Some(false),
                },
            )]),
            Some("openai_responses"),
        );

        let mapped = map_proxy_request_model(
            json!({
                "model": "claude-sonnet-4-6",
                "system": [{ "type": "text", "text": "Use short answers." }],
                "messages": [
                    {
                        "role": "user",
                        "content": [
                            { "type": "text", "text": "hello" }
                        ]
                    }
                ],
                "max_tokens": 2048,
                "stop_sequences": ["END"],
                "tool_choice": { "type": "tool", "name": "lookup_price" },
                "tools": [
                    {
                        "name": "lookup_price",
                        "input_schema": { "type": "object" }
                    }
                ]
            }),
            &provider,
        )
        .expect("mapped body");

        assert_eq!(mapped["model"], "gpt-5.1-codex");
        assert_eq!(mapped["instructions"], "Use short answers.");
        assert!(mapped.get("messages").is_none());
        assert_eq!(
            mapped["input"][0]["content"],
            json!([{ "type": "input_text", "text": "hello" }])
        );
        assert_eq!(mapped["max_output_tokens"], 2048);
        assert!(mapped.get("max_tokens").is_none());
        assert_eq!(mapped["stop"], json!(["END"]));
        assert!(mapped.get("stop_sequences").is_none());
        assert_eq!(
            mapped["tool_choice"],
            json!({ "type": "function", "name": "lookup_price" })
        );
        assert_eq!(mapped["tools"][0]["type"], "function");
        assert_eq!(mapped["tools"][0]["name"], "lookup_price");
    }

    #[test]
    fn openai_responses_proxy_request_maps_reasoning_and_filters_batch_tool() {
        let provider = proxy_provider_with_api_format(
            HashMap::from([(
                "claude-sonnet-4-6".to_string(),
                ClaudeDesktopModelRoute {
                    model: "gpt-5.1".to_string(),
                    label_override: None,
                    supports_1m: Some(false),
                },
            )]),
            Some("openai_responses"),
        );

        let mapped = map_proxy_request_model(
            json!({
                "model": "claude-sonnet-4-6",
                "messages": [{ "role": "user", "content": "hello" }],
                "thinking": { "type": "enabled", "budget_tokens": 2000 },
                "tools": [
                    {
                        "type": "BatchTool",
                        "name": "batch",
                        "input_schema": { "type": "object" }
                    },
                    {
                        "name": "lookup_price",
                        "input_schema": { "type": "object" }
                    }
                ]
            }),
            &provider,
        )
        .expect("mapped body");

        assert_eq!(mapped["reasoning"], json!({ "effort": "low" }));
        assert!(mapped.get("thinking").is_none());
        assert_eq!(mapped["tools"].as_array().expect("tools").len(), 1);
        assert_eq!(mapped["tools"][0]["name"], "lookup_price");
    }

    #[test]
    fn openai_responses_codex_oauth_applies_upstream_protocol_constraints() {
        let mut provider = proxy_provider_with_api_format(
            HashMap::from([(
                "claude-sonnet-4-6".to_string(),
                ClaudeDesktopModelRoute {
                    model: "gpt-5.1-codex".to_string(),
                    label_override: None,
                    supports_1m: Some(false),
                },
            )]),
            Some("openai_responses"),
        );
        let meta = provider.meta.as_mut().expect("provider meta");
        meta.provider_type = Some("codex_oauth".to_string());
        meta.prompt_cache_key = Some("cache-key".to_string());
        meta.codex_fast_mode = Some(true);

        let mapped = map_proxy_request_model(
            json!({
                "model": "claude-sonnet-4-6",
                "messages": [{"role": "user", "content": "hello"}],
                "max_tokens": 2048,
                "temperature": 0.7,
                "top_p": 0.9,
                "stop_sequences": ["END"],
                "stream": false,
                "include": ["something.else", "reasoning.encrypted_content"],
                "thinking": {"type": "enabled", "budget_tokens": 1024}
            }),
            &provider,
        )
        .expect("mapped body");

        assert_eq!(mapped["store"], json!(false));
        assert_eq!(mapped["service_tier"], json!("priority"));
        assert_eq!(mapped["prompt_cache_key"], json!("cache-key"));
        assert_eq!(mapped["instructions"], json!(""));
        assert_eq!(mapped["tools"], json!([]));
        assert_eq!(mapped["parallel_tool_calls"], json!(false));
        assert_eq!(mapped["stream"], json!(true));
        assert_eq!(mapped["include"][0], json!("something.else"));
        assert_eq!(mapped["include"][1], json!("reasoning.encrypted_content"));
        assert_eq!(
            mapped["include"]
                .as_array()
                .expect("include array")
                .iter()
                .filter(|value| value.as_str() == Some("reasoning.encrypted_content"))
                .count(),
            1
        );
        assert!(mapped.get("max_output_tokens").is_none());
        assert!(mapped.get("max_tokens").is_none());
        assert!(mapped.get("temperature").is_none());
        assert!(mapped.get("top_p").is_none());
        assert!(mapped.get("stop").is_none());
        assert!(mapped.get("stop_sequences").is_none());
        assert!(mapped.get("thinking").is_none());
    }

    #[test]
    fn openai_responses_non_codex_keeps_general_responses_parameters() {
        let provider = proxy_provider_with_api_format(
            HashMap::from([(
                "claude-sonnet-4-6".to_string(),
                ClaudeDesktopModelRoute {
                    model: "gpt-5.1".to_string(),
                    label_override: None,
                    supports_1m: Some(false),
                },
            )]),
            Some("openai_responses"),
        );

        let mapped = map_proxy_request_model(
            json!({
                "model": "claude-sonnet-4-6",
                "messages": [{"role": "user", "content": "hello"}],
                "max_tokens": 2048,
                "temperature": 0.7,
                "top_p": 0.9,
                "stop_sequences": ["END"],
                "stream": false
            }),
            &provider,
        )
        .expect("mapped body");

        assert_eq!(mapped["max_output_tokens"], json!(2048));
        assert_eq!(mapped["temperature"], json!(0.7));
        assert_eq!(mapped["top_p"], json!(0.9));
        assert_eq!(mapped["stop"], json!(["END"]));
        assert_eq!(mapped["stream"], json!(false));
        assert!(mapped.get("store").is_none());
        assert!(mapped.get("include").is_none());
        assert!(mapped.get("service_tier").is_none());
        assert!(mapped.get("parallel_tool_calls").is_none());
        assert!(mapped.get("instructions").is_none());
        assert!(mapped.get("tools").is_none());
    }

    #[test]
    fn gemini_native_proxy_request_maps_anthropic_body_to_generate_content() {
        let provider = proxy_provider_with_api_format(
            HashMap::from([(
                "claude-sonnet-4-6".to_string(),
                ClaudeDesktopModelRoute {
                    model: "gemini-2.5-pro".to_string(),
                    label_override: None,
                    supports_1m: Some(false),
                },
            )]),
            Some("gemini_native"),
        );

        let mapped = map_proxy_request_model(
            json!({
                "model": "claude-sonnet-4-6",
                "system": [{ "type": "text", "text": "Use short answers." }],
                "messages": [
                    {
                        "role": "user",
                        "content": [
                            { "type": "text", "text": "hello" },
                            {
                                "type": "image",
                                "source": {
                                    "type": "base64",
                                    "media_type": "image/png",
                                    "data": "aGVsbG8="
                                }
                            }
                        ]
                    }
                ],
                "max_tokens": 1024,
                "temperature": 0.3,
                "top_p": 0.8,
                "stop_sequences": ["END"],
                "tool_choice": { "type": "tool", "name": "lookup_price" },
                "tools": [
                    {
                        "name": "lookup_price",
                        "description": "Look up a price",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "symbol": { "type": "string" }
                            }
                        }
                    }
                ]
            }),
            &provider,
        )
        .expect("mapped body");

        assert!(mapped.get("model").is_none());
        assert_eq!(
            mapped["systemInstruction"]["parts"][0]["text"],
            "Use short answers."
        );
        assert_eq!(mapped["contents"][0]["role"], "user");
        assert_eq!(mapped["contents"][0]["parts"][0]["text"], "hello");
        assert_eq!(
            mapped["contents"][0]["parts"][1]["inlineData"],
            json!({
                "mimeType": "image/png",
                "data": "aGVsbG8="
            })
        );
        assert_eq!(mapped["generationConfig"]["maxOutputTokens"], 1024);
        assert_eq!(mapped["generationConfig"]["temperature"], 0.3);
        assert_eq!(mapped["generationConfig"]["topP"], 0.8);
        assert_eq!(mapped["generationConfig"]["stopSequences"], json!(["END"]));
        assert_eq!(
            mapped["tools"][0]["functionDeclarations"][0]["name"],
            "lookup_price"
        );
        assert_eq!(
            mapped["toolConfig"]["functionCallingConfig"],
            json!({
                "mode": "ANY",
                "allowedFunctionNames": ["lookup_price"]
            })
        );
    }

    #[test]
    fn gemini_native_tool_schema_uses_json_schema_channel_when_needed() {
        let provider = proxy_provider_with_api_format(
            HashMap::from([(
                "claude-sonnet-4-6".to_string(),
                ClaudeDesktopModelRoute {
                    model: "gemini-2.5-pro".to_string(),
                    label_override: None,
                    supports_1m: Some(false),
                },
            )]),
            Some("gemini_native"),
        );

        let mapped = map_proxy_request_model(
            json!({
                "model": "claude-sonnet-4-6",
                "messages": [{ "role": "user", "content": "hello" }],
                "tools": [
                    {
                        "name": "weather",
                        "description": "Weather lookup",
                        "input_schema": {
                            "$schema": "https://json-schema.org/draft/2020-12/schema",
                            "type": "object",
                            "properties": {
                                "city": { "type": "string" }
                            },
                            "required": ["city"],
                            "additionalProperties": false
                        }
                    },
                    {
                        "name": "ping",
                        "description": "No argument tool"
                    }
                ]
            }),
            &provider,
        )
        .expect("mapped body");

        let declarations = mapped["tools"][0]["functionDeclarations"]
            .as_array()
            .expect("function declarations");
        let weather = declarations
            .iter()
            .find(|declaration| declaration["name"] == "weather")
            .expect("weather declaration");
        assert!(weather.get("parameters").is_none());
        assert!(weather.get("parametersJsonSchema").is_some());
        assert!(weather["parametersJsonSchema"].get("$schema").is_none());
        assert_eq!(
            weather["parametersJsonSchema"]["additionalProperties"],
            false
        );

        let ping = declarations
            .iter()
            .find(|declaration| declaration["name"] == "ping")
            .expect("ping declaration");
        assert_eq!(ping["parameters"]["type"], "object");
        assert!(ping["parameters"]["properties"].is_object());
    }

    #[test]
    fn gemini_native_response_maps_to_anthropic_message() {
        let mapped = map_gemini_native_response_to_anthropic(json!({
            "responseId": "resp-1",
            "modelVersion": "gemini-2.5-pro",
            "candidates": [{
                "finishReason": "STOP",
                "content": {
                    "parts": [
                        { "text": "Checking." },
                        {
                            "functionCall": {
                                "name": "lookup_price",
                                "args": { "symbol": "AAPL" }
                            }
                        }
                    ]
                }
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "totalTokenCount": 15,
                "cachedContentTokenCount": 3
            }
        }))
        .expect("mapped response");

        assert_eq!(mapped["id"], "resp-1");
        assert_eq!(mapped["type"], "message");
        assert_eq!(mapped["role"], "assistant");
        assert_eq!(mapped["model"], "gemini-2.5-pro");
        assert_eq!(mapped["stop_reason"], "tool_use");
        assert_eq!(
            mapped["content"][0],
            json!({ "type": "text", "text": "Checking." })
        );
        assert_eq!(mapped["content"][1]["type"], "tool_use");
        assert_eq!(mapped["content"][1]["id"], "gemini_synth_1");
        assert_eq!(mapped["content"][1]["name"], "lookup_price");
        assert_eq!(mapped["content"][1]["input"], json!({ "symbol": "AAPL" }));
        assert_eq!(mapped["usage"]["input_tokens"], 10);
        assert_eq!(mapped["usage"]["output_tokens"], 5);
        assert_eq!(mapped["usage"]["cache_read_input_tokens"], 3);
    }

    #[test]
    fn gemini_native_response_rectifies_tool_args_using_request_schema_hints() {
        let request_body = json!({
            "tools": [
                {
                    "name": "install_skill",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "skill": { "type": "string" }
                        },
                        "required": ["skill"]
                    }
                },
                {
                    "name": "read_file",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string" },
                            "limit": { "type": "integer" }
                        },
                        "required": ["path"]
                    }
                }
            ]
        });
        let hints = crate::proxy::gemini_schema::extract_anthropic_tool_schema_hints(&request_body);

        let mapped = map_gemini_native_response_to_anthropic_with_shadow_and_hints(
            json!({
                "responseId": "resp-rectify",
                "modelVersion": "gemini-2.5-pro",
                "candidates": [{
                    "finishReason": "STOP",
                    "content": {
                        "parts": [
                            {
                                "functionCall": {
                                    "name": "install_skill",
                                    "args": { "name": "python-tools" }
                                }
                            },
                            {
                                "functionCall": {
                                    "name": "read_file",
                                    "args": {
                                        "parameters": {
                                            "path": ["README.md"],
                                            "limit": 20
                                        }
                                    }
                                }
                            }
                        ]
                    }
                }]
            }),
            None,
            None,
            None,
            Some(&hints),
        )
        .expect("mapped response");

        assert_eq!(
            mapped["content"][0]["input"],
            json!({ "skill": "python-tools" })
        );
        assert_eq!(
            mapped["content"][1]["input"],
            json!({ "path": "README.md", "limit": 20 })
        );
    }

    #[test]
    fn openai_chat_response_maps_to_anthropic_message() {
        let mapped = map_openai_chat_response_to_anthropic(json!({
            "id": "chatcmpl-1",
            "model": "gpt-4.1",
            "choices": [{
                "finish_reason": "stop",
                "message": {
                    "role": "assistant",
                    "content": [
                        { "type": "text", "text": "Hello." },
                        { "type": "refusal", "refusal": "" }
                    ],
                    "refusal": "Cannot do that."
                }
            }],
            "usage": {
                "prompt_tokens": 12,
                "completion_tokens": 5
            }
        }))
        .expect("mapped response");

        assert_eq!(mapped["id"], "chatcmpl-1");
        assert_eq!(mapped["type"], "message");
        assert_eq!(mapped["role"], "assistant");
        assert_eq!(mapped["model"], "gpt-4.1");
        assert_eq!(mapped["stop_reason"], "end_turn");
        assert_eq!(
            mapped["content"],
            json!([
                { "type": "text", "text": "Hello." },
                { "type": "text", "text": "Cannot do that." }
            ])
        );
        assert_eq!(mapped["usage"]["input_tokens"], 12);
        assert_eq!(mapped["usage"]["output_tokens"], 5);
    }

    #[test]
    fn openai_chat_response_maps_tool_calls_and_cache_usage() {
        let mapped = map_openai_chat_response_to_anthropic(json!({
            "id": "chatcmpl-2",
            "model": "gpt-4.1",
            "choices": [{
                "finish_reason": "tool_calls",
                "message": {
                    "role": "assistant",
                    "content": "Checking.",
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "lookup_price",
                            "arguments": "{\"symbol\":\"AAPL\"}"
                        }
                    }]
                }
            }],
            "usage": {
                "prompt_tokens": 30,
                "completion_tokens": 7,
                "prompt_tokens_details": { "cached_tokens": 11 },
                "cache_creation_input_tokens": 3
            }
        }))
        .expect("mapped response");

        assert_eq!(mapped["stop_reason"], "tool_use");
        assert_eq!(
            mapped["content"][0],
            json!({ "type": "text", "text": "Checking." })
        );
        assert_eq!(
            mapped["content"][1],
            json!({
                "type": "tool_use",
                "id": "call_1",
                "name": "lookup_price",
                "input": { "symbol": "AAPL" }
            })
        );
        assert_eq!(mapped["usage"]["input_tokens"], 30);
        assert_eq!(mapped["usage"]["output_tokens"], 7);
        assert_eq!(mapped["usage"]["cache_read_input_tokens"], 11);
        assert_eq!(mapped["usage"]["cache_creation_input_tokens"], 3);
    }

    #[test]
    fn openai_responses_response_maps_to_anthropic_message() {
        let mapped = map_openai_responses_response_to_anthropic(json!({
            "id": "resp-1",
            "model": "gpt-5.1-codex",
            "status": "completed",
            "output": [{
                "type": "message",
                "content": [
                    { "type": "output_text", "text": "Done." },
                    { "type": "refusal", "refusal": "No." }
                ]
            }],
            "usage": {
                "input_tokens": 20,
                "output_tokens": 4
            }
        }))
        .expect("mapped response");

        assert_eq!(mapped["id"], "resp-1");
        assert_eq!(mapped["model"], "gpt-5.1-codex");
        assert_eq!(mapped["stop_reason"], "end_turn");
        assert_eq!(
            mapped["content"],
            json!([
                { "type": "text", "text": "Done." },
                { "type": "text", "text": "No." }
            ])
        );
        assert_eq!(mapped["usage"]["input_tokens"], 20);
        assert_eq!(mapped["usage"]["output_tokens"], 4);
    }

    #[test]
    fn openai_responses_response_maps_function_call_and_cache_usage() {
        let mapped = map_openai_responses_response_to_anthropic(json!({
            "id": "resp-2",
            "model": "gpt-5.1-codex",
            "status": "completed",
            "output": [
                {
                    "type": "reasoning",
                    "summary": [{ "type": "summary_text", "text": "Need a lookup." }]
                },
                {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "Read",
                    "arguments": "{\"file_path\":\"README.md\",\"pages\":\"\"}"
                }
            ],
            "usage": {
                "input_tokens": 44,
                "output_tokens": 9,
                "input_tokens_details": { "cached_tokens": 17 },
                "cache_creation_input_tokens": 2
            }
        }))
        .expect("mapped response");

        assert_eq!(mapped["stop_reason"], "tool_use");
        assert_eq!(
            mapped["content"][0],
            json!({ "type": "thinking", "thinking": "Need a lookup." })
        );
        assert_eq!(
            mapped["content"][1],
            json!({
                "type": "tool_use",
                "id": "call_1",
                "name": "Read",
                "input": { "file_path": "README.md" }
            })
        );
        assert_eq!(mapped["usage"]["input_tokens"], 44);
        assert_eq!(mapped["usage"]["output_tokens"], 9);
        assert_eq!(mapped["usage"]["cache_read_input_tokens"], 17);
        assert_eq!(mapped["usage"]["cache_creation_input_tokens"], 2);
    }

    #[test]
    fn gemini_native_request_does_not_replay_synthesized_tool_ids_upstream() {
        let provider = proxy_provider_with_api_format(
            HashMap::from([(
                "claude-sonnet-4-6".to_string(),
                ClaudeDesktopModelRoute {
                    model: "gemini-2.5-pro".to_string(),
                    label_override: None,
                    supports_1m: Some(false),
                },
            )]),
            Some("gemini_native"),
        );

        let mapped = map_proxy_request_model(
            json!({
                "model": "claude-sonnet-4-6",
                "messages": [
                    {
                        "role": "assistant",
                        "content": [{
                            "type": "tool_use",
                            "id": "gemini_synth_1",
                            "name": "lookup_price",
                            "input": { "symbol": "AAPL" }
                        }]
                    },
                    {
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": "gemini_synth_1",
                            "content": "190"
                        }]
                    }
                ]
            }),
            &provider,
        )
        .expect("mapped body");

        let function_call = &mapped["contents"][0]["parts"][0]["functionCall"];
        let function_response = &mapped["contents"][1]["parts"][0]["functionResponse"];

        assert_eq!(function_call["name"], "lookup_price");
        assert!(function_call.get("id").is_none());
        assert_eq!(function_response["name"], "lookup_price");
        assert_eq!(function_response["response"], json!({ "content": "190" }));
        assert!(function_response.get("id").is_none());
    }

    #[test]
    fn gemini_native_shadow_replays_thought_signature_parts() {
        let provider = proxy_provider_with_api_format(
            HashMap::from([(
                "claude-sonnet-4-6".to_string(),
                ClaudeDesktopModelRoute {
                    model: "gemini-2.5-pro".to_string(),
                    label_override: None,
                    supports_1m: Some(false),
                },
            )]),
            Some("gemini_native"),
        );
        let shadow = GeminiShadowStore::with_limits(8, 8);

        let mapped_response = map_gemini_native_response_to_anthropic_with_shadow(
            json!({
                "responseId": "gemini-resp-1",
                "modelVersion": "gemini-2.5-pro",
                "candidates": [{
                    "finishReason": "STOP",
                    "content": {
                        "parts": [{
                            "thoughtSignature": "sig-tool",
                            "functionCall": {
                                "name": "lookup_price",
                                "args": { "symbol": "AAPL" }
                            }
                        }]
                    }
                }],
                "usageMetadata": { "promptTokenCount": 10, "totalTokenCount": 12 }
            }),
            Some(&shadow),
            Some(&provider.id),
            Some("session-1"),
        )
        .expect("mapped response");

        assert_eq!(mapped_response["content"][0]["id"], json!("gemini_synth_0"));

        let mapped_request = map_proxy_request_model_with_gemini_shadow(
            json!({
                "model": "claude-sonnet-4-6",
                "session_id": "session-1",
                "messages": [
                    {
                        "role": "assistant",
                        "content": [{
                            "type": "tool_use",
                            "id": "gemini_synth_0",
                            "name": "lookup_price",
                            "input": { "symbol": "AAPL" }
                        }]
                    },
                    {
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": "gemini_synth_0",
                            "content": "190"
                        }]
                    }
                ]
            }),
            &provider,
            &shadow,
            Some("session-1"),
        )
        .expect("mapped request");

        let replayed_part = &mapped_request["contents"][0]["parts"][0];
        assert_eq!(replayed_part["thoughtSignature"], json!("sig-tool"));
        assert_eq!(replayed_part["functionCall"]["name"], json!("lookup_price"));
        assert_eq!(
            replayed_part["functionCall"]["args"],
            json!({ "symbol": "AAPL" })
        );
        assert!(replayed_part["functionCall"].get("id").is_none());
        assert!(
            mapped_request["contents"][1]["parts"][0]["functionResponse"]
                .get("id")
                .is_none()
        );
    }

    #[test]
    fn openai_chat_proxy_request_maps_tool_blocks_with_canonical_arguments() {
        let provider = proxy_provider_with_api_format(
            HashMap::from([(
                "claude-sonnet-4-6".to_string(),
                ClaudeDesktopModelRoute {
                    model: "gpt-4.1".to_string(),
                    label_override: None,
                    supports_1m: Some(false),
                },
            )]),
            Some("openai_chat"),
        );

        let mapped = map_proxy_request_model(
            json!({
                "model": "claude-sonnet-4-6",
                "messages": [
                    {
                        "role": "assistant",
                        "content": [
                            { "type": "text", "text": "Checking." },
                            {
                                "type": "tool_use",
                                "id": "toolu_1",
                                "name": "lookup_price",
                                "input": { "symbol": "AAPL", "market": { "country": "US", "code": "NASDAQ" } }
                            }
                        ]
                    },
                    {
                        "role": "user",
                        "content": [
                            {
                                "type": "tool_result",
                                "tool_use_id": "toolu_1",
                                "content": { "price": 201, "currency": "USD" }
                            }
                        ]
                    }
                ]
            }),
            &provider,
        )
        .expect("mapped body");

        assert_eq!(mapped["messages"][0]["content"], "Checking.");
        assert_eq!(mapped["messages"][0]["tool_calls"][0]["id"], "toolu_1");
        assert_eq!(
            mapped["messages"][0]["tool_calls"][0]["function"]["arguments"],
            r#"{"market":{"code":"NASDAQ","country":"US"},"symbol":"AAPL"}"#
        );
        assert_eq!(mapped["messages"][1]["role"], "tool");
        assert_eq!(mapped["messages"][1]["tool_call_id"], "toolu_1");
        assert_eq!(
            mapped["messages"][1]["content"],
            r#"{"currency":"USD","price":201}"#
        );
    }

    #[test]
    fn github_copilot_proxy_request_prepares_body_before_openai_mapping() {
        let mut provider = proxy_provider_with_api_format(
            HashMap::from([(
                "claude-sonnet-4-6".to_string(),
                ClaudeDesktopModelRoute {
                    model: "gpt-4.1".to_string(),
                    label_override: None,
                    supports_1m: Some(false),
                },
            )]),
            Some("openai_chat"),
        );
        provider.meta.as_mut().expect("meta").provider_type = Some("github_copilot".to_string());

        let mapped = map_proxy_request_model(
            json!({
                "model": "claude-sonnet-4-6",
                "messages": [
                    {
                        "role": "assistant",
                        "content": [
                            { "type": "thinking", "thinking": "hidden" },
                            {
                                "type": "tool_use",
                                "id": "toolu_1",
                                "name": "Read",
                                "input": { "file_path": "README.md" }
                            }
                        ]
                    },
                    {
                        "role": "user",
                        "content": [
                            {
                                "type": "tool_result",
                                "tool_use_id": "toolu_1",
                                "content": "contents"
                            },
                            { "type": "text", "text": "continue" }
                        ]
                    }
                ]
            }),
            &provider,
        )
        .expect("mapped body");

        assert!(mapped["messages"][0].get("reasoning_content").is_none());
        assert_eq!(mapped["messages"][0]["tool_calls"][0]["id"], "toolu_1");
        assert_eq!(mapped["messages"][1]["role"], "tool");
        assert_eq!(mapped["messages"][1]["tool_call_id"], "toolu_1");
        assert_eq!(mapped["messages"][1]["content"], "contents\ncontinue");
    }

    #[test]
    fn openai_responses_proxy_request_maps_tool_blocks_with_canonical_arguments() {
        let provider = proxy_provider_with_api_format(
            HashMap::from([(
                "claude-sonnet-4-6".to_string(),
                ClaudeDesktopModelRoute {
                    model: "gpt-5.1-codex".to_string(),
                    label_override: None,
                    supports_1m: Some(false),
                },
            )]),
            Some("openai_responses"),
        );

        let mapped = map_proxy_request_model(
            json!({
                "model": "claude-sonnet-4-6",
                "messages": [
                    {
                        "role": "assistant",
                        "content": [
                            { "type": "text", "text": "Checking." },
                            {
                                "type": "tool_use",
                                "id": "toolu_1",
                                "name": "lookup_price",
                                "input": { "symbol": "AAPL", "market": { "country": "US", "code": "NASDAQ" } }
                            }
                        ]
                    },
                    {
                        "role": "user",
                        "content": [
                            {
                                "type": "tool_result",
                                "tool_use_id": "toolu_1",
                                "content": { "price": 201, "currency": "USD" }
                            }
                        ]
                    }
                ]
            }),
            &provider,
        )
        .expect("mapped body");

        assert_eq!(
            mapped["input"][0]["content"],
            json!([{ "type": "output_text", "text": "Checking." }])
        );
        assert_eq!(mapped["input"][1]["type"], "function_call");
        assert_eq!(mapped["input"][1]["call_id"], "toolu_1");
        assert_eq!(
            mapped["input"][1]["arguments"],
            r#"{"market":{"code":"NASDAQ","country":"US"},"symbol":"AAPL"}"#
        );
        assert_eq!(mapped["input"][2]["type"], "function_call_output");
        assert_eq!(mapped["input"][2]["call_id"], "toolu_1");
        assert_eq!(
            mapped["input"][2]["output"],
            r#"{"currency":"USD","price":201}"#
        );
    }

    #[test]
    fn anthropic_proxy_request_preserves_tool_choice_shape() {
        let provider = proxy_provider(HashMap::from([(
            "claude-haiku-4-5".to_string(),
            ClaudeDesktopModelRoute {
                model: "claude-3-5-haiku-latest".to_string(),
                label_override: None,
                supports_1m: Some(false),
            },
        )]));

        let body = map_proxy_request_model(
            json!({
                "model": "claude-haiku-4-5",
                "tool_choice": {"type": "tool", "name": "lookup_price"}
            }),
            &provider,
        )
        .expect("mapped body");

        assert_eq!(
            body["tool_choice"],
            json!({"type": "tool", "name": "lookup_price"})
        );
    }

    #[test]
    fn suggested_routes_strip_one_m_marker_and_preserve_explicit_labels() {
        let provider = provider_with_meta(
            json!({
                "env": {
                    "ANTHROPIC_DEFAULT_SONNET_MODEL": "qianfan-code-latest [1m]",
                    "ANTHROPIC_DEFAULT_SONNET_MODEL_NAME": "Qianfan Coding"
                }
            }),
            ProviderMeta::default(),
        );

        let routes = suggested_routes_from_claude_provider(&provider).expect("suggested routes");
        let route = routes
            .get("claude-sonnet-4-6")
            .expect("sonnet route should be present");

        assert_eq!(route.model, "qianfan-code-latest");
        assert_eq!(route.label_override.as_deref(), Some("Qianfan Coding"));
        assert_eq!(route.supports_1m, Some(true));
    }
}
