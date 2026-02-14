/// Deep link import functionality for CC Switch
///
/// This module implements the ccswitch:// protocol for importing provider configurations
/// via deep links. See docs/ccswitch-deeplink-design.md for detailed design.
use crate::error::AppError;
use crate::provider::Provider;
use crate::services::ProviderService;
use crate::store::AppState;
use crate::AppType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

/// Deep link import request model
/// Represents a parsed ccswitch:// URL ready for processing
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepLinkImportRequest {
    /// Protocol version (e.g., "v1")
    pub version: String,
    /// Resource type to import (e.g., "provider")
    pub resource: String,
    /// Target application (claude/codex/gemini)
    pub app: String,
    /// Provider name
    pub name: String,
    /// Provider homepage URL
    pub homepage: String,
    /// API endpoint/base URL
    pub endpoint: String,
    /// API key
    pub api_key: String,
    /// Optional model name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Optional notes/description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
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

    if resource != "provider" {
        return Err(AppError::InvalidInput(format!(
            "Unsupported resource type: {resource}"
        )));
    }

    // Extract required fields
    let app = params
        .get("app")
        .ok_or_else(|| AppError::InvalidInput("Missing 'app' parameter".to_string()))?
        .clone();

    // Validate app type
    if app != "claude" && app != "codex" && app != "gemini" {
        return Err(AppError::InvalidInput(format!(
            "Invalid app type: must be 'claude', 'codex', or 'gemini', got '{app}'"
        )));
    }

    let name = params
        .get("name")
        .ok_or_else(|| AppError::InvalidInput("Missing 'name' parameter".to_string()))?
        .clone();

    let homepage = params
        .get("homepage")
        .ok_or_else(|| AppError::InvalidInput("Missing 'homepage' parameter".to_string()))?
        .clone();

    let endpoint = params
        .get("endpoint")
        .ok_or_else(|| AppError::InvalidInput("Missing 'endpoint' parameter".to_string()))?
        .clone();

    let api_key = params
        .get("apiKey")
        .ok_or_else(|| AppError::InvalidInput("Missing 'apiKey' parameter".to_string()))?
        .clone();

    // Validate URLs
    validate_url(&homepage, "homepage")?;
    validate_url(&endpoint, "endpoint")?;

    // Extract optional fields
    let model = params.get("model").cloned();
    let notes = params.get("notes").cloned();

    Ok(DeepLinkImportRequest {
        version,
        resource,
        app,
        name,
        homepage,
        endpoint,
        api_key,
        model,
        notes,
    })
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
    // Parse app type
    let app_type = AppType::parse_supported(&request.app)
        .map_err(|_| AppError::InvalidInput(format!("Invalid app type: {}", request.app)))?;

    // Build provider configuration based on app type
    let mut provider = build_provider_from_request(&app_type, &request)?;

    // Generate a unique ID for the provider using timestamp + sanitized name
    // This is similar to how frontend generates IDs
    let timestamp = chrono::Utc::now().timestamp_millis();
    let sanitized_name = request
        .name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect::<String>()
        .to_lowercase();
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

    let settings_config = match app_type {
        AppType::Claude => {
            // Claude configuration structure
            let mut env = serde_json::Map::new();
            env.insert("ANTHROPIC_AUTH_TOKEN".to_string(), json!(request.api_key));
            env.insert("ANTHROPIC_BASE_URL".to_string(), json!(request.endpoint));

            // Add model if provided (use as default model)
            if let Some(model) = &request.model {
                env.insert("ANTHROPIC_MODEL".to_string(), json!(model));
            }

            json!({ "env": env })
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
                let raw: String = request.name.chars().filter(|c| !c.is_control()).collect();
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
            let endpoint = request.endpoint.trim().trim_end_matches('/').to_string();

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
                    "OPENAI_API_KEY": request.api_key,
                },
                "config": config_toml
            })
        }
        AppType::Gemini => {
            // Gemini configuration structure (.env format)
            let mut env = serde_json::Map::new();
            env.insert("GEMINI_API_KEY".to_string(), json!(request.api_key));
            env.insert(
                "GOOGLE_GEMINI_BASE_URL".to_string(),
                json!(request.endpoint),
            );

            // Add model if provided
            if let Some(model) = &request.model {
                env.insert("GEMINI_MODEL".to_string(), json!(model));
            }

            json!({ "env": env })
        }
        AppType::Opencode | AppType::Omo => {
            return Err(AppError::localized(
                "app_not_supported_yet",
                format!("应用 '{}' 暂未支持，敬请期待。", app_type.as_str()),
                format!("App '{}' is not supported yet.", app_type.as_str()),
            ));
        }
    };

    let provider = Provider {
        id: String::new(), // Will be generated by ProviderService
        name: request.name.clone(),
        settings_config,
        website_url: Some(request.homepage.clone()),
        category: None,
        created_at: None,
        sort_index: None,
        notes: request.notes.clone(),
        meta: None,
    };

    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_claude_deeplink() {
        let url = "ccswitch://v1/import?resource=provider&app=claude&name=Test%20Provider&homepage=https%3A%2F%2Fexample.com&endpoint=https%3A%2F%2Fapi.example.com&apiKey=sk-test-123";

        let request = parse_deeplink_url(url).unwrap();

        assert_eq!(request.version, "v1");
        assert_eq!(request.resource, "provider");
        assert_eq!(request.app, "claude");
        assert_eq!(request.name, "Test Provider");
        assert_eq!(request.homepage, "https://example.com");
        assert_eq!(request.endpoint, "https://api.example.com");
        assert_eq!(request.api_key, "sk-test-123");
    }

    #[test]
    fn test_parse_deeplink_with_notes() {
        let url = "ccswitch://v1/import?resource=provider&app=codex&name=Codex&homepage=https%3A%2F%2Fcodex.com&endpoint=https%3A%2F%2Fapi.codex.com&apiKey=key123&notes=Test%20notes";

        let request = parse_deeplink_url(url).unwrap();

        assert_eq!(request.notes, Some("Test notes".to_string()));
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
        let url = "ccswitch://v1/import?resource=provider&app=claude&name=Test";

        let result = parse_deeplink_url(url);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing 'homepage' parameter"));
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
    fn test_build_gemini_provider_with_model() {
        let request = DeepLinkImportRequest {
            version: "v1".to_string(),
            resource: "provider".to_string(),
            app: "gemini".to_string(),
            name: "Test Gemini".to_string(),
            homepage: "https://example.com".to_string(),
            endpoint: "https://api.example.com".to_string(),
            api_key: "test-api-key".to_string(),
            model: Some("gemini-2.0-flash".to_string()),
            notes: None,
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
            app: "gemini".to_string(),
            name: "Test Gemini".to_string(),
            homepage: "https://example.com".to_string(),
            endpoint: "https://api.example.com".to_string(),
            api_key: "test-api-key".to_string(),
            model: None,
            notes: None,
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
