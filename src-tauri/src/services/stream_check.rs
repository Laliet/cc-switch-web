use std::time::Instant;

use futures::StreamExt;
use hmac::{Hmac, Mac};
use regex::Regex;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, CONTENT_TYPE, HOST},
    Client, RequestBuilder,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use url::Url;

use crate::{
    app_config::AppType,
    database::{StreamCheckLogFilters, StreamCheckLogRecord},
    error::AppError,
    provider::Provider,
    store::AppState,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Operational,
    Degraded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StreamCheckConfig {
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub degraded_threshold_ms: u64,
    pub claude_model: String,
    pub codex_model: String,
    pub gemini_model: String,
    #[serde(default = "default_test_prompt")]
    pub test_prompt: String,
}

impl Default for StreamCheckConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 45,
            max_retries: 2,
            degraded_threshold_ms: 6000,
            claude_model: "claude-haiku-4-5-20251001".to_string(),
            codex_model: "gpt-5.4@low".to_string(),
            gemini_model: "gemini-3-flash-preview".to_string(),
            test_prompt: default_test_prompt(),
        }
    }
}

fn default_test_prompt() -> String {
    "Who are you?".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamCheckResult {
    pub status: HealthStatus,
    pub success: bool,
    pub message: String,
    pub response_time_ms: Option<u64>,
    pub http_status: Option<u16>,
    pub model_used: String,
    pub tested_at: i64,
    pub retry_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_category: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamCheckLog {
    pub id: i64,
    pub provider_id: String,
    pub provider_name: String,
    pub app_type: String,
    pub status: String,
    pub success: bool,
    pub message: String,
    pub response_time_ms: Option<i64>,
    pub http_status: Option<i64>,
    pub model_used: String,
    pub retry_count: i64,
    pub error_category: Option<String>,
    pub tested_at: i64,
}

pub struct StreamCheckService;

impl StreamCheckService {
    pub fn ensure_app_supported(app_type: &AppType) -> Result<(), AppError> {
        match app_type {
            AppType::OpenClaw => Err(AppError::localized(
                "stream_check.openclaw.unsupported",
                "OpenClaw 第一阶段不支持直接 Stream Check，请检测其上游 Provider。",
                "OpenClaw phase one does not support direct Stream Check; test its upstream provider.",
            )),
            AppType::Omo | AppType::OmoSlim => Err(AppError::localized(
                "stream_check.omo.unsupported",
                "OMO profile 不支持直接 Stream Check，请检测底层 OpenCode Provider。",
                "OMO profiles do not support direct Stream Check; test the underlying OpenCode provider.",
            )),
            _ => Ok(()),
        }
    }

    pub fn get_config(state: &AppState) -> Result<StreamCheckConfig, AppError> {
        state.db.get_stream_check_config()
    }

    pub fn save_config(state: &AppState, config: &StreamCheckConfig) -> Result<(), AppError> {
        validate_config(config)?;
        state.db.save_stream_check_config(config)
    }

    pub fn record_result(
        state: &AppState,
        app_type: &AppType,
        provider: &Provider,
        result: &StreamCheckResult,
    ) -> Result<StreamCheckLog, AppError> {
        let record = StreamCheckLogRecord {
            id: 0,
            provider_id: provider.id.clone(),
            provider_name: provider.name.clone(),
            app_type: app_type.as_str().to_string(),
            status: status_name(&result.status).to_string(),
            success: result.success,
            message: result.message.clone(),
            response_time_ms: result.response_time_ms.map(|value| value as i64),
            http_status: result.http_status.map(|value| value as i64),
            model_used: result.model_used.clone(),
            retry_count: result.retry_count as i64,
            error_category: result.error_category.clone(),
            tested_at: result.tested_at,
        };
        state.db.insert_stream_check_log(&record).map(Into::into)
    }

    pub fn list_logs(
        state: &AppState,
        filters: StreamCheckLogFilters,
    ) -> Result<Vec<StreamCheckLog>, AppError> {
        state
            .db
            .list_stream_check_logs(&filters)
            .map(|logs| logs.into_iter().map(Into::into).collect())
    }

    pub fn latest_logs(
        state: &AppState,
        app_type: Option<&str>,
    ) -> Result<Vec<StreamCheckLog>, AppError> {
        state
            .db
            .list_latest_stream_check_logs(app_type)
            .map(|logs| logs.into_iter().map(Into::into).collect())
    }

    pub async fn check_with_retry(
        app_type: &AppType,
        provider: &Provider,
        config: &StreamCheckConfig,
    ) -> StreamCheckResult {
        let mut last = None;
        for attempt in 0..=config.max_retries {
            let mut result = Self::check_once(app_type, provider, config).await;
            result.retry_count = attempt;
            if result.success || !should_retry(&result) || attempt == config.max_retries {
                return result;
            }
            last = Some(result);
        }
        last.unwrap_or_else(|| failed("", "Check failed", None, None, 0))
    }

    async fn check_once(
        app_type: &AppType,
        provider: &Provider,
        config: &StreamCheckConfig,
    ) -> StreamCheckResult {
        let request = match build_request(app_type, provider, config) {
            Ok(request) => request,
            Err(_) => {
                return failed(
                    "",
                    "Provider configuration is invalid",
                    None,
                    Some("configurationError".to_string()),
                    0,
                )
            }
        };
        let model = request.model.clone();
        let start = Instant::now();
        let client = match Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
        {
            Ok(client) => client,
            Err(err) => {
                return failed(
                    &model,
                    &err.to_string(),
                    None,
                    Some("clientError".to_string()),
                    0,
                )
            }
        };

        let response = match client
            .post(&request.url)
            .headers(request.headers.clone())
            .send_probe_body(&request)
            .send()
            .await
        {
            Ok(response) => response,
            Err(err) => {
                let (message, category) = if err.is_timeout() {
                    ("Stream Check request timed out", "timeout")
                } else if err.is_connect() {
                    (
                        "Stream Check could not connect to the upstream",
                        "connectionFailed",
                    )
                } else {
                    ("Stream Check network request failed", "networkError")
                };
                let mut result = failed(&model, message, None, Some(category.to_string()), 0);
                result.response_time_ms = Some(start.elapsed().as_millis() as u64);
                return result;
            }
        };

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let category = classify_error(status.as_u16(), &body);
            let public_body = redact_probe_secrets(&body, &request);
            let message = if public_body.trim().is_empty() {
                format!("HTTP {}", status.as_u16())
            } else {
                format!("HTTP {}: {}", status.as_u16(), truncate(&public_body))
            };
            let mut result = failed(&model, &message, Some(status.as_u16()), category, 0);
            result.response_time_ms = Some(start.elapsed().as_millis() as u64);
            return result;
        }

        let mut stream = response.bytes_stream();
        while let Some(next) = stream.next().await {
            match next {
                Ok(bytes) if !bytes.is_empty() => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    let status = if elapsed > config.degraded_threshold_ms {
                        HealthStatus::Degraded
                    } else {
                        HealthStatus::Operational
                    };
                    return StreamCheckResult {
                        status,
                        success: true,
                        message: "Stream responded".to_string(),
                        response_time_ms: Some(elapsed),
                        http_status: Some(200),
                        model_used: model,
                        tested_at: chrono::Utc::now().timestamp(),
                        retry_count: 0,
                        error_category: None,
                    };
                }
                Ok(_) => continue,
                Err(err) => {
                    let mut result = failed(
                        &model,
                        if err.is_timeout() {
                            "Stream Check response timed out"
                        } else {
                            "Stream Check response stream failed"
                        },
                        None,
                        Some(if err.is_timeout() {
                            "timeout".to_string()
                        } else {
                            "streamError".to_string()
                        }),
                        0,
                    );
                    result.response_time_ms = Some(start.elapsed().as_millis() as u64);
                    return result;
                }
            }
        }

        let mut result = failed(
            &model,
            "Stream ended without data",
            Some(200),
            Some("emptyStream".to_string()),
            0,
        );
        result.response_time_ms = Some(start.elapsed().as_millis() as u64);
        result
    }
}

fn validate_config(config: &StreamCheckConfig) -> Result<(), AppError> {
    if !(1..=300).contains(&config.timeout_secs) {
        return Err(AppError::InvalidInput(
            "Stream Check timeoutSecs must be between 1 and 300".to_string(),
        ));
    }
    if config.max_retries > 10 {
        return Err(AppError::InvalidInput(
            "Stream Check maxRetries must be between 0 and 10".to_string(),
        ));
    }
    if config.degraded_threshold_ms == 0
        || config.degraded_threshold_ms > config.timeout_secs.saturating_mul(1000)
    {
        return Err(AppError::InvalidInput(
            "Stream Check degradedThresholdMs must be positive and no greater than timeoutSecs"
                .to_string(),
        ));
    }
    for (name, value) in [
        ("claudeModel", config.claude_model.as_str()),
        ("codexModel", config.codex_model.as_str()),
        ("geminiModel", config.gemini_model.as_str()),
    ] {
        if value.trim().is_empty() || value.len() > 256 {
            return Err(AppError::InvalidInput(format!(
                "Stream Check {name} must contain 1 to 256 bytes"
            )));
        }
    }
    if config.test_prompt.trim().is_empty() || config.test_prompt.len() > 4096 {
        return Err(AppError::InvalidInput(
            "Stream Check testPrompt must contain 1 to 4096 bytes".to_string(),
        ));
    }
    Ok(())
}

fn status_name(status: &HealthStatus) -> &'static str {
    match status {
        HealthStatus::Operational => "operational",
        HealthStatus::Degraded => "degraded",
        HealthStatus::Failed => "failed",
    }
}

impl From<StreamCheckLogRecord> for StreamCheckLog {
    fn from(record: StreamCheckLogRecord) -> Self {
        Self {
            id: record.id,
            provider_id: record.provider_id,
            provider_name: record.provider_name,
            app_type: record.app_type,
            status: record.status,
            success: record.success,
            message: record.message,
            response_time_ms: record.response_time_ms,
            http_status: record.http_status,
            model_used: record.model_used,
            retry_count: record.retry_count,
            error_category: record.error_category,
            tested_at: record.tested_at,
        }
    }
}

#[derive(Debug)]
struct ProbeRequest {
    url: String,
    headers: HeaderMap,
    body: Value,
    body_bytes: Option<Vec<u8>>,
    model: String,
}

fn redact_probe_secrets(message: &str, request: &ProbeRequest) -> String {
    let mut secrets = Vec::new();
    for (name, value) in &request.headers {
        if !is_sensitive_name(name.as_str()) {
            continue;
        }
        let Ok(value) = value.to_str() else {
            continue;
        };
        secrets.push(value.to_string());
        if let Some((_, credential)) = value.split_once(' ') {
            secrets.push(credential.to_string());
        }
        for marker in ["Credential=", "Signature="] {
            if let Some(value) = value.split(marker).nth(1) {
                let value = value
                    .split([',', '/', ' '])
                    .next()
                    .unwrap_or_default()
                    .trim();
                if !value.is_empty() {
                    secrets.push(value.to_string());
                }
            }
        }
    }
    collect_sensitive_json_values(&request.body, &mut secrets);
    if let Ok(url) = Url::parse(&request.url) {
        if !url.username().is_empty() {
            secrets.push(url.username().to_string());
        }
        if let Some(password) = url.password() {
            secrets.push(password.to_string());
        }
        for (key, value) in url.query_pairs() {
            if is_sensitive_name(&key) && !value.is_empty() {
                secrets.push(value.into_owned());
            }
        }
    }

    secrets.retain(|value| !value.is_empty());
    secrets.sort_by_key(|value| std::cmp::Reverse(value.len()));
    secrets.dedup();
    secrets
        .into_iter()
        .fold(message.to_string(), |value, secret| {
            value.replace(&secret, "[redacted]")
        })
}

fn collect_sensitive_json_values(value: &Value, secrets: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if is_sensitive_name(key) {
                    match value {
                        Value::String(value) if !value.is_empty() => secrets.push(value.clone()),
                        Value::Number(value) => secrets.push(value.to_string()),
                        _ => {}
                    }
                }
                collect_sensitive_json_values(value, secrets);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_sensitive_json_values(value, secrets);
            }
        }
        _ => {}
    }
}

fn is_sensitive_name(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase().replace(['-', '_'], "");
    normalized == "authorization"
        || normalized == "proxyauthorization"
        || normalized == "cookie"
        || normalized == "setcookie"
        || normalized.contains("apikey")
        || normalized.contains("accesstoken")
        || normalized.contains("refreshtoken")
        || normalized.contains("sessiontoken")
        || normalized.contains("secret")
        || normalized.contains("password")
}

trait ProbeRequestBuilderExt {
    fn send_probe_body(self, request: &ProbeRequest) -> RequestBuilder;
}

impl ProbeRequestBuilderExt for RequestBuilder {
    fn send_probe_body(self, request: &ProbeRequest) -> RequestBuilder {
        match &request.body_bytes {
            Some(bytes) => self.body(bytes.clone()),
            None => self.json(&request.body),
        }
    }
}

fn build_request(
    app_type: &AppType,
    provider: &Provider,
    config: &StreamCheckConfig,
) -> Result<ProbeRequest, AppError> {
    match app_type {
        AppType::Claude | AppType::ClaudeDesktop => build_claude_request(provider, config),
        AppType::Codex => build_openai_chat_request(
            codex_base_url(provider)?,
            codex_api_key(provider)?,
            config.codex_model.clone(),
            &config.test_prompt,
            Map::new(),
            None,
        ),
        AppType::Gemini => build_gemini_request(provider, config),
        AppType::Opencode => build_opencode_request(provider, config),
        AppType::OpenClaw => Err(AppError::Message(
            "OpenClaw does not expose a direct stream endpoint; check its configured upstream provider"
                .to_string(),
        )),
        AppType::Omo | AppType::OmoSlim => Err(AppError::Message(
            "OMO profiles do not expose a direct stream endpoint; test the underlying OpenCode provider"
                .to_string(),
        )),
    }
}

fn build_claude_request(
    provider: &Provider,
    config: &StreamCheckConfig,
) -> Result<ProbeRequest, AppError> {
    let env = provider
        .settings_config
        .get("env")
        .and_then(Value::as_object)
        .ok_or_else(|| AppError::Config("Missing env".to_string()))?;
    let base = env
        .get("ANTHROPIC_BASE_URL")
        .and_then(Value::as_str)
        .unwrap_or("https://api.anthropic.com")
        .trim_end_matches('/');
    let api_key = env
        .get("ANTHROPIC_AUTH_TOKEN")
        .or_else(|| env.get("ANTHROPIC_API_KEY"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if api_key.trim().is_empty() {
        return Err(AppError::Config("Missing Claude API key".to_string()));
    }
    let model = config.claude_model.clone();
    let mut headers = json_headers();
    insert_header(&mut headers, "x-api-key", api_key);
    insert_header(&mut headers, "anthropic-version", "2023-06-01");
    Ok(ProbeRequest {
        url: format!("{base}/v1/messages"),
        headers,
        model: model.clone(),
        body: json!({
            "model": model,
            "max_tokens": 16,
            "stream": true,
            "messages": [{ "role": "user", "content": config.test_prompt }]
        }),
        body_bytes: None,
    })
}

fn build_gemini_request(
    provider: &Provider,
    config: &StreamCheckConfig,
) -> Result<ProbeRequest, AppError> {
    let env = crate::gemini_config::json_to_env(&provider.settings_config)?;
    let base = env
        .get("GOOGLE_GEMINI_BASE_URL")
        .map(String::as_str)
        .unwrap_or("https://generativelanguage.googleapis.com")
        .trim_end_matches('/');
    let api_key = env
        .get("GEMINI_API_KEY")
        .map(String::as_str)
        .unwrap_or_default();
    if api_key.trim().is_empty() {
        return Err(AppError::Config("Missing Gemini API key".to_string()));
    }
    let model = config.gemini_model.clone();
    let mut headers = json_headers();
    insert_header(&mut headers, "x-goog-api-key", api_key);
    let base = if base.ends_with("/v1beta") {
        base.to_string()
    } else {
        format!("{base}/v1beta")
    };
    Ok(ProbeRequest {
        url: format!("{base}/models/{model}:streamGenerateContent"),
        headers,
        model,
        body: json!({
            "contents": [{ "role": "user", "parts": [{ "text": config.test_prompt }] }],
            "generationConfig": { "maxOutputTokens": 16 }
        }),
        body_bytes: None,
    })
}

fn build_opencode_request(
    provider: &Provider,
    config: &StreamCheckConfig,
) -> Result<ProbeRequest, AppError> {
    let npm = provider
        .settings_config
        .get("npm")
        .and_then(Value::as_str)
        .unwrap_or("@ai-sdk/openai-compatible");
    let options = provider
        .settings_config
        .get("options")
        .and_then(Value::as_object)
        .ok_or_else(|| AppError::Config("Missing OpenCode options".to_string()))?;
    let (model, model_options, model_variant) = first_opencode_model(provider)
        .unwrap_or_else(|| (config.codex_model.clone(), Map::new(), None));
    if npm == "@ai-sdk/amazon-bedrock" {
        return build_bedrock_request(options, model, &config.test_prompt);
    }
    let api_key = options
        .get("apiKey")
        .or_else(|| options.get("api_key"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if api_key.trim().is_empty() {
        return Err(AppError::Config("Missing OpenCode API key".to_string()));
    }

    let base = options
        .get("baseURL")
        .or_else(|| options.get("baseUrl"))
        .or_else(|| options.get("base_url"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| default_opencode_base_url(npm))
        .trim_end_matches('/');

    let mut headers = json_headers();
    insert_header(&mut headers, "authorization", &format!("Bearer {api_key}"));
    if let Some(extra) = options.get("headers").and_then(Value::as_object) {
        for (key, value) in extra {
            if let Some(value) = value.as_str() {
                insert_header(&mut headers, key, value);
            }
        }
    }

    match npm {
        "@ai-sdk/anthropic" => {
            headers.remove("authorization");
            insert_header(&mut headers, "x-api-key", api_key);
            insert_header(&mut headers, "anthropic-version", "2023-06-01");
            let mut body = json!({
                "model": model.clone(),
                "max_tokens": 16,
                "stream": true,
                "messages": [{ "role": "user", "content": config.test_prompt }]
            });
            apply_model_options(&mut body, &model_options);
            Ok(ProbeRequest {
                url: format!("{base}/v1/messages"),
                headers,
                model,
                body,
                body_bytes: None,
            })
        }
        "@ai-sdk/google" => {
            let base = if base.ends_with("/v1beta") {
                base.to_string()
            } else {
                format!("{base}/v1beta")
            };
            headers.remove("authorization");
            insert_header(&mut headers, "x-goog-api-key", api_key);
            let mut body = json!({
                "contents": [{ "role": "user", "parts": [{ "text": config.test_prompt }] }],
                "generationConfig": { "maxOutputTokens": 16 }
            });
            apply_model_options(&mut body, &model_options);
            Ok(ProbeRequest {
                url: format!("{base}/models/{model}:streamGenerateContent"),
                headers,
                model,
                body,
                body_bytes: None,
            })
        }
        "@ai-sdk/openai" => {
            let mut body = json!({
                "model": model.clone(),
                "input": config.test_prompt,
                "stream": true,
                "max_output_tokens": 16
            });
            apply_model_options(&mut body, &model_options);
            apply_model_variant(&mut body, model_variant.as_deref());
            Ok(ProbeRequest {
                url: format!("{}/responses", normalize_v1(base)),
                headers,
                model,
                body,
                body_bytes: None,
            })
        }
        _ => build_openai_chat_request(
            base.to_string(),
            api_key.to_string(),
            model,
            &config.test_prompt,
            model_options,
            model_variant,
        ),
    }
}

fn build_openai_chat_request(
    base_url: String,
    api_key: String,
    model: String,
    prompt: &str,
    model_options: Map<String, Value>,
    model_variant: Option<String>,
) -> Result<ProbeRequest, AppError> {
    if api_key.trim().is_empty() {
        return Err(AppError::Config("Missing API key".to_string()));
    }
    let mut headers = json_headers();
    insert_header(&mut headers, "authorization", &format!("Bearer {api_key}"));
    let mut body = json!({
        "model": model.clone(),
        "stream": true,
        "max_tokens": 16,
        "messages": [{ "role": "user", "content": prompt }]
    });
    apply_model_options(&mut body, &model_options);
    apply_model_variant(&mut body, model_variant.as_deref());
    Ok(ProbeRequest {
        url: format!("{}/chat/completions", normalize_v1(&base_url)),
        headers,
        model,
        body,
        body_bytes: None,
    })
}

fn build_bedrock_request(
    options: &Map<String, Value>,
    model: String,
    prompt: &str,
) -> Result<ProbeRequest, AppError> {
    let region = option_string(options, &["region"])
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::Config("Missing Bedrock region".to_string()))?;
    let access_key_id = option_string(options, &["accessKeyId", "access_key_id"])
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::Config("Missing Bedrock accessKeyId".to_string()))?;
    let secret_access_key = option_string(options, &["secretAccessKey", "secret_access_key"])
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::Config("Missing Bedrock secretAccessKey".to_string()))?;
    let session_token = option_string(options, &["sessionToken", "session_token"])
        .filter(|value| !value.trim().is_empty());
    let base = option_string(options, &["baseURL", "baseUrl", "base_url"])
        .unwrap_or_else(|| format!("https://bedrock-runtime.{region}.amazonaws.com"));
    let model_path = url::form_urlencoded::byte_serialize(model.as_bytes()).collect::<String>();
    let url = format!(
        "{}/model/{}/converse-stream",
        base.trim_end_matches('/'),
        model_path
    );
    let body = json!({
        "messages": [{
            "role": "user",
            "content": [{ "text": prompt }]
        }],
        "inferenceConfig": {
            "maxTokens": 16
        }
    });
    let body_bytes = serde_json::to_vec(&body).map_err(|err| AppError::Config(err.to_string()))?;
    let mut headers = json_headers();
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.amazon.eventstream"),
    );
    sign_bedrock_request(
        BedrockSigningInput {
            url: &url,
            region: &region,
            access_key_id: &access_key_id,
            secret_access_key: &secret_access_key,
            session_token: session_token.as_deref(),
            body: &body_bytes,
            now: chrono::Utc::now(),
        },
        &mut headers,
    )?;

    Ok(ProbeRequest {
        url,
        headers,
        model,
        body,
        body_bytes: Some(body_bytes),
    })
}

fn option_string(options: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| options.get(*key).and_then(Value::as_str))
        .map(str::to_string)
}

struct BedrockSigningInput<'a> {
    url: &'a str,
    region: &'a str,
    access_key_id: &'a str,
    secret_access_key: &'a str,
    session_token: Option<&'a str>,
    body: &'a [u8],
    now: chrono::DateTime<chrono::Utc>,
}

fn sign_bedrock_request(
    input: BedrockSigningInput<'_>,
    headers: &mut HeaderMap,
) -> Result<(), AppError> {
    let parsed = Url::parse(input.url).map_err(|err| AppError::Config(err.to_string()))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| AppError::Config("Missing Bedrock request host".to_string()))?;
    let amz_date = input.now.format("%Y%m%dT%H%M%SZ").to_string();
    let date_stamp = input.now.format("%Y%m%d").to_string();
    let payload_hash = hex::encode(Sha256::digest(input.body));

    insert_header(headers, HOST.as_str(), host);
    insert_header(headers, "x-amz-date", &amz_date);
    insert_header(headers, "x-amz-content-sha256", &payload_hash);
    if let Some(token) = input.session_token {
        insert_header(headers, "x-amz-security-token", token);
    }

    let canonical_uri = parsed.path();
    let canonical_query = parsed.query().unwrap_or_default();
    let mut header_pairs = canonical_header_pairs(headers)?;
    header_pairs.sort_by(|a, b| a.0.cmp(&b.0));
    let canonical_headers = header_pairs
        .iter()
        .map(|(key, value)| format!("{key}:{value}\n"))
        .collect::<String>();
    let signed_headers = header_pairs
        .iter()
        .map(|(key, _)| key.as_str())
        .collect::<Vec<_>>()
        .join(";");
    let canonical_request = format!(
        "POST\n{canonical_uri}\n{canonical_query}\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
    );
    let credential_scope = format!("{date_stamp}/{}/bedrock/aws4_request", input.region);
    let canonical_request_hash = hex::encode(Sha256::digest(canonical_request.as_bytes()));
    let string_to_sign =
        format!("AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{canonical_request_hash}");
    let signing_key = bedrock_signing_key(input.secret_access_key, &date_stamp, input.region)?;
    let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes())?);
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}",
        input.access_key_id
    );
    insert_header(headers, "authorization", &authorization);
    Ok(())
}

fn canonical_header_pairs(headers: &HeaderMap) -> Result<Vec<(String, String)>, AppError> {
    headers
        .iter()
        .map(|(name, value)| {
            let value = value
                .to_str()
                .map_err(|err| AppError::Config(err.to_string()))?
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            Ok((name.as_str().to_ascii_lowercase(), value))
        })
        .collect()
}

fn bedrock_signing_key(
    secret_access_key: &str,
    date_stamp: &str,
    region: &str,
) -> Result<Vec<u8>, AppError> {
    let date_key = hmac_sha256(
        format!("AWS4{secret_access_key}").as_bytes(),
        date_stamp.as_bytes(),
    )?;
    let date_region_key = hmac_sha256(&date_key, region.as_bytes())?;
    let date_region_service_key = hmac_sha256(&date_region_key, b"bedrock")?;
    hmac_sha256(&date_region_service_key, b"aws4_request")
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Result<Vec<u8>, AppError> {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(key).map_err(|err| AppError::Config(err.to_string()))?;
    mac.update(data);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn codex_base_url(provider: &Provider) -> Result<String, AppError> {
    let config = provider
        .settings_config
        .get("config")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let re = Regex::new(r#"base_url\s*=\s*["']([^"']+)["']"#)
        .map_err(|e| AppError::Config(e.to_string()))?;
    re.captures(config)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| AppError::Config("Missing Codex base_url".to_string()))
}

fn codex_api_key(provider: &Provider) -> Result<String, AppError> {
    provider
        .settings_config
        .get("auth")
        .and_then(Value::as_object)
        .and_then(|auth| auth.get("OPENAI_API_KEY"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| AppError::Config("Missing Codex OPENAI_API_KEY".to_string()))
}

fn first_opencode_model(
    provider: &Provider,
) -> Option<(String, Map<String, Value>, Option<String>)> {
    provider
        .settings_config
        .get("models")
        .and_then(Value::as_object)
        .and_then(|models| {
            models.iter().next().map(|(model_id, model)| {
                let options = model
                    .get("options")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                let variant = model
                    .get("variant")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                (model_id.clone(), options, variant)
            })
        })
}

fn apply_model_options(body: &mut Value, options: &Map<String, Value>) {
    if options.is_empty() {
        return;
    }
    if let Some(obj) = body.as_object_mut() {
        for (key, value) in options {
            if matches!(key.as_str(), "model" | "messages" | "contents" | "stream") {
                continue;
            }
            obj.insert(key.clone(), value.clone());
        }
    }
}

fn apply_model_variant(body: &mut Value, variant: Option<&str>) {
    if let Some(variant) = variant {
        if let Some(obj) = body.as_object_mut() {
            obj.insert("variant".to_string(), Value::String(variant.to_string()));
        }
    }
}

fn default_opencode_base_url(npm: &str) -> &'static str {
    match npm {
        "@ai-sdk/anthropic" => "https://api.anthropic.com",
        "@ai-sdk/google" => "https://generativelanguage.googleapis.com",
        _ => "https://api.openai.com/v1",
    }
}

fn normalize_v1(base_url: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    if base.ends_with("/v1") {
        base.to_string()
    } else {
        format!("{base}/v1")
    }
}

fn json_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
    headers
}

fn insert_header(headers: &mut HeaderMap, key: &str, value: &str) {
    if let (Ok(name), Ok(value)) = (
        HeaderName::from_bytes(key.as_bytes()),
        HeaderValue::from_str(value),
    ) {
        headers.insert(name, value);
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::provider::Provider;
    use reqwest::header::AUTHORIZATION;
    use serde_json::json;

    fn opencode_provider(npm: &str, options: Value, models: Value) -> Provider {
        Provider::with_id(
            "provider".to_string(),
            "Provider".to_string(),
            json!({
                "npm": npm,
                "options": options,
                "models": models,
            }),
            None,
        )
    }

    #[test]
    fn opencode_stream_request_merges_model_options_into_body() {
        let provider = opencode_provider(
            "@ai-sdk/openai-compatible",
            json!({
                    "baseURL": "https://api.example.com/v1",
                    "apiKey": "sk-test"
            }),
            json!({
                "gpt-5.4": {
                    "name": "GPT-5.4",
                    "variant": "high",
                    "options": {
                        "reasoningEffort": "high",
                        "textVerbosity": "low",
                        "provider": { "order": ["baseten"] }
                    }
                }
            }),
        );
        let config = StreamCheckConfig::default();

        let request = build_opencode_request(&provider, &config).expect("build request");
        assert_eq!(request.model, "gpt-5.4");
        assert_eq!(request.url, "https://api.example.com/v1/chat/completions");
        assert_eq!(request.body["model"], "gpt-5.4");
        assert_eq!(request.body["reasoningEffort"], "high");
        assert_eq!(request.body["textVerbosity"], "low");
        assert_eq!(request.body["provider"]["order"][0], "baseten");
        assert_eq!(request.body["variant"], "high");
        assert_eq!(request.body["stream"], true);
    }

    #[test]
    fn opencode_anthropic_stream_request_uses_messages_api_headers() {
        let provider = opencode_provider(
            "@ai-sdk/anthropic",
            json!({
                "baseURL": "https://anthropic.example.com",
                "apiKey": "sk-ant"
            }),
            json!({
                "claude-sonnet": {
                    "name": "Claude Sonnet",
                    "options": { "temperature": 0.2 }
                }
            }),
        );
        let config = StreamCheckConfig {
            test_prompt: "ping".to_string(),
            ..StreamCheckConfig::default()
        };

        let request = build_opencode_request(&provider, &config).expect("build request");
        assert_eq!(request.url, "https://anthropic.example.com/v1/messages");
        assert_eq!(request.model, "claude-sonnet");
        assert_eq!(request.body["model"], "claude-sonnet");
        assert_eq!(request.body["messages"][0]["content"], "ping");
        assert_eq!(request.body["temperature"], 0.2);
        assert!(request.headers.get(AUTHORIZATION).is_none());
        assert_eq!(request.headers.get("x-api-key").unwrap(), "sk-ant");
        assert_eq!(
            request.headers.get("anthropic-version").unwrap(),
            "2023-06-01"
        );
    }

    #[test]
    fn opencode_google_stream_request_uses_gemini_endpoint_and_key_header() {
        let provider = opencode_provider(
            "@ai-sdk/google",
            json!({
                "baseURL": "https://generativelanguage.googleapis.com/v1beta",
                "apiKey": "google-key"
            }),
            json!({
                "gemini-3-flash": {
                    "name": "Gemini 3 Flash",
                    "options": { "generationConfig": { "temperature": 0.1 } }
                }
            }),
        );
        let config = StreamCheckConfig::default();

        let request = build_opencode_request(&provider, &config).expect("build request");
        assert_eq!(
            request.url,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash:streamGenerateContent"
        );
        assert_eq!(request.model, "gemini-3-flash");
        assert_eq!(
            request.body["contents"][0]["parts"][0]["text"],
            "Who are you?"
        );
        assert_eq!(request.body["generationConfig"]["temperature"], 0.1);
        assert!(request.headers.get(AUTHORIZATION).is_none());
        assert_eq!(request.headers.get("x-goog-api-key").unwrap(), "google-key");
    }

    #[test]
    fn opencode_openai_stream_request_uses_responses_api() {
        let provider = opencode_provider(
            "@ai-sdk/openai",
            json!({
                "baseURL": "https://api.openai.com",
                "apiKey": "sk-openai"
            }),
            json!({
                "gpt-5.4": {
                    "name": "GPT-5.4",
                    "variant": "low",
                    "options": { "reasoning": { "effort": "low" } }
                }
            }),
        );
        let config = StreamCheckConfig {
            test_prompt: "hello".to_string(),
            ..StreamCheckConfig::default()
        };

        let request = build_opencode_request(&provider, &config).expect("build request");
        assert_eq!(request.url, "https://api.openai.com/v1/responses");
        assert_eq!(request.body["model"], "gpt-5.4");
        assert_eq!(request.body["input"], "hello");
        assert_eq!(request.body["stream"], true);
        assert_eq!(request.body["max_output_tokens"], 16);
        assert_eq!(request.body["reasoning"]["effort"], "low");
        assert_eq!(request.body["variant"], "low");
        assert_eq!(
            request.headers.get(AUTHORIZATION).unwrap(),
            "Bearer sk-openai"
        );
    }

    #[test]
    fn opencode_bedrock_stream_request_builds_signed_converse_stream_probe() {
        let provider = opencode_provider(
            "@ai-sdk/amazon-bedrock",
            json!({
                "region": "us-east-1",
                "accessKeyId": "AKIATEST",
                "secretAccessKey": "secret",
                "sessionToken": "session-token"
            }),
            json!({
                "global.anthropic.claude-haiku-4-5-20251001-v1:0": {
                    "name": "Claude Haiku"
                }
            }),
        );
        let config = StreamCheckConfig {
            test_prompt: "ping".to_string(),
            ..StreamCheckConfig::default()
        };

        let request = build_opencode_request(&provider, &config).expect("build request");

        assert_eq!(
            request.url,
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/global.anthropic.claude-haiku-4-5-20251001-v1%3A0/converse-stream"
        );
        assert_eq!(
            request.model,
            "global.anthropic.claude-haiku-4-5-20251001-v1:0"
        );
        assert_eq!(request.body["messages"][0]["content"][0]["text"], "ping");
        assert_eq!(request.body["inferenceConfig"]["maxTokens"], 16);
        assert!(request
            .body_bytes
            .as_ref()
            .is_some_and(|bytes| !bytes.is_empty()));
        assert_eq!(
            request.headers.get("x-amz-security-token").unwrap(),
            "session-token"
        );
        assert!(request.headers.get("x-amz-date").is_some());
        assert!(request.headers.get("x-amz-content-sha256").is_some());
        assert!(request
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("AWS4-HMAC-SHA256 ")));
    }

    #[test]
    fn omo_stream_check_points_to_underlying_opencode_provider() {
        let provider = opencode_provider(
            "@ai-sdk/openai-compatible",
            json!({ "apiKey": "sk-test" }),
            json!({}),
        );
        let err = build_request(&AppType::OmoSlim, &provider, &StreamCheckConfig::default())
            .expect_err("OMO Slim should not expose direct stream probes");

        assert!(err
            .to_string()
            .contains("test the underlying OpenCode provider"));
    }

    #[test]
    fn opencode_stream_request_falls_back_to_configured_codex_model() {
        let provider = opencode_provider(
            "@ai-sdk/openai-compatible",
            json!({
                "baseURL": "https://api.example.com/v1",
                "apiKey": "sk-test"
            }),
            json!({}),
        );
        let config = StreamCheckConfig {
            codex_model: "fallback-model".to_string(),
            ..StreamCheckConfig::default()
        };

        let request = build_opencode_request(&provider, &config).expect("build request");
        assert_eq!(request.model, "fallback-model");
        assert_eq!(request.body["model"], "fallback-model");
    }

    #[test]
    fn opencode_stream_request_ignores_protected_model_option_keys() {
        let provider = opencode_provider(
            "@ai-sdk/openai-compatible",
            json!({
                "baseURL": "https://api.example.com/v1",
                "apiKey": "sk-test"
            }),
            json!({
                "safe-model": {
                    "options": {
                        "model": "malicious-model",
                        "messages": [],
                        "stream": false,
                        "max_tokens": 32
                    }
                }
            }),
        );
        let config = StreamCheckConfig::default();

        let request = build_opencode_request(&provider, &config).expect("build request");
        assert_eq!(request.body["model"], "safe-model");
        assert_eq!(request.body["max_tokens"], 32);
        assert_eq!(request.body["stream"], true);
        assert!(request.body["messages"]
            .as_array()
            .is_some_and(|messages| !messages.is_empty()));
    }

    #[test]
    fn stream_check_config_rejects_unsafe_boundaries() {
        let config = StreamCheckConfig {
            timeout_secs: 0,
            ..StreamCheckConfig::default()
        };
        assert!(validate_config(&config)
            .expect_err("zero timeout should fail")
            .to_string()
            .contains("timeoutSecs"));

        let defaults = StreamCheckConfig::default();
        let config = StreamCheckConfig {
            degraded_threshold_ms: defaults.timeout_secs * 1000 + 1,
            ..defaults
        };
        assert!(validate_config(&config)
            .expect_err("threshold after timeout should fail")
            .to_string()
            .contains("degradedThresholdMs"));

        let mut config = StreamCheckConfig::default();
        config.test_prompt.clear();
        assert!(validate_config(&config)
            .expect_err("empty prompt should fail")
            .to_string()
            .contains("testPrompt"));
    }

    #[test]
    fn stream_check_error_categories_are_stable() {
        assert_eq!(
            classify_error(401, "invalid token").as_deref(),
            Some("authenticationFailed")
        );
        assert_eq!(
            classify_error(404, "model missing").as_deref(),
            Some("modelNotFound")
        );
        assert_eq!(
            classify_error(429, "rate limit").as_deref(),
            Some("quotaExceeded")
        );
        assert_eq!(
            classify_error(503, "maintenance").as_deref(),
            Some("upstreamServerError")
        );
    }

    #[test]
    fn stream_check_error_bodies_redact_request_credentials() {
        let provider = opencode_provider(
            "@ai-sdk/openai-compatible",
            json!({
                "baseURL": "https://api.example.com/v1",
                "apiKey": "header-secret"
            }),
            json!({}),
        );
        let mut request =
            build_opencode_request(&provider, &StreamCheckConfig::default()).expect("request");
        request.url.push_str("?api_key=query-secret");
        request.body["metadata"] = json!({ "refresh_token": "body-secret" });
        insert_header(&mut request.headers, "x-session-token", "session-secret");

        let public = redact_probe_secrets(
            "Bearer header-secret query-secret body-secret session-secret",
            &request,
        );

        for secret in [
            "header-secret",
            "query-secret",
            "body-secret",
            "session-secret",
        ] {
            assert!(!public.contains(secret));
        }
        assert!(public.contains("[redacted]"));
    }
}

fn failed(
    model: &str,
    message: &str,
    http_status: Option<u16>,
    error_category: Option<String>,
    retry_count: u32,
) -> StreamCheckResult {
    StreamCheckResult {
        status: HealthStatus::Failed,
        success: false,
        message: message.to_string(),
        response_time_ms: None,
        http_status,
        model_used: model.to_string(),
        tested_at: chrono::Utc::now().timestamp(),
        retry_count,
        error_category,
    }
}

fn should_retry(result: &StreamCheckResult) -> bool {
    matches!(result.http_status, Some(408 | 409 | 429 | 500..=599)) || result.http_status.is_none()
}

fn classify_error(status: u16, body: &str) -> Option<String> {
    let lower = body.to_ascii_lowercase();
    if status == 404 || lower.contains("model") && lower.contains("not found") {
        Some("modelNotFound".to_string())
    } else if status == 429 || lower.contains("quota") || lower.contains("rate limit") {
        Some("quotaExceeded".to_string())
    } else {
        Some(
            match status {
                400 | 405 | 415 | 422 => "invalidRequest",
                401 => "authenticationFailed",
                403 => "permissionDenied",
                408 => "timeout",
                409 => "conflict",
                500..=599 => "upstreamServerError",
                _ => "httpError",
            }
            .to_string(),
        )
    }
}

fn truncate(body: &str) -> String {
    const MAX: usize = 512;
    if body.chars().count() <= MAX {
        body.to_string()
    } else {
        let mut value: String = body.chars().take(MAX).collect();
        value.push_str("...");
        value
    }
}
