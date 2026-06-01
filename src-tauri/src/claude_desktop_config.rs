use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(any(target_os = "macos", windows))]
use crate::config::get_home_dir;
use crate::config::{atomic_write, delete_file, read_json_file, write_json_file};
use crate::database::{Database, CLAUDE_DESKTOP_OFFICIAL_PROVIDER_ID};
use crate::error::AppError;
use crate::provider::{ClaudeDesktopMode, Provider};

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
    })
}

pub fn default_proxy_routes() -> Vec<ClaudeDesktopDefaultRoute> {
    DEFAULT_PROXY_ROUTES.to_vec()
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
    let api_key = required_env(env, "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_AUTH_TOKEN")?;
    Ok(DirectGatewayCredentials { base_url, api_key })
}

fn required_env(
    env: &serde_json::Map<String, Value>,
    key: &str,
    label: &str,
) -> Result<String, AppError> {
    env.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            AppError::localized(
                "claude_desktop.provider.env_key_missing",
                format!("Claude Desktop 供应商缺少 {label}"),
                format!("Claude Desktop provider is missing {label}"),
            )
        })
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
        if let Some(api_format) = meta.api_format.as_deref() {
            if !api_format.trim().is_empty() && api_format != "anthropic" {
                return Err(AppError::localized(
                    "claude_desktop.provider.api_format_unsupported",
                    "Claude Desktop 直连模式只支持原生 Anthropic Messages API",
                    "Claude Desktop direct mode only supports native Anthropic Messages API",
                ));
            }
        }
        if matches!(meta.claude_desktop_mode, Some(ClaudeDesktopMode::Proxy)) {
            return Err(AppError::localized(
                "claude_desktop.provider.mode_unsupported",
                "该供应商是 Claude Desktop 本地路由模式，不能按直连模式写入",
                "This provider uses Claude Desktop proxy mode and cannot be written as direct mode",
            ));
        }
        if matches!(
            meta.provider_type.as_deref(),
            Some("github_copilot") | Some("codex_oauth")
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
    if let Some(api_format) = provider
        .meta
        .as_ref()
        .and_then(|meta| meta.api_format.as_deref())
    {
        if !matches!(
            api_format,
            "" | "anthropic" | "openai_chat" | "openai_responses" | "gemini_native"
        ) {
            return Err(AppError::localized(
                "claude_desktop.provider.api_format_unsupported",
                format!("Claude Desktop 本地路由模式不支持 API 格式: {api_format}"),
                format!("Claude Desktop proxy mode does not support API format: {api_format}"),
            ));
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

    if provider
        .meta
        .as_ref()
        .and_then(|meta| meta.provider_type.as_deref())
        .is_some_and(|value| matches!(value, "github_copilot" | "codex_oauth"))
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
    let route = proxy_model_routes(provider)?
        .into_iter()
        .find(|route| route.route_id == requested)
        .ok_or_else(|| {
            AppError::localized(
                "claude_desktop.provider.route_unknown",
                format!("Claude Desktop 模型路由未配置: {requested}"),
                format!("Claude Desktop model route is not configured: {requested}"),
            )
        })?;
    body["model"] = json!(route.upstream_model);
    Ok(body)
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
    let supports_1m_default = !matches!(
        provider
            .meta
            .as_ref()
            .and_then(|meta| meta.provider_type.as_deref()),
        Some("github_copilot") | Some("codex_oauth")
    );

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
