/// Deep link import functionality for CC Switch
///
/// This module implements the ccswitch:// protocol for importing provider configurations
/// via deep links. See docs/ccswitch-deeplink-design.md for detailed design.
use crate::error::AppError;
use crate::prompt::Prompt;
use crate::provider::Provider;
use crate::services::{McpService, PromptService, ProviderService, SkillRepo, SkillService};
use crate::store::AppState;
use crate::AppType;
use base64::{engine::general_purpose, Engine};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

/// Deep link import request model.
///
/// The shape follows upstream cc-switch's extensible deeplink payload: provider,
/// MCP, prompt, and skill links share one confirmation model, with fields filled
/// according to the resource type.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepLinkImportRequest {
    /// Protocol version (e.g., "v1")
    pub version: String,
    /// Resource type to import: provider | mcp | prompt | skill
    pub resource: String,

    /// Target application (provider/prompt/skill)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    /// Resource name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Whether to enable after import.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    /// Provider homepage URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    /// API endpoint/base URL. Multiple provider endpoints may be comma-separated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// API key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Optional provider icon name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Optional model name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Optional notes/description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Optional Haiku model (Claude only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub haiku_model: Option<String>,
    /// Optional Sonnet model (Claude only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sonnet_model: Option<String>,
    /// Optional Opus model (Claude only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opus_model: Option<String>,

    /// Base64 encoded prompt/MCP content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Prompt description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Target applications for MCP, comma-separated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apps: Option<String>,

    /// GitHub repository for skill, owner/name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// Skill directory name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
    /// Skill repository branch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Optional subdirectory inside the skill repository.
    #[serde(rename = "skillsPath", skip_serializing_if = "Option::is_none")]
    pub skills_path: Option<String>,

    /// Base64 encoded config content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<String>,
    /// Config format (json/toml).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_format: Option<String>,
    /// Remote config URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_url: Option<String>,

    /// Whether to enable usage query.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_enabled: Option<bool>,
    /// Base64 encoded usage query script.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_script: Option<String>,
    /// Usage query API key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_api_key: Option<String>,
    /// Usage query base URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_base_url: Option<String>,
    /// Usage query access token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_access_token: Option<String>,
    /// Usage query user id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_user_id: Option<String>,
    /// Auto query interval in minutes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_auto_interval: Option<u64>,
}

/// Parse a ccswitch:// URL into a DeepLinkImportRequest
///
/// Expected format:
/// ccswitch://v1/import?resource=provider&app=claude&name=...&homepage=...&endpoint=...&apiKey=...
pub fn parse_deeplink_url(url_str: &str) -> Result<DeepLinkImportRequest, AppError> {
    // Parse URL
    let url = Url::parse(url_str)
        .map_err(|e| AppError::InvalidInput(format!("Invalid deep link URL: {e}")))?;

    // Validate scheme
    let scheme = url.scheme();
    if scheme != "ccswitch" {
        return Err(AppError::InvalidInput(format!(
            "Invalid scheme: expected 'ccswitch', got '{scheme}'"
        )));
    }

    // Extract version from host
    let version = url
        .host_str()
        .ok_or_else(|| AppError::InvalidInput("Missing version in URL host".to_string()))?
        .to_string();

    // Validate version
    if version != "v1" {
        return Err(AppError::InvalidInput(format!(
            "Unsupported protocol version: {version}"
        )));
    }

    // Extract path (should be "/import")
    let path = url.path();
    if path != "/import" {
        return Err(AppError::InvalidInput(format!(
            "Invalid path: expected '/import', got '{path}'"
        )));
    }

    // Parse query parameters
    let params: HashMap<String, String> = url.query_pairs().into_owned().collect();

    // Extract and validate resource type
    let resource = params
        .get("resource")
        .ok_or_else(|| AppError::InvalidInput("Missing 'resource' parameter".to_string()))?
        .clone();

    match resource.as_str() {
        "provider" => parse_provider_params(&params, version, resource),
        "prompt" => parse_prompt_params(&params, version, resource),
        "mcp" => parse_mcp_params(&params, version, resource),
        "skill" => parse_skill_params(&params, version, resource),
        _ => Err(AppError::InvalidInput(format!(
            "Unsupported resource type: {resource}"
        ))),
    }
}

/// Merge embedded provider configuration into a parsed deep link request.
///
/// Priority follows upstream cc-switch: explicit URL parameters win over fields
/// extracted from inline config. Remote config URLs are intentionally rejected
/// here, matching upstream 3.15.0 behavior and avoiding server-side URL fetches.
pub fn parse_and_merge_config(
    request: &DeepLinkImportRequest,
) -> Result<DeepLinkImportRequest, AppError> {
    if request.config.is_none() && request.config_url.is_none() {
        return Ok(request.clone());
    }

    let config_content = if let Some(config_b64) = &request.config {
        decode_base64_utf8("config", config_b64)?
    } else if request.config_url.is_some() {
        return Err(AppError::InvalidInput(
            "Remote config URL is not yet supported. Use inline config instead.".to_string(),
        ));
    } else {
        return Ok(request.clone());
    };

    let format = request.config_format.as_deref().unwrap_or("json");
    let config_value = match format {
        "json" => serde_json::from_str::<serde_json::Value>(&config_content)
            .map_err(|err| AppError::InvalidInput(format!("Invalid JSON config: {err}")))?,
        "toml" => {
            let toml_value = toml::from_str::<toml::Value>(&config_content)
                .map_err(|err| AppError::InvalidInput(format!("Invalid TOML config: {err}")))?;
            serde_json::to_value(toml_value).map_err(|err| {
                AppError::Message(format!("Failed to convert TOML to JSON: {err}"))
            })?
        }
        _ => {
            return Err(AppError::InvalidInput(format!(
                "Unsupported config format: {format}"
            )))
        }
    };

    let mut merged = request.clone();
    if request.resource != "provider" {
        return Ok(merged);
    }

    match request.app.as_deref().unwrap_or("") {
        "claude" => merge_claude_config(&mut merged, &config_value)?,
        "codex" => merge_codex_config(&mut merged, &config_value),
        "gemini" => merge_gemini_config(&mut merged, &config_value),
        "opencode" | "openclaw" => merge_additive_config(&mut merged, &config_value),
        "" => {}
        other => {
            return Err(AppError::InvalidInput(format!(
                "Invalid app type for provider config merge: {other}"
            )))
        }
    }

    Ok(merged)
}

fn merge_claude_config(
    request: &mut DeepLinkImportRequest,
    config: &serde_json::Value,
) -> Result<(), AppError> {
    let env = config
        .get("env")
        .and_then(|value| value.as_object())
        .ok_or_else(|| {
            AppError::InvalidInput("Claude config must have 'env' object".to_string())
        })?;

    if is_blank(request.api_key.as_deref()) {
        if let Some(token) = env
            .get("ANTHROPIC_AUTH_TOKEN")
            .and_then(|value| value.as_str())
        {
            request.api_key = Some(token.to_string());
        } else if let Some(key) = env
            .get("ANTHROPIC_API_KEY")
            .and_then(|value| value.as_str())
        {
            request.api_key = Some(key.to_string());
        }
    }
    if is_blank(request.endpoint.as_deref()) {
        if let Some(base_url) = env
            .get("ANTHROPIC_BASE_URL")
            .and_then(|value| value.as_str())
        {
            request.endpoint = Some(base_url.to_string());
        }
    }
    if is_blank(request.homepage.as_deref()) {
        fill_homepage_from_endpoint(request, "https://anthropic.com");
    }
    if request.model.is_none() {
        request.model = env
            .get("ANTHROPIC_MODEL")
            .and_then(|value| value.as_str())
            .map(ToString::to_string);
    }
    if request.haiku_model.is_none() {
        request.haiku_model = env
            .get("ANTHROPIC_DEFAULT_HAIKU_MODEL")
            .and_then(|value| value.as_str())
            .map(ToString::to_string);
    }
    if request.sonnet_model.is_none() {
        request.sonnet_model = env
            .get("ANTHROPIC_DEFAULT_SONNET_MODEL")
            .and_then(|value| value.as_str())
            .map(ToString::to_string);
    }
    if request.opus_model.is_none() {
        request.opus_model = env
            .get("ANTHROPIC_DEFAULT_OPUS_MODEL")
            .and_then(|value| value.as_str())
            .map(ToString::to_string);
    }

    Ok(())
}

fn merge_codex_config(request: &mut DeepLinkImportRequest, config: &serde_json::Value) {
    if is_blank(request.api_key.as_deref()) {
        if let Some(api_key) = config
            .get("auth")
            .and_then(|value| value.get("OPENAI_API_KEY"))
            .and_then(|value| value.as_str())
        {
            request.api_key = Some(api_key.to_string());
        }
    }

    if let Some(config_str) = config.get("config").and_then(|value| value.as_str()) {
        if let Ok(toml_value) = toml::from_str::<toml::Value>(config_str) {
            if is_blank(request.endpoint.as_deref()) {
                if let Some(base_url) = extract_codex_base_url(&toml_value) {
                    request.endpoint = Some(base_url);
                }
            }
            if request.model.is_none() {
                request.model = toml_value
                    .get("model")
                    .and_then(|value| value.as_str())
                    .map(ToString::to_string);
            }
        }
    }

    if is_blank(request.homepage.as_deref()) {
        fill_homepage_from_endpoint(request, "https://openai.com");
    }
}

fn merge_gemini_config(request: &mut DeepLinkImportRequest, config: &serde_json::Value) {
    let env = config
        .get("env")
        .and_then(|value| value.as_object())
        .cloned()
        .map(serde_json::Value::Object);
    let source = env.as_ref().unwrap_or(config);

    if is_blank(request.api_key.as_deref()) {
        if let Some(api_key) = source
            .get("GEMINI_API_KEY")
            .and_then(|value| value.as_str())
        {
            request.api_key = Some(api_key.to_string());
        }
    }
    if is_blank(request.endpoint.as_deref()) {
        if let Some(base_url) = source
            .get("GOOGLE_GEMINI_BASE_URL")
            .or_else(|| source.get("GEMINI_BASE_URL"))
            .and_then(|value| value.as_str())
        {
            request.endpoint = Some(base_url.to_string());
        }
    }
    if request.model.is_none() {
        request.model = source
            .get("GEMINI_MODEL")
            .and_then(|value| value.as_str())
            .map(ToString::to_string);
    }
    if is_blank(request.homepage.as_deref()) {
        fill_homepage_from_endpoint(request, "https://ai.google.dev");
    }
}

fn merge_additive_config(request: &mut DeepLinkImportRequest, config: &serde_json::Value) {
    if is_blank(request.api_key.as_deref()) {
        if let Some(api_key) = config
            .get("apiKey")
            .or_else(|| config.get("api_key"))
            .and_then(|value| value.as_str())
        {
            request.api_key = Some(api_key.to_string());
        }
    }
    if is_blank(request.endpoint.as_deref()) {
        if let Some(base_url) = config
            .get("baseUrl")
            .or_else(|| config.get("base_url"))
            .or_else(|| {
                config
                    .get("options")
                    .and_then(|options| options.get("baseURL"))
            })
            .and_then(|value| value.as_str())
        {
            request.endpoint = Some(base_url.to_string());
        }
    }
    if is_blank(request.homepage.as_deref()) {
        fill_homepage_from_endpoint(request, "https://opencode.ai");
    }
}

fn is_blank(value: Option<&str>) -> bool {
    value.map(str::trim).unwrap_or_default().is_empty()
}

fn fill_homepage_from_endpoint(request: &mut DeepLinkImportRequest, fallback: &str) {
    let Some(endpoint) = request
        .endpoint
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    else {
        return;
    };
    request.homepage =
        infer_homepage_from_endpoint(endpoint).or_else(|| Some(fallback.to_string()));
}

fn infer_homepage_from_endpoint(endpoint: &str) -> Option<String> {
    let url = Url::parse(endpoint).ok()?;
    let host = url.host_str()?;
    Some(format!("{}://{}", url.scheme(), host))
}

fn extract_codex_base_url(toml_value: &toml::Value) -> Option<String> {
    let providers = toml_value
        .get("model_providers")
        .and_then(|value| value.as_table())?;
    providers.values().find_map(|provider| {
        provider
            .get("base_url")
            .and_then(|value| value.as_str())
            .map(ToString::to_string)
    })
}

fn parse_provider_params(
    params: &HashMap<String, String>,
    version: String,
    resource: String,
) -> Result<DeepLinkImportRequest, AppError> {
    // Extract required fields
    let app = params
        .get("app")
        .ok_or_else(|| AppError::InvalidInput("Missing 'app' parameter".to_string()))?
        .clone();

    if !matches!(
        app.as_str(),
        "claude" | "codex" | "gemini" | "opencode" | "openclaw"
    ) {
        return Err(AppError::InvalidInput(format!(
            "Invalid app type: must be 'claude', 'codex', 'gemini', 'opencode', or 'openclaw', got '{app}'"
        )));
    }

    let name = params
        .get("name")
        .ok_or_else(|| AppError::InvalidInput("Missing 'name' parameter".to_string()))?
        .clone();

    let homepage = params.get("homepage").cloned();
    let endpoint = params.get("endpoint").cloned();
    let api_key = params.get("apiKey").cloned();

    // Validate URLs
    if let Some(homepage) = &homepage {
        if !homepage.trim().is_empty() {
            validate_url(homepage, "homepage")?;
        }
    }
    if let Some(endpoint) = &endpoint {
        for (index, endpoint) in endpoint.split(',').enumerate() {
            let endpoint = endpoint.trim();
            if !endpoint.is_empty() {
                validate_url(endpoint, &format!("endpoint[{index}]"))?;
            }
        }
    }

    // Extract optional fields
    let model = params.get("model").cloned();
    let notes = params.get("notes").cloned();
    let icon = params
        .get("icon")
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty());
    let enabled = params.get("enabled").and_then(|value| value.parse().ok());

    Ok(DeepLinkImportRequest {
        version,
        resource,
        app: Some(app),
        name: Some(name),
        enabled,
        homepage,
        endpoint,
        api_key,
        icon,
        model,
        notes,
        haiku_model: params.get("haikuModel").cloned(),
        sonnet_model: params.get("sonnetModel").cloned(),
        opus_model: params.get("opusModel").cloned(),
        config: params.get("config").cloned(),
        config_format: params.get("configFormat").cloned(),
        config_url: params.get("configUrl").cloned(),
        usage_enabled: params
            .get("usageEnabled")
            .and_then(|value| value.parse().ok()),
        usage_script: params.get("usageScript").cloned(),
        usage_api_key: params.get("usageApiKey").cloned(),
        usage_base_url: params.get("usageBaseUrl").cloned(),
        usage_access_token: params.get("usageAccessToken").cloned(),
        usage_user_id: params.get("usageUserId").cloned(),
        usage_auto_interval: params
            .get("usageAutoInterval")
            .and_then(|value| value.parse().ok()),
        ..Default::default()
    })
}

fn parse_prompt_params(
    params: &HashMap<String, String>,
    version: String,
    resource: String,
) -> Result<DeepLinkImportRequest, AppError> {
    let app = params
        .get("app")
        .ok_or_else(|| AppError::InvalidInput("Missing 'app' parameter for prompt".to_string()))?
        .clone();
    let name = params
        .get("name")
        .ok_or_else(|| AppError::InvalidInput("Missing 'name' parameter for prompt".to_string()))?
        .clone();
    let content = params
        .get("content")
        .ok_or_else(|| {
            AppError::InvalidInput("Missing 'content' parameter for prompt".to_string())
        })?
        .clone();

    AppType::parse_supported(&app)?;

    Ok(DeepLinkImportRequest {
        version,
        resource,
        app: Some(app),
        name: Some(name),
        enabled: params.get("enabled").and_then(|value| value.parse().ok()),
        content: Some(content),
        description: params.get("description").cloned(),
        ..Default::default()
    })
}

fn parse_mcp_params(
    params: &HashMap<String, String>,
    version: String,
    resource: String,
) -> Result<DeepLinkImportRequest, AppError> {
    let apps = params
        .get("apps")
        .ok_or_else(|| AppError::InvalidInput("Missing 'apps' parameter for MCP".to_string()))?
        .clone();
    let name = params
        .get("name")
        .ok_or_else(|| AppError::InvalidInput("Missing 'name' parameter for MCP".to_string()))?
        .clone();
    let config = params
        .get("config")
        .or_else(|| params.get("content"))
        .ok_or_else(|| AppError::InvalidInput("Missing 'config' parameter for MCP".to_string()))?
        .clone();

    parse_mcp_apps(&apps)?;

    Ok(DeepLinkImportRequest {
        version,
        resource,
        name: Some(name),
        enabled: params.get("enabled").and_then(|value| value.parse().ok()),
        apps: Some(apps),
        config: Some(config),
        ..Default::default()
    })
}

fn parse_skill_params(
    params: &HashMap<String, String>,
    version: String,
    resource: String,
) -> Result<DeepLinkImportRequest, AppError> {
    let app = params
        .get("app")
        .cloned()
        .unwrap_or_else(|| "claude".to_string());
    AppType::parse_skills_app(&app)?;

    let repo = params
        .get("repo")
        .ok_or_else(|| AppError::InvalidInput("Missing 'repo' parameter for skill".to_string()))?
        .clone();
    if repo.split('/').count() != 2 {
        return Err(AppError::InvalidInput(format!(
            "Invalid repo format: expected 'owner/name', got '{repo}'"
        )));
    }

    Ok(DeepLinkImportRequest {
        version,
        resource,
        app: Some(app),
        name: params.get("name").cloned(),
        enabled: params.get("enabled").and_then(|value| value.parse().ok()),
        repo: Some(repo),
        directory: params.get("directory").cloned(),
        branch: params.get("branch").cloned(),
        skills_path: params.get("skillsPath").cloned(),
        ..Default::default()
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpImportResult {
    pub imported_count: usize,
    pub imported_ids: Vec<String>,
    pub failed: Vec<McpImportError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpImportError {
    pub id: String,
    pub error: String,
}

/// Import a prompt from a deep link request.
pub fn import_prompt_from_deeplink(
    state: &AppState,
    request: DeepLinkImportRequest,
) -> Result<String, AppError> {
    if request.resource != "prompt" {
        return Err(AppError::InvalidInput(format!(
            "Expected prompt resource, got '{}'",
            request.resource
        )));
    }

    let app = required_field(request.app.as_deref(), "app")?;
    let app_type = AppType::parse_supported(app)?;
    let name = required_field(request.name.as_deref(), "name")?;
    let content_b64 = required_field(request.content.as_deref(), "content")?;
    let content = decode_base64_utf8("content", content_b64)?;

    let timestamp = chrono::Utc::now().timestamp_millis();
    let id = format!("{}-{timestamp}", sanitize_id_component(name, "prompt"));
    let should_enable = request.enabled.unwrap_or(false);
    let prompt = Prompt {
        id: id.clone(),
        name: name.to_string(),
        content,
        description: request.description,
        enabled: false,
        created_at: Some(timestamp),
        updated_at: Some(timestamp),
    };

    PromptService::upsert_prompt(state, app_type.clone(), &id, prompt)?;
    if should_enable {
        PromptService::enable_prompt(state, app_type, &id)?;
    }

    Ok(id)
}

/// Import MCP servers from a deep link request.
pub fn import_mcp_from_deeplink(
    state: &AppState,
    request: DeepLinkImportRequest,
) -> Result<McpImportResult, AppError> {
    if request.resource != "mcp" {
        return Err(AppError::InvalidInput(format!(
            "Expected mcp resource, got '{}'",
            request.resource
        )));
    }

    let apps = required_field(request.apps.as_deref(), "apps")?;
    let target_apps = parse_mcp_apps(apps)?;
    let config_b64 = required_field(request.config.as_deref(), "config")?;
    let config = decode_base64_utf8("config", config_b64)?;
    let config_json: serde_json::Value = serde_json::from_str(&config)
        .map_err(|err| AppError::InvalidInput(format!("Invalid JSON in MCP config: {err}")))?;
    let mcp_servers = config_json
        .get("mcpServers")
        .and_then(|value| value.as_object())
        .ok_or_else(|| {
            AppError::InvalidInput("MCP config must contain 'mcpServers' object".to_string())
        })?;

    if mcp_servers.is_empty() {
        return Err(AppError::InvalidInput(
            "No MCP servers found in config".to_string(),
        ));
    }

    let existing_servers = McpService::get_all_servers(state)?;
    let mut imported_ids = Vec::new();
    let mut failed = Vec::new();

    for (id, spec) in mcp_servers {
        let server = if let Some(existing) = existing_servers.get(id) {
            let mut merged = existing.clone();
            if target_apps.claude {
                merged.apps.claude = true;
            }
            if target_apps.codex {
                merged.apps.codex = true;
            }
            if target_apps.gemini {
                merged.apps.gemini = true;
            }
            if target_apps.opencode {
                merged.apps.opencode = true;
            }
            merged
        } else {
            crate::app_config::McpServer {
                id: id.clone(),
                name: id.clone(),
                server: spec.clone(),
                apps: target_apps.clone(),
                description: None,
                homepage: None,
                docs: None,
                tags: vec!["imported".to_string()],
            }
        };

        match McpService::upsert_server(state, server) {
            Ok(()) => imported_ids.push(id.clone()),
            Err(err) => failed.push(McpImportError {
                id: id.clone(),
                error: err.to_string(),
            }),
        }
    }

    Ok(McpImportResult {
        imported_count: imported_ids.len(),
        imported_ids,
        failed,
    })
}

/// Import a skill repository from a deep link request.
pub fn import_skill_from_deeplink(
    state: &AppState,
    request: DeepLinkImportRequest,
) -> Result<String, AppError> {
    if request.resource != "skill" {
        return Err(AppError::InvalidInput(format!(
            "Expected skill resource, got '{}'",
            request.resource
        )));
    }

    let repo = required_field(request.repo.as_deref(), "repo")?;
    let mut parts = repo.split('/');
    let owner = parts.next().unwrap_or_default().trim();
    let name = parts.next().unwrap_or_default().trim();
    if owner.is_empty() || name.is_empty() || parts.next().is_some() {
        return Err(AppError::InvalidInput(format!(
            "Invalid repo format: expected 'owner/name', got '{repo}'"
        )));
    }

    let skill_repo = SkillRepo {
        owner: owner.to_string(),
        name: name.to_string(),
        branch: request.branch.unwrap_or_else(|| "main".to_string()),
        enabled: request.enabled.unwrap_or(true),
        skills_path: request.skills_path,
    };

    let service = SkillService::new().map_err(|err| AppError::Config(err.to_string()))?;
    state.update_config(|config| {
        service
            .add_repo(&mut config.skills, skill_repo)
            .map_err(|err| AppError::Config(err.to_string()))
    })?;

    Ok(format!("{owner}/{name}"))
}

fn parse_mcp_apps(apps: &str) -> Result<crate::app_config::McpApps, AppError> {
    let mut parsed = crate::app_config::McpApps::default();
    for app in apps.split(',').map(str::trim).filter(|app| !app.is_empty()) {
        let app_type = AppType::parse_supported(app)?;
        match app_type {
            AppType::Claude | AppType::Codex | AppType::Gemini | AppType::Opencode => {
                parsed.set_enabled_for(&app_type, true);
            }
            AppType::ClaudeDesktop | AppType::OpenClaw | AppType::Omo | AppType::OmoSlim => {
                return Err(AppError::InvalidInput(format!(
                    "MCP deep link does not support app '{}'",
                    app_type.as_str()
                )));
            }
        }
    }

    if parsed.is_empty() {
        return Err(AppError::InvalidInput(
            "At least one app must be specified in 'apps'".to_string(),
        ));
    }

    Ok(parsed)
}

fn decode_base64_utf8(field: &str, raw: &str) -> Result<String, AppError> {
    let bytes = decode_base64_param(field, raw)?;
    String::from_utf8(bytes)
        .map_err(|err| AppError::InvalidInput(format!("Invalid UTF-8 in {field}: {err}")))
}

fn decode_base64_param(field: &str, raw: &str) -> Result<Vec<u8>, AppError> {
    let trimmed = raw.trim_matches(|ch| ch == '\r' || ch == '\n');
    let mut candidates = Vec::<String>::new();

    if trimmed.contains(' ') {
        candidates.push(trimmed.replace(' ', "+"));
    }
    if !trimmed.is_empty() && !candidates.iter().any(|value| value == trimmed) {
        candidates.push(trimmed.to_string());
    }

    for value in candidates.clone() {
        let mut padded = value.clone();
        let remainder = padded.len() % 4;
        if remainder != 0 {
            padded.extend(std::iter::repeat('=').take(4 - remainder));
        }
        if !candidates.iter().any(|candidate| candidate == &padded) {
            candidates.push(padded);
        }
    }

    let engines = [
        &general_purpose::STANDARD,
        &general_purpose::STANDARD_NO_PAD,
        &general_purpose::URL_SAFE,
        &general_purpose::URL_SAFE_NO_PAD,
    ];
    let mut last_error = None;
    for candidate in candidates {
        for engine in engines {
            match engine.decode(&candidate) {
                Ok(bytes) => return Ok(bytes),
                Err(err) => last_error = Some(err.to_string()),
            }
        }
    }

    Err(AppError::InvalidInput(format!(
        "{field} 参数 Base64 解码失败：{}。请确认链接参数已用 Base64 编码并经过 URL 转义。",
        last_error.unwrap_or_else(|| "未知错误".to_string())
    )))
}

fn sanitize_id_component(value: &str, fallback: &str) -> String {
    let sanitized = value
        .chars()
        .filter(|ch| ch.is_alphanumeric() || *ch == '-' || *ch == '_')
        .collect::<String>()
        .to_lowercase();
    if sanitized.is_empty() {
        fallback.to_string()
    } else {
        sanitized
    }
}

/// Validate that a string is a valid HTTP(S) URL
fn validate_url(url_str: &str, field_name: &str) -> Result<(), AppError> {
    let url = Url::parse(url_str)
        .map_err(|e| AppError::InvalidInput(format!("Invalid URL for '{field_name}': {e}")))?;

    let scheme = url.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(AppError::InvalidInput(format!(
            "Invalid URL scheme for '{field_name}': must be http or https, got '{scheme}'"
        )));
    }

    Ok(())
}

/// Import a provider from a deep link request
///
/// This function:
/// 1. Validates the request
/// 2. Converts it to a Provider structure
/// 3. Delegates to ProviderService for actual import
pub fn import_provider_from_deeplink(
    state: &AppState,
    request: DeepLinkImportRequest,
) -> Result<String, AppError> {
    let request = parse_and_merge_config(&request)?;

    // Parse app type
    let app = required_field(request.app.as_deref(), "app")?;
    let app_type = AppType::parse_supported(app)
        .map_err(|_| AppError::InvalidInput(format!("Invalid app type: {app}")))?;

    // Build provider configuration based on app type
    let mut provider = build_provider_from_request(&app_type, &request)?;

    // Generate a unique ID for the provider using timestamp + sanitized name
    // This is similar to how frontend generates IDs
    let timestamp = chrono::Utc::now().timestamp_millis();
    let sanitized_name =
        sanitize_id_component(request.name.as_deref().unwrap_or("provider"), "provider");
    provider.id = format!("{sanitized_name}-{timestamp}");

    let provider_id = provider.id.clone();

    // Use ProviderService to add the provider
    ProviderService::add(state, app_type, provider)?;

    Ok(provider_id)
}

/// Build a Provider structure from a deep link request
fn build_provider_from_request(
    app_type: &AppType,
    request: &DeepLinkImportRequest,
) -> Result<Provider, AppError> {
    use serde_json::json;
    let name = required_field(request.name.as_deref(), "name")?;
    let endpoint = required_field(request.endpoint.as_deref(), "endpoint")?;
    let api_key = required_field(request.api_key.as_deref(), "apiKey")?;
    let homepage = required_field(request.homepage.as_deref(), "homepage")?;

    let settings_config = match app_type {
        AppType::Claude => {
            // Claude configuration structure
            let mut env = serde_json::Map::new();
            env.insert("ANTHROPIC_AUTH_TOKEN".to_string(), json!(api_key));
            env.insert("ANTHROPIC_BASE_URL".to_string(), json!(endpoint));

            // Add model if provided (use as default model)
            if let Some(model) = &request.model {
                env.insert("ANTHROPIC_MODEL".to_string(), json!(model));
                json!({ "env": env, "model": model })
            } else {
                json!({ "env": env })
            }
        }
        AppType::Codex => {
            // Codex configuration structure
            // For Codex, we store auth.json (JSON) and config.toml (TOML string) in settings_config。
            //
            // 这里尽量与前端 `getCodexCustomTemplate` 的默认模板保持一致，
            // 再根据深链接参数注入 base_url / model，避免出现“只有 base_url 行”的极简配置，
            // 让通过 UI 新建和通过深链接导入的 Codex 自定义供应商行为一致。

            // 1. 生成一个适合作为 model_provider 名的安全标识
            //    规则尽量与前端 codexProviderPresets.generateThirdPartyConfig 保持一致：
            //    - 转小写
            //    - 非 [a-z0-9_] 统一替换为下划线
            //    - 去掉首尾下划线
            //    - 若结果为空，则使用 "custom"
            let clean_provider_name = {
                let raw: String = name.chars().filter(|c| !c.is_control()).collect();
                let lower = raw.to_lowercase();
                let mut key: String = lower
                    .chars()
                    .map(|c| match c {
                        'a'..='z' | '0'..='9' | '_' => c,
                        _ => '_',
                    })
                    .collect();

                // 去掉首尾下划线
                while key.starts_with('_') {
                    key.remove(0);
                }
                while key.ends_with('_') {
                    key.pop();
                }

                if key.is_empty() {
                    "custom".to_string()
                } else {
                    key
                }
            };

            // 2. 模型名称：优先使用 deeplink 中的 model，否则退回到 Codex 默认模型
            let model_name = request
                .model
                .as_deref()
                .unwrap_or("gpt-5-codex")
                .to_string();

            // 3. 端点：与 UI 中 Base URL 处理方式保持一致，去掉结尾多余的斜杠
            let endpoint = endpoint.trim().trim_end_matches('/').to_string();

            // 4. 组装 config.toml 内容
            // 使用 Rust 1.58+ 的内联格式化语法，避免 clippy::uninlined_format_args 警告
            let config_toml = format!(
                r#"model_provider = "{clean_provider_name}"
model = "{model_name}"
model_reasoning_effort = "high"
disable_response_storage = true

[model_providers.{clean_provider_name}]
name = "{clean_provider_name}"
base_url = "{endpoint}"
wire_api = "responses"
requires_openai_auth = true
"#
            );

            json!({
                "auth": {
                    "OPENAI_API_KEY": api_key,
                },
                "config": config_toml
            })
        }
        AppType::Gemini => {
            // Gemini configuration structure (.env format)
            let mut env = serde_json::Map::new();
            env.insert("GEMINI_API_KEY".to_string(), json!(api_key));
            env.insert("GOOGLE_GEMINI_BASE_URL".to_string(), json!(endpoint));

            // Add model if provided
            if let Some(model) = &request.model {
                env.insert("GEMINI_MODEL".to_string(), json!(model));
            }

            json!({ "env": env })
        }
        AppType::OpenClaw => json!({
            "baseUrl": endpoint,
            "apiKey": api_key,
            "api": "openai-completions",
            "models": request
                .model
                .iter()
                .map(|model| json!({ "id": model }))
                .collect::<Vec<_>>()
        }),
        AppType::ClaudeDesktop | AppType::Opencode | AppType::Omo | AppType::OmoSlim => {
            return Err(AppError::localized(
                "app_not_supported_yet",
                format!("应用 '{}' 暂未支持，敬请期待。", app_type.as_str()),
                format!("App '{}' is not supported yet.", app_type.as_str()),
            ));
        }
    };

    let provider = Provider {
        id: String::new(), // Will be generated by ProviderService
        name: name.to_string(),
        settings_config,
        website_url: Some(homepage.to_string()),
        category: None,
        created_at: None,
        sort_index: None,
        notes: request.notes.clone(),
        meta: None,
    };

    Ok(provider)
}

fn required_field<'a>(value: Option<&'a str>, field: &str) -> Result<&'a str, AppError> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::InvalidInput(format!("Missing '{field}' parameter")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_valid_claude_deeplink() {
        let url = "ccswitch://v1/import?resource=provider&app=claude&name=Test%20Provider&homepage=https%3A%2F%2Fexample.com&endpoint=https%3A%2F%2Fapi.example.com&apiKey=sk-test-123";

        let request = parse_deeplink_url(url).unwrap();

        assert_eq!(request.version, "v1");
        assert_eq!(request.resource, "provider");
        assert_eq!(request.app.as_deref(), Some("claude"));
        assert_eq!(request.name.as_deref(), Some("Test Provider"));
        assert_eq!(request.homepage.as_deref(), Some("https://example.com"));
        assert_eq!(request.endpoint.as_deref(), Some("https://api.example.com"));
        assert_eq!(request.api_key.as_deref(), Some("sk-test-123"));
    }

    #[test]
    fn test_parse_deeplink_with_notes() {
        let url = "ccswitch://v1/import?resource=provider&app=codex&name=Codex&homepage=https%3A%2F%2Fcodex.com&endpoint=https%3A%2F%2Fapi.codex.com&apiKey=key123&notes=Test%20notes";

        let request = parse_deeplink_url(url).unwrap();

        assert_eq!(request.notes, Some("Test notes".to_string()));
    }

    #[test]
    fn test_parse_prompt_deeplink() {
        let url = "ccswitch://v1/import?resource=prompt&app=claude&name=Review&content=IyBSZXZpZXc&description=Code%20review&enabled=true";

        let request = parse_deeplink_url(url).unwrap();

        assert_eq!(request.resource, "prompt");
        assert_eq!(request.app.as_deref(), Some("claude"));
        assert_eq!(request.name.as_deref(), Some("Review"));
        assert_eq!(request.content.as_deref(), Some("IyBSZXZpZXc"));
        assert_eq!(request.description.as_deref(), Some("Code review"));
        assert_eq!(request.enabled, Some(true));
    }

    #[test]
    fn test_parse_mcp_deeplink() {
        let url = "ccswitch://v1/import?resource=mcp&apps=claude,codex&name=tools&config=eyJtY3BTZXJ2ZXJzIjp7fX0";

        let request = parse_deeplink_url(url).unwrap();

        assert_eq!(request.resource, "mcp");
        assert_eq!(request.apps.as_deref(), Some("claude,codex"));
        assert_eq!(request.config.as_deref(), Some("eyJtY3BTZXJ2ZXJzIjp7fX0"));
    }

    #[test]
    fn test_parse_skill_deeplink_repo_only() {
        let url =
            "ccswitch://v1/import?resource=skill&repo=owner%2Fskills&branch=main&skillsPath=skills";

        let request = parse_deeplink_url(url).unwrap();

        assert_eq!(request.resource, "skill");
        assert_eq!(request.app.as_deref(), Some("claude"));
        assert_eq!(request.repo.as_deref(), Some("owner/skills"));
        assert_eq!(request.branch.as_deref(), Some("main"));
        assert_eq!(request.skills_path.as_deref(), Some("skills"));
        assert!(request.directory.is_none());
    }

    #[test]
    fn test_merge_claude_inline_config_fills_missing_provider_fields() {
        let config = general_purpose::URL_SAFE_NO_PAD.encode(
            json!({
                "env": {
                    "ANTHROPIC_AUTH_TOKEN": "sk-inline",
                    "ANTHROPIC_BASE_URL": "https://api.example.com/v1",
                    "ANTHROPIC_MODEL": "claude-sonnet-4-6",
                    "ANTHROPIC_DEFAULT_HAIKU_MODEL": "claude-haiku-4-5"
                }
            })
            .to_string(),
        );
        let request = DeepLinkImportRequest {
            version: "v1".to_string(),
            resource: "provider".to_string(),
            app: Some("claude".to_string()),
            name: Some("Claude Inline".to_string()),
            config: Some(config),
            config_format: Some("json".to_string()),
            ..Default::default()
        };

        let merged = parse_and_merge_config(&request).expect("merge config");

        assert_eq!(merged.api_key.as_deref(), Some("sk-inline"));
        assert_eq!(
            merged.endpoint.as_deref(),
            Some("https://api.example.com/v1")
        );
        assert_eq!(merged.homepage.as_deref(), Some("https://api.example.com"));
        assert_eq!(merged.model.as_deref(), Some("claude-sonnet-4-6"));
        assert_eq!(merged.haiku_model.as_deref(), Some("claude-haiku-4-5"));
    }

    #[test]
    fn test_merge_config_keeps_explicit_url_params() {
        let config = general_purpose::STANDARD.encode(
            json!({
                "env": {
                    "ANTHROPIC_AUTH_TOKEN": "sk-inline",
                    "ANTHROPIC_BASE_URL": "https://inline.example.com"
                }
            })
            .to_string(),
        );
        let request = DeepLinkImportRequest {
            version: "v1".to_string(),
            resource: "provider".to_string(),
            app: Some("claude".to_string()),
            name: Some("Claude Inline".to_string()),
            endpoint: Some("https://url.example.com".to_string()),
            api_key: Some("sk-url".to_string()),
            config: Some(config),
            config_format: Some("json".to_string()),
            ..Default::default()
        };

        let merged = parse_and_merge_config(&request).expect("merge config");

        assert_eq!(merged.api_key.as_deref(), Some("sk-url"));
        assert_eq!(merged.endpoint.as_deref(), Some("https://url.example.com"));
    }

    #[test]
    fn test_merge_codex_inline_config_extracts_auth_endpoint_and_model() {
        let config_toml = r#"
model = "gpt-5-codex"
model_provider = "custom"

[model_providers.custom]
name = "custom"
base_url = "https://codex.example.com/v1"
wire_api = "responses"
"#;
        let config = general_purpose::URL_SAFE_NO_PAD.encode(
            json!({
                "auth": { "OPENAI_API_KEY": "codex-key" },
                "config": config_toml
            })
            .to_string(),
        );
        let request = DeepLinkImportRequest {
            version: "v1".to_string(),
            resource: "provider".to_string(),
            app: Some("codex".to_string()),
            name: Some("Codex Inline".to_string()),
            config: Some(config),
            config_format: Some("json".to_string()),
            ..Default::default()
        };

        let merged = parse_and_merge_config(&request).expect("merge config");

        assert_eq!(merged.api_key.as_deref(), Some("codex-key"));
        assert_eq!(
            merged.endpoint.as_deref(),
            Some("https://codex.example.com/v1")
        );
        assert_eq!(merged.model.as_deref(), Some("gpt-5-codex"));
    }

    #[test]
    fn test_merge_config_url_is_rejected_like_upstream() {
        let request = DeepLinkImportRequest {
            version: "v1".to_string(),
            resource: "provider".to_string(),
            app: Some("claude".to_string()),
            name: Some("Remote".to_string()),
            config_url: Some("https://example.com/config.json".to_string()),
            ..Default::default()
        };

        let err = parse_and_merge_config(&request).unwrap_err();

        assert!(err
            .to_string()
            .contains("Remote config URL is not yet supported"));
    }

    #[test]
    fn test_decode_base64_param_accepts_url_safe_without_padding() {
        let decoded = decode_base64_utf8("content", "SGVsbG8td29ybGQ").unwrap();
        assert_eq!(decoded, "Hello-world");
    }

    #[test]
    fn test_parse_invalid_scheme() {
        let url = "https://v1/import?resource=provider&app=claude&name=Test";

        let result = parse_deeplink_url(url);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid scheme"));
    }

    #[test]
    fn test_parse_unsupported_version() {
        let url = "ccswitch://v2/import?resource=provider&app=claude&name=Test";

        let result = parse_deeplink_url(url);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unsupported protocol version"));
    }

    #[test]
    fn test_parse_missing_required_field() {
        let url = "ccswitch://v1/import?resource=provider&app=claude";

        let result = parse_deeplink_url(url);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing 'name' parameter"));
    }

    #[test]
    fn test_validate_invalid_url() {
        let result = validate_url("not-a-url", "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_invalid_scheme() {
        let result = validate_url("ftp://example.com", "test");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must be http or https"));
    }

    #[test]
    fn test_build_claude_provider_with_model() {
        let request = DeepLinkImportRequest {
            version: "v1".to_string(),
            resource: "provider".to_string(),
            app: Some("claude".to_string()),
            name: Some("Test Claude".to_string()),
            homepage: Some("https://example.com".to_string()),
            endpoint: Some("https://api.example.com".to_string()),
            api_key: Some("test-api-key".to_string()),
            model: Some("claude-sonnet-4-20250514".to_string()),
            ..Default::default()
        };

        let provider = build_provider_from_request(&AppType::Claude, &request).unwrap();
        let env = provider.settings_config["env"].as_object().unwrap();
        assert_eq!(env["ANTHROPIC_AUTH_TOKEN"], "test-api-key");
        assert_eq!(env["ANTHROPIC_BASE_URL"], "https://api.example.com");
        assert_eq!(env["ANTHROPIC_MODEL"], "claude-sonnet-4-20250514");
        assert_eq!(
            provider.settings_config["model"],
            "claude-sonnet-4-20250514"
        );
    }

    #[test]
    fn test_build_gemini_provider_with_model() {
        let request = DeepLinkImportRequest {
            version: "v1".to_string(),
            resource: "provider".to_string(),
            app: Some("gemini".to_string()),
            name: Some("Test Gemini".to_string()),
            homepage: Some("https://example.com".to_string()),
            endpoint: Some("https://api.example.com".to_string()),
            api_key: Some("test-api-key".to_string()),
            model: Some("gemini-2.0-flash".to_string()),
            ..Default::default()
        };

        let provider = build_provider_from_request(&AppType::Gemini, &request).unwrap();

        // Verify provider basic info
        assert_eq!(provider.name, "Test Gemini");
        assert_eq!(
            provider.website_url,
            Some("https://example.com".to_string())
        );

        // Verify settings_config structure
        let env = provider.settings_config["env"].as_object().unwrap();
        assert_eq!(env["GEMINI_API_KEY"], "test-api-key");
        assert_eq!(env["GOOGLE_GEMINI_BASE_URL"], "https://api.example.com");
        assert_eq!(env["GEMINI_MODEL"], "gemini-2.0-flash");
    }

    #[test]
    fn test_build_gemini_provider_without_model() {
        let request = DeepLinkImportRequest {
            version: "v1".to_string(),
            resource: "provider".to_string(),
            app: Some("gemini".to_string()),
            name: Some("Test Gemini".to_string()),
            homepage: Some("https://example.com".to_string()),
            endpoint: Some("https://api.example.com".to_string()),
            api_key: Some("test-api-key".to_string()),
            ..Default::default()
        };

        let provider = build_provider_from_request(&AppType::Gemini, &request).unwrap();

        // Verify settings_config structure
        let env = provider.settings_config["env"].as_object().unwrap();
        assert_eq!(env["GEMINI_API_KEY"], "test-api-key");
        assert_eq!(env["GOOGLE_GEMINI_BASE_URL"], "https://api.example.com");
        // Model should not be present
        assert!(env.get("GEMINI_MODEL").is_none());
    }
}
