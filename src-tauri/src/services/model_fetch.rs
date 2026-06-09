//! 模型列表获取服务
//!
//! 通过 OpenAI 兼容的 GET /v1/models 端点获取供应商可用模型列表。
//! 主要面向第三方聚合站（硅基流动、OpenRouter 等），以及把 Anthropic
//! 协议挂在兼容子路径上的官方供应商（DeepSeek、Kimi、智谱 GLM 等）。

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::{
    error::AppError,
    services::auth::{
        fetch_github_copilot_api_endpoint, github_token_source, GITHUB_COPILOT_API_VERSION,
        GITHUB_COPILOT_EDITOR_VERSION, GITHUB_COPILOT_INTEGRATION_ID,
        GITHUB_COPILOT_PLUGIN_VERSION, GITHUB_COPILOT_USER_AGENT,
    },
    services::{CodexOAuthManager, CopilotAuthManager},
    store::AppState,
};
use std::sync::Arc;

/// 获取到的模型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchedModel {
    pub id: String,
    pub owned_by: Option<String>,
}

const FETCH_TIMEOUT_SECS: u64 = 15;

/// 404/405 响应体截断长度：避免把几十 KB HTML 404 页整页保留到错误串里。
const ERROR_BODY_MAX_CHARS: usize = 512;

/// 已知的「Anthropic 协议兼容子路径」后缀；按长度降序，最长前缀优先匹配。
/// baseURL 命中这些后缀时，候选列表会追加「剥离后缀再拼 /v1/models / /models」的版本。
const KNOWN_COMPAT_SUFFIXES: &[&str] = &[
    "/api/claudecode",
    "/api/anthropic",
    "/apps/anthropic",
    "/api/coding",
    "/claudecode",
    "/anthropic",
    "/step_plan",
    "/coding",
    "/claude",
];

/// 获取供应商的可用模型列表
///
/// 使用 OpenAI 兼容的 GET /v1/models 端点，按候选列表顺序尝试。
pub async fn fetch_models(
    base_url: &str,
    api_key: &str,
    npm: Option<&str>,
    is_full_url: bool,
    models_url_override: Option<&str>,
) -> Result<Vec<FetchedModel>, String> {
    if api_key.is_empty() {
        return Err("API Key is required to fetch models".to_string());
    }

    let npm = npm.unwrap_or("@ai-sdk/openai-compatible");
    let candidates = build_models_url_candidates(base_url, npm, is_full_url, models_url_override)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(FETCH_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;
    let mut last_err: Option<String> = None;

    for url in &candidates {
        log::debug!("[ModelFetch] Trying endpoint: {url}");
        let request = match npm {
            "@ai-sdk/anthropic" => client
                .get(url)
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01"),
            "@ai-sdk/google" => client.get(url).query(&[("key", api_key)]),
            _ => client
                .get(url)
                .header("Authorization", format!("Bearer {api_key}")),
        };
        let response = match request
            .timeout(Duration::from_secs(FETCH_TIMEOUT_SECS))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                last_err = Some(format!("Request failed: {e}"));
                continue;
            }
        };

        let status = response.status();

        if status.is_success() {
            let raw: serde_json::Value = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse response: {e}"))?;

            let mut models = extract_models(&raw, npm);

            models.sort_by(|a, b| a.id.cmp(&b.id));
            return Ok(models);
        }

        if status == StatusCode::NOT_FOUND || status == StatusCode::METHOD_NOT_ALLOWED {
            let body = truncate_body(response.text().await.unwrap_or_default());
            last_err = Some(format!("HTTP {status}: {body}"));
            continue;
        }

        let body = truncate_body(response.text().await.unwrap_or_default());
        if status == StatusCode::BAD_GATEWAY
            || status == StatusCode::SERVICE_UNAVAILABLE
            || status == StatusCode::GATEWAY_TIMEOUT
        {
            last_err = Some(format!("HTTP {status}: {body}"));
            continue;
        }

        return Err(format!("HTTP {status}: {body}"));
    }

    Err(format!(
        "All candidates failed: {}",
        last_err.unwrap_or_else(|| "no candidates".to_string())
    ))
}

/// Fetch live models for Codex OAuth/ChatGPT-backed accounts.
pub async fn fetch_codex_oauth_models(
    state: &Arc<AppState>,
    account_id: Option<&str>,
) -> Result<Vec<FetchedModel>, AppError> {
    let (account, tokens) = CodexOAuthManager::resolve_token(state, account_id).await?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(FETCH_TIMEOUT_SECS))
        .build()
        .map_err(|e| AppError::Config(format!("Failed to build HTTP client: {e}")))?;
    let response = client
        .get("https://chatgpt.com/backend-api/codex/models")
        .query(&[("client_version", env!("CARGO_PKG_VERSION"))])
        .bearer_auth(&tokens.access_token)
        .header("accept", "application/json")
        .header("originator", "cc-switch")
        .header("chatgpt-account-id", account.id.as_str())
        .send()
        .await
        .map_err(|err| AppError::Config(format!("Failed to fetch Codex OAuth models: {err}")))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Err(AppError::Unauthorized(format!(
            "Codex OAuth models rejected token: {}",
            truncate_body(body)
        )));
    }
    if !status.is_success() {
        return Err(AppError::Config(format!(
            "Failed to fetch Codex OAuth models with HTTP {status}: {}",
            truncate_body(body)
        )));
    }

    let raw: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| AppError::Config(format!("Failed to parse Codex models: {e}")))?;
    let mut models = extract_models(&raw, "@ai-sdk/openai-compatible");
    if models.is_empty() {
        models = extract_codex_models_flexible(&raw);
    }
    if models.is_empty() {
        return Err(AppError::Config(
            "Codex models response did not contain model ids".to_string(),
        ));
    }
    models.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(models)
}

/// Fetch live models for GitHub Copilot-backed accounts.
///
/// GitHub does not expose this as a regular OpenAI-compatible endpoint. We use
/// the Copilot API surface with the Auth Center token and parse the response
/// flexibly because the payload shape can differ across Copilot clients.
pub async fn fetch_github_copilot_models(
    state: &Arc<AppState>,
    account_id: Option<&str>,
) -> Result<Vec<FetchedModel>, AppError> {
    let (_account, tokens) = CopilotAuthManager::resolve_token(state, account_id).await?;
    let github_token = github_token_source(&tokens)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(FETCH_TIMEOUT_SECS))
        .user_agent(GITHUB_COPILOT_USER_AGENT)
        .build()
        .map_err(|e| AppError::Config(format!("Failed to build HTTP client: {e}")))?;
    let api_base = fetch_github_copilot_api_endpoint(&client, github_token).await?;
    let url = format!("{api_base}/models");
    let response = client
        .get(url)
        .bearer_auth(&tokens.access_token)
        .header("accept", "application/json")
        .header("content-type", "application/json")
        .header("copilot-integration-id", GITHUB_COPILOT_INTEGRATION_ID)
        .header("editor-version", GITHUB_COPILOT_EDITOR_VERSION)
        .header("editor-plugin-version", GITHUB_COPILOT_PLUGIN_VERSION)
        .header("user-agent", GITHUB_COPILOT_USER_AGENT)
        .header("x-github-api-version", GITHUB_COPILOT_API_VERSION)
        .send()
        .await
        .map_err(|err| AppError::Config(format!("Failed to fetch GitHub Copilot models: {err}")))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Err(AppError::Unauthorized(format!(
            "GitHub Copilot models rejected token: {}",
            truncate_body(body)
        )));
    }
    if !status.is_success() {
        return Err(AppError::Config(format!(
            "Failed to fetch GitHub Copilot models with HTTP {status}: {}",
            truncate_body(body)
        )));
    }

    let raw: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| AppError::Config(format!("Failed to parse Copilot models: {e}")))?;
    let mut models = extract_copilot_models(&raw);
    if models.is_empty() {
        models = extract_codex_models_flexible(&raw);
    }
    if models.is_empty() {
        return Err(AppError::Config(
            "Copilot models response did not contain model ids".to_string(),
        ));
    }
    models.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(models)
}

/// 构造「模型列表端点」的候选 URL 列表
///
/// 候选顺序：
/// 1. `models_url_override` 非空 → 只返回它
/// 2. baseURL 直接拼 `/v1/models`（若已有 `/v1` 结尾则拼 `/models`）
/// 3. 若 baseURL 命中 [`KNOWN_COMPAT_SUFFIXES`]，剥离后缀再拼 `/v1/models`
/// 4. 同上，但拼 `/models`（部分站点如 DeepSeek 官方只暴露 `/models`）
///
/// 结果已去重且保持首次出现顺序。
pub fn build_models_url_candidates(
    base_url: &str,
    npm: &str,
    is_full_url: bool,
    models_url_override: Option<&str>,
) -> Result<Vec<String>, String> {
    if let Some(raw) = models_url_override {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Ok(vec![trimmed.to_string()]);
        }
    }

    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("Base URL is empty".to_string());
    }

    if npm == "@ai-sdk/google" {
        return Ok(vec![build_google_models_url(trimmed)?]);
    }

    let mut candidates: Vec<String> = Vec::new();

    if is_full_url {
        if let Some(idx) = trimmed.find("/v1/") {
            candidates.push(format!("{}/v1/models", &trimmed[..idx]));
        } else if let Some(idx) = trimmed.rfind('/') {
            let root = &trimmed[..idx];
            if root.contains("://") && root.len() > root.find("://").unwrap() + 3 {
                candidates.push(format!("{root}/v1/models"));
            }
        }
        if candidates.is_empty() {
            return Err("Cannot derive models endpoint from full URL".to_string());
        }
        return Ok(candidates);
    }

    let primary = if trimmed.ends_with("/v1") {
        format!("{trimmed}/models")
    } else {
        format!("{trimmed}/v1/models")
    };
    candidates.push(primary);

    if let Some(stripped) = strip_compat_suffix(trimmed) {
        let root = stripped.trim_end_matches('/');
        if !root.is_empty() && root.contains("://") {
            candidates.push(format!("{root}/v1/models"));
            candidates.push(format!("{root}/models"));
        }
    }

    // 候选最多 3 条，线性去重即可，不值得上 HashSet。
    let mut unique: Vec<String> = Vec::with_capacity(candidates.len());
    for url in candidates {
        if !unique.iter().any(|u| u == &url) {
            unique.push(url);
        }
    }

    Ok(unique)
}

fn build_google_models_url(base_url: &str) -> Result<String, String> {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("Base URL is empty".to_string());
    }
    if trimmed.ends_with("/v1beta") {
        Ok(format!("{trimmed}/models"))
    } else if trimmed.ends_with("/v1") {
        Ok(format!("{trimmed}beta/models"))
    } else {
        Ok(format!("{trimmed}/v1beta/models"))
    }
}

/// 截断响应体到 [`ERROR_BODY_MAX_CHARS`] 字符，避免 HTML 404 页占用错误串。
fn truncate_body(body: String) -> String {
    if body.chars().count() <= ERROR_BODY_MAX_CHARS {
        body
    } else {
        let mut s: String = body.chars().take(ERROR_BODY_MAX_CHARS).collect();
        s.push('…');
        s
    }
}

/// 若 baseURL 以任一已知兼容子路径结尾，返回剥离后的剩余部分；否则 `None`。
///
/// 依赖 [`KNOWN_COMPAT_SUFFIXES`] 按长度降序排列，确保最长前缀优先命中
/// （否则 `/anthropic` 会提前匹配掉 `/api/anthropic` 的场景）。
fn strip_compat_suffix(base_url: &str) -> Option<&str> {
    for suffix in KNOWN_COMPAT_SUFFIXES {
        if base_url.ends_with(*suffix) {
            return Some(&base_url[..base_url.len() - suffix.len()]);
        }
    }
    None
}

fn extract_models(raw: &serde_json::Value, npm: &str) -> Vec<FetchedModel> {
    let mut models = Vec::new();
    if let Some(items) = raw.get("data").and_then(|v| v.as_array()) {
        for item in items {
            if let Some(model) = extract_model_entry(item, "id") {
                models.push(model);
            }
        }
    }
    if npm == "@ai-sdk/google" {
        if let Some(items) = raw.get("models").and_then(|v| v.as_array()) {
            for item in items {
                if let Some(model) = extract_model_entry(item, "name") {
                    models.push(model);
                }
            }
        }
    }
    models
}

fn extract_model_entry(item: &serde_json::Value, primary_key: &str) -> Option<FetchedModel> {
    let id = item
        .get("baseModelId")
        .and_then(|v| v.as_str())
        .map(normalize_model_id)
        .or_else(|| {
            item.get(primary_key)
                .and_then(|v| v.as_str())
                .map(normalize_model_id)
        })
        .or_else(|| {
            item.get("name")
                .and_then(|v| v.as_str())
                .map(normalize_model_id)
        })?
        .to_string();
    let owned_by = item
        .get("owned_by")
        .or_else(|| item.get("ownedBy"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Some(FetchedModel { id, owned_by })
}

fn extract_copilot_models(raw: &serde_json::Value) -> Vec<FetchedModel> {
    let Some(items) = raw.get("data").and_then(|value| value.as_array()) else {
        return Vec::new();
    };

    items
        .iter()
        .filter(|item| {
            item.get("model_picker_enabled")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true)
        })
        .filter_map(|item| {
            let id = item
                .get("id")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())?;
            let owned_by = item
                .get("vendor")
                .or_else(|| item.get("owned_by"))
                .or_else(|| item.get("ownedBy"))
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string);
            Some(FetchedModel {
                id: id.to_string(),
                owned_by,
            })
        })
        .collect()
}

fn extract_codex_models_flexible(raw: &serde_json::Value) -> Vec<FetchedModel> {
    let mut models = Vec::new();
    collect_model_ids(raw, &mut models);
    models.sort_by(|a, b| a.id.cmp(&b.id));
    models.dedup_by(|a, b| a.id == b.id);
    models
}

fn collect_model_ids(value: &serde_json::Value, models: &mut Vec<FetchedModel>) {
    match value {
        serde_json::Value::Object(map) => {
            for key in ["id", "slug", "model", "name"] {
                if let Some(id) = map
                    .get(key)
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                {
                    if looks_like_model_id(id) {
                        models.push(FetchedModel {
                            id: normalize_model_id(id),
                            owned_by: map
                                .get("owned_by")
                                .or_else(|| map.get("ownedBy"))
                                .and_then(|v| v.as_str())
                                .map(ToString::to_string),
                        });
                    }
                }
            }
            for child in map.values() {
                collect_model_ids(child, models);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_model_ids(item, models);
            }
        }
        _ => {}
    }
}

fn looks_like_model_id(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("gpt")
        || lower.contains("o3")
        || lower.contains("o4")
        || lower.contains("o5")
        || lower.contains("claude")
        || lower.contains("gemini")
        || lower.contains("codex")
}

fn normalize_model_id(id: &str) -> String {
    id.strip_prefix("models/").unwrap_or(id).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candidates_plain_root() {
        let c = build_models_url_candidates(
            "https://api.siliconflow.cn",
            "@ai-sdk/openai-compatible",
            false,
            None,
        )
        .unwrap();
        assert_eq!(c, vec!["https://api.siliconflow.cn/v1/models"]);
    }

    #[test]
    fn test_candidates_trailing_slash() {
        let c = build_models_url_candidates(
            "https://api.example.com/",
            "@ai-sdk/openai-compatible",
            false,
            None,
        )
        .unwrap();
        assert_eq!(c, vec!["https://api.example.com/v1/models"]);
    }

    #[test]
    fn test_candidates_with_v1() {
        let c = build_models_url_candidates(
            "https://api.example.com/v1",
            "@ai-sdk/openai-compatible",
            false,
            None,
        )
        .unwrap();
        assert_eq!(c, vec!["https://api.example.com/v1/models"]);
    }

    #[test]
    fn test_candidates_full_url() {
        let c = build_models_url_candidates(
            "https://proxy.example.com/v1/chat/completions",
            "@ai-sdk/openai-compatible",
            true,
            None,
        )
        .unwrap();
        assert_eq!(c, vec!["https://proxy.example.com/v1/models"]);
    }

    #[test]
    fn test_candidates_empty() {
        assert!(build_models_url_candidates("", "@ai-sdk/openai-compatible", false, None).is_err());
    }

    #[test]
    fn test_candidates_override_returns_single() {
        let c = build_models_url_candidates(
            "https://api.deepseek.com/anthropic",
            "@ai-sdk/openai-compatible",
            false,
            Some("https://api.deepseek.com/models"),
        )
        .unwrap();
        assert_eq!(c, vec!["https://api.deepseek.com/models"]);
    }

    #[test]
    fn test_candidates_override_empty_falls_through() {
        let c = build_models_url_candidates(
            "https://api.siliconflow.cn",
            "@ai-sdk/openai-compatible",
            false,
            Some("   "),
        )
        .unwrap();
        assert_eq!(c, vec!["https://api.siliconflow.cn/v1/models"]);
    }

    #[test]
    fn test_candidates_deepseek_strip_anthropic() {
        let c = build_models_url_candidates(
            "https://api.deepseek.com/anthropic",
            "@ai-sdk/openai-compatible",
            false,
            None,
        )
        .unwrap();
        assert_eq!(
            c,
            vec![
                "https://api.deepseek.com/anthropic/v1/models",
                "https://api.deepseek.com/v1/models",
                "https://api.deepseek.com/models",
            ]
        );
    }

    #[test]
    fn test_candidates_zhipu_strip_api_anthropic() {
        let c = build_models_url_candidates(
            "https://open.bigmodel.cn/api/anthropic",
            "@ai-sdk/openai-compatible",
            false,
            None,
        )
        .unwrap();
        assert_eq!(
            c,
            vec![
                "https://open.bigmodel.cn/api/anthropic/v1/models",
                "https://open.bigmodel.cn/v1/models",
                "https://open.bigmodel.cn/models",
            ]
        );
    }

    #[test]
    fn test_candidates_bailian_strip_apps_anthropic() {
        let c = build_models_url_candidates(
            "https://dashscope.aliyuncs.com/apps/anthropic",
            "@ai-sdk/openai-compatible",
            false,
            None,
        )
        .unwrap();
        assert_eq!(
            c,
            vec![
                "https://dashscope.aliyuncs.com/apps/anthropic/v1/models",
                "https://dashscope.aliyuncs.com/v1/models",
                "https://dashscope.aliyuncs.com/models",
            ]
        );
    }

    #[test]
    fn test_candidates_stepfun_strip_step_plan() {
        let c = build_models_url_candidates(
            "https://api.stepfun.com/step_plan",
            "@ai-sdk/openai-compatible",
            false,
            None,
        )
        .unwrap();
        assert_eq!(
            c,
            vec![
                "https://api.stepfun.com/step_plan/v1/models",
                "https://api.stepfun.com/v1/models",
                "https://api.stepfun.com/models",
            ]
        );
    }

    #[test]
    fn test_candidates_doubao_strip_api_coding() {
        let c = build_models_url_candidates(
            "https://ark.cn-beijing.volces.com/api/coding",
            "@ai-sdk/openai-compatible",
            false,
            None,
        )
        .unwrap();
        assert_eq!(
            c,
            vec![
                "https://ark.cn-beijing.volces.com/api/coding/v1/models",
                "https://ark.cn-beijing.volces.com/v1/models",
                "https://ark.cn-beijing.volces.com/models",
            ]
        );
    }

    #[test]
    fn test_candidates_rightcode_strip_claude() {
        let c = build_models_url_candidates(
            "https://www.right.codes/claude",
            "@ai-sdk/openai-compatible",
            false,
            None,
        )
        .unwrap();
        assert_eq!(
            c,
            vec![
                "https://www.right.codes/claude/v1/models",
                "https://www.right.codes/v1/models",
                "https://www.right.codes/models",
            ]
        );
    }

    #[test]
    fn test_candidates_longer_suffix_wins() {
        // baseURL 以 /api/anthropic 结尾时，应剥离整个 /api/anthropic，
        // 而不是只剥离 /anthropic（那样会得到残缺的 https://.../api 根）。
        let c = build_models_url_candidates(
            "https://api.z.ai/api/anthropic",
            "@ai-sdk/openai-compatible",
            false,
            None,
        )
        .unwrap();
        assert_eq!(
            c,
            vec![
                "https://api.z.ai/api/anthropic/v1/models",
                "https://api.z.ai/v1/models",
                "https://api.z.ai/models",
            ]
        );
    }

    #[test]
    fn test_candidates_no_suffix_no_strip() {
        let c = build_models_url_candidates(
            "https://openrouter.ai/api",
            "@ai-sdk/openai-compatible",
            false,
            None,
        )
        .unwrap();
        assert_eq!(c, vec!["https://openrouter.ai/api/v1/models"]);
    }

    #[test]
    fn test_candidates_deduplicate() {
        // 虚构 case：baseURL 就是 "scheme://host"，剥不出子路径，应只有一个候选。
        let c = build_models_url_candidates(
            "https://host.example.com",
            "@ai-sdk/openai-compatible",
            false,
            None,
        )
        .unwrap();
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn test_candidates_google_uses_generative_language_models_endpoint() {
        let c = build_models_url_candidates(
            "https://generativelanguage.googleapis.com",
            "@ai-sdk/google",
            false,
            None,
        )
        .unwrap();
        assert_eq!(
            c,
            vec!["https://generativelanguage.googleapis.com/v1beta/models"]
        );
    }

    #[test]
    fn test_candidates_google_v1_upgrades_to_v1beta_models_endpoint() {
        let c = build_models_url_candidates(
            "https://generativelanguage.googleapis.com/v1",
            "@ai-sdk/google",
            false,
            None,
        )
        .unwrap();
        assert_eq!(
            c,
            vec!["https://generativelanguage.googleapis.com/v1beta/models"]
        );
    }

    #[test]
    fn test_parse_response() {
        let json = r#"{"object":"list","data":[{"id":"gpt-4","object":"model","owned_by":"openai"},{"id":"claude-3-sonnet","object":"model","owned_by":"anthropic"}]}"#;
        let raw: serde_json::Value = serde_json::from_str(json).unwrap();
        let data = extract_models(&raw, "@ai-sdk/openai-compatible");
        assert_eq!(data.len(), 2);
        assert_eq!(data[0].id, "gpt-4");
        assert_eq!(data[0].owned_by.as_deref(), Some("openai"));
        assert_eq!(data[1].id, "claude-3-sonnet");
    }

    #[test]
    fn test_parse_response_no_owned_by() {
        let json = r#"{"object":"list","data":[{"id":"my-model","object":"model"}]}"#;
        let raw: serde_json::Value = serde_json::from_str(json).unwrap();
        let data = extract_models(&raw, "@ai-sdk/openai-compatible");
        assert_eq!(data[0].id, "my-model");
        assert!(data[0].owned_by.is_none());
    }

    #[test]
    fn test_parse_response_empty_data() {
        let json = r#"{"object":"list","data":[]}"#;
        let raw: serde_json::Value = serde_json::from_str(json).unwrap();
        assert!(extract_models(&raw, "@ai-sdk/openai-compatible").is_empty());
    }

    #[test]
    fn test_parse_google_response_normalizes_model_names() {
        let json = r#"{"models":[{"name":"models/gemini-3-pro-preview"},{"name":"gemini-3-flash-preview"}]}"#;
        let raw: serde_json::Value = serde_json::from_str(json).unwrap();
        let data = extract_models(&raw, "@ai-sdk/google");
        assert_eq!(data.len(), 2);
        assert_eq!(data[0].id, "gemini-3-pro-preview");
        assert_eq!(data[1].id, "gemini-3-flash-preview");
    }

    #[test]
    fn test_flexible_managed_model_parser_accepts_claude_and_gemini_ids() {
        let json = r#"{
            "models": [
                { "slug": "claude-sonnet-4.6", "ownedBy": "github-copilot" },
                { "name": "gemini-3-pro-preview", "owned_by": "google" }
            ]
        }"#;
        let raw: serde_json::Value = serde_json::from_str(json).unwrap();
        let data = extract_codex_models_flexible(&raw);

        assert_eq!(data.len(), 2);
        assert!(data.iter().any(|model| model.id == "claude-sonnet-4.6"));
        assert!(data.iter().any(|model| model.id == "gemini-3-pro-preview"));
    }

    #[tokio::test]
    async fn test_fetch_models_continues_after_retryable_candidate_failure() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind test upstream");
        let addr = listener.local_addr().expect("local addr");
        let app = axum::Router::new()
            .route(
                "/anthropic/v1/models",
                axum::routing::get(|| async {
                    axum::response::Response::builder()
                        .status(axum::http::StatusCode::BAD_GATEWAY)
                        .body(axum::body::Body::from("temporary upstream error"))
                        .expect("response")
                }),
            )
            .route(
                "/v1/models",
                axum::routing::get(|| async {
                    axum::Json(serde_json::json!({
                        "data": [
                            { "id": "fallback-model", "owned_by": "test" }
                        ]
                    }))
                }),
            );
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let models = fetch_models(
            &format!("http://{addr}/anthropic"),
            "sk-test",
            Some("@ai-sdk/openai-compatible"),
            false,
            None,
        )
        .await
        .expect("fallback candidate should succeed");

        handle.abort();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "fallback-model");
    }
}
