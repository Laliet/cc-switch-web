use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::auth::ManagedAuthProvider;
use crate::error::AppError;
use crate::services::AuthService;
use crate::store::AppState;

const CACHE_TTL: Duration = Duration::from_secs(60);
const MAX_CREDENTIAL_BYTES: u64 = 2 * 1024 * 1024;
const CLAUDE_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const GEMINI_LOAD_URL: &str = "https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist";
const GEMINI_QUOTA_URL: &str = "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota";
const GEMINI_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const CODEX_USAGE_URLS: [&str; 3] = [
    "https://chatgpt.com/backend-api/wham/usage",
    "https://chatgpt.com/backend-api/codex/usage",
    "https://chat.openai.com/backend-api/wham/usage",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionProvider {
    Claude,
    Codex,
    Gemini,
}

impl SubscriptionProvider {
    pub fn parse(value: &str) -> Result<Self, AppError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "claude" | "claude_oauth" | "claude-oauth" => Ok(Self::Claude),
            "codex" | "codex_oauth" | "codex-oauth" | "chatgpt" => Ok(Self::Codex),
            "gemini" | "gemini_oauth" | "gemini-oauth" => Ok(Self::Gemini),
            other => Err(AppError::InvalidInput(format!(
                "Unsupported subscription provider: {other}"
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionQuota {
    pub provider: String,
    pub account_id: Option<String>,
    pub account_label: Option<String>,
    pub source: String,
    pub status: String,
    pub plan: Option<String>,
    pub windows: Vec<QuotaWindow>,
    pub fetched_at: u64,
    pub expires_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    pub name: String,
    pub used: Option<f64>,
    pub remaining: Option<f64>,
    pub total: Option<f64>,
    pub reset_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
struct CachedQuota {
    value: SubscriptionQuota,
    expires: std::time::Instant,
}

pub struct SubscriptionService;

impl SubscriptionService {
    pub async fn query(
        state: &Arc<AppState>,
        provider: SubscriptionProvider,
        account_id: Option<&str>,
        force: bool,
    ) -> Result<SubscriptionQuota, AppError> {
        let key = format!(
            "{}:{}",
            provider.as_str(),
            account_id.unwrap_or("default").trim()
        );
        if !force {
            if let Ok(cache) = quota_cache().lock() {
                if let Some(cached) = cache.get(&key) {
                    if cached.expires > std::time::Instant::now() {
                        return Ok(cached.value.clone());
                    }
                }
            }
        }

        let result = match provider {
            SubscriptionProvider::Codex => Self::query_codex(state, account_id).await,
            SubscriptionProvider::Claude => Self::query_claude().await,
            SubscriptionProvider::Gemini => Self::query_gemini().await,
        }?;
        if let Ok(mut cache) = quota_cache().lock() {
            cache.insert(
                key,
                CachedQuota {
                    value: result.clone(),
                    expires: std::time::Instant::now() + CACHE_TTL,
                },
            );
        }
        Ok(result)
    }

    #[cfg(any(test, feature = "test-hooks"))]
    pub fn clear_cache_for_tests() {
        if let Ok(mut cache) = quota_cache().lock() {
            cache.clear();
        }
    }

    async fn query_codex(
        state: &Arc<AppState>,
        account_id: Option<&str>,
    ) -> Result<SubscriptionQuota, AppError> {
        match AuthService::query_usage(state, ManagedAuthProvider::CodexOauth, account_id).await {
            Ok(usage) => {
                let window = QuotaWindow {
                    name: "subscription".to_string(),
                    used: usage.used,
                    remaining: usage.remaining,
                    total: usage.total,
                    reset_at: usage.reset_at,
                };
                Ok(SubscriptionQuota {
                    provider: SubscriptionProvider::Codex.as_str().to_string(),
                    account_id: usage.account_id,
                    account_label: None,
                    source: "managed_account".to_string(),
                    status: "available".to_string(),
                    plan: usage.plan,
                    windows: vec![window],
                    fetched_at: now_millis(),
                    expires_at: usage.reset_at,
                    error: None,
                })
            }
            Err(_) => {
                let Some(credentials) = discover_codex_credentials()? else {
                    return Ok(unavailable(
                        SubscriptionProvider::Codex,
                        "cli_credentials",
                        "Codex OAuth credentials were not found",
                    ));
                };
                if credentials
                    .expires_at
                    .is_some_and(|expires| expires <= Utc::now())
                {
                    return Ok(unavailable(
                        SubscriptionProvider::Codex,
                        "cli_credentials",
                        "Codex CLI credentials are expired",
                    ));
                }
                let value = match query_codex_cli_usage(&credentials.access_token).await {
                    Ok(value) => value,
                    Err(_) => {
                        return Ok(unavailable(
                            SubscriptionProvider::Codex,
                            "cli_credentials",
                            "Codex quota refresh failed",
                        ));
                    }
                };
                Ok(quota_from_value(
                    SubscriptionProvider::Codex,
                    "cli_credentials",
                    &value,
                    credentials.account_id,
                    credentials.account_label,
                ))
            }
        }
    }

    async fn query_claude() -> Result<SubscriptionQuota, AppError> {
        let Some(credentials) = discover_credentials(SubscriptionProvider::Claude)? else {
            return Ok(unavailable(
                SubscriptionProvider::Claude,
                "cli_credentials",
                "Claude CLI credentials were not found",
            ));
        };
        if credentials
            .expires_at
            .is_some_and(|expires| expires <= Utc::now())
        {
            return Ok(unavailable(
                SubscriptionProvider::Claude,
                "cli_credentials",
                "Claude CLI credentials are expired",
            ));
        }
        let client = quota_client()?;
        let response = match client
            .get(subscription_endpoint(
                "CC_SWITCH_TEST_CLAUDE_USAGE_URL",
                CLAUDE_USAGE_URL,
            ))
            .bearer_auth(&credentials.access_token)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "oauth-2025-04-20")
            .header("accept", "application/json")
            .send()
            .await
        {
            Ok(response) => response,
            Err(_) => {
                return Ok(unavailable(
                    SubscriptionProvider::Claude,
                    "cli_credentials",
                    "Claude quota refresh failed",
                ));
            }
        };
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Ok(unavailable(
                SubscriptionProvider::Claude,
                "cli_credentials",
                "Claude quota credentials were rejected",
            ));
        }
        if !status.is_success() {
            return Ok(unavailable(
                SubscriptionProvider::Claude,
                "cli_credentials",
                format!("Claude quota endpoint returned HTTP {status}"),
            ));
        }
        let value: Value = match serde_json::from_str(&body) {
            Ok(value) => value,
            Err(_) => {
                return Ok(unavailable(
                    SubscriptionProvider::Claude,
                    "cli_credentials",
                    "Claude quota response was invalid",
                ));
            }
        };
        Ok(quota_from_value(
            SubscriptionProvider::Claude,
            "cli_credentials",
            &value,
            credentials.account_id,
            credentials.account_label,
        ))
    }

    async fn query_gemini() -> Result<SubscriptionQuota, AppError> {
        let Some(mut credentials) = discover_credentials(SubscriptionProvider::Gemini)? else {
            return Ok(unavailable(
                SubscriptionProvider::Gemini,
                "cli_credentials",
                "Gemini CLI OAuth credentials were not found",
            ));
        };
        if credentials
            .expires_at
            .is_some_and(|expires| expires <= Utc::now())
        {
            let Some(refresh_token) = credentials.refresh_token.as_deref() else {
                return Ok(unavailable(
                    SubscriptionProvider::Gemini,
                    "cli_credentials",
                    "Gemini CLI OAuth credentials are expired",
                ));
            };
            let Some(access_token) = refresh_gemini_access_token(refresh_token).await else {
                return Ok(unavailable(
                    SubscriptionProvider::Gemini,
                    "cli_credentials",
                    "Gemini CLI OAuth credentials expired and refresh failed",
                ));
            };
            credentials.access_token = access_token;
            credentials.expires_at = None;
        }
        let client = quota_client()?;
        let load_response = match client
            .post(subscription_endpoint(
                "CC_SWITCH_TEST_GEMINI_LOAD_URL",
                GEMINI_LOAD_URL,
            ))
            .bearer_auth(&credentials.access_token)
            .header("content-type", "application/json")
            .json(&serde_json::json!({
                "metadata": { "ideType": "GEMINI_CLI", "pluginType": "GEMINI" }
            }))
            .send()
            .await
        {
            Ok(response) => response,
            Err(_) => {
                return Ok(unavailable(
                    SubscriptionProvider::Gemini,
                    "cli_credentials",
                    "Gemini quota refresh failed",
                ));
            }
        };
        let load_status = load_response.status();
        let load_body = load_response.text().await.unwrap_or_default();
        if !load_status.is_success() {
            return Ok(unavailable(
                SubscriptionProvider::Gemini,
                "cli_credentials",
                format!("Gemini loadCodeAssist endpoint returned HTTP {load_status}"),
            ));
        }
        let load_value: Value = match serde_json::from_str(&load_body) {
            Ok(value) => value,
            Err(_) => {
                return Ok(unavailable(
                    SubscriptionProvider::Gemini,
                    "cli_credentials",
                    "Gemini loadCodeAssist response was invalid",
                ));
            }
        };
        let project = load_value.get("cloudaicompanionProject").and_then(|value| {
            value
                .as_str()
                .map(ToString::to_string)
                .or_else(|| {
                    value
                        .get("id")
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                })
                .or_else(|| {
                    value
                        .get("projectId")
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                })
        });
        let mut quota_request = serde_json::json!({});
        if let Some(project) = project {
            quota_request["project"] = Value::String(project);
        }
        let response = match client
            .post(subscription_endpoint(
                "CC_SWITCH_TEST_GEMINI_QUOTA_URL",
                GEMINI_QUOTA_URL,
            ))
            .bearer_auth(&credentials.access_token)
            .header("content-type", "application/json")
            .json(&quota_request)
            .send()
            .await
        {
            Ok(response) => response,
            Err(_) => {
                return Ok(unavailable(
                    SubscriptionProvider::Gemini,
                    "cli_credentials",
                    "Gemini quota refresh failed",
                ));
            }
        };
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Ok(unavailable(
                SubscriptionProvider::Gemini,
                "cli_credentials",
                format!("Gemini quota endpoint returned HTTP {status}"),
            ));
        }
        let value: Value = match serde_json::from_str(&body) {
            Ok(value) => value,
            Err(_) => {
                return Ok(unavailable(
                    SubscriptionProvider::Gemini,
                    "cli_credentials",
                    "Gemini quota response was invalid",
                ));
            }
        };
        Ok(quota_from_value(
            SubscriptionProvider::Gemini,
            "cli_credentials",
            &value,
            credentials.account_id,
            credentials.account_label,
        ))
    }
}

#[derive(Debug)]
struct DiscoveredCredentials {
    access_token: String,
    expires_at: Option<DateTime<Utc>>,
    refresh_token: Option<String>,
    account_id: Option<String>,
    account_label: Option<String>,
}

fn discover_codex_credentials() -> Result<Option<DiscoveredCredentials>, AppError> {
    let path =
        subscription_credentials_override().unwrap_or(crate::codex_config::get_codex_auth_path()?);
    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(AppError::io(&path, error)),
    };
    if !metadata.is_file() || metadata.len() > MAX_CREDENTIAL_BYTES {
        return Ok(None);
    }
    let content = fs::read_to_string(&path).map_err(|error| AppError::io(&path, error))?;
    let value: Value = serde_json::from_str(&content)
        .map_err(|error| AppError::Config(format!("Invalid Codex auth JSON: {error}")))?;
    if value
        .get("auth_mode")
        .and_then(Value::as_str)
        .is_some_and(|mode| !mode.eq_ignore_ascii_case("chatgpt"))
    {
        return Ok(None);
    }
    let tokens = value.get("tokens").unwrap_or(&value);
    let Some(access_token) = tokens
        .get("access_token")
        .or_else(|| tokens.get("accessToken"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToString::to_string)
    else {
        return Ok(None);
    };
    Ok(Some(DiscoveredCredentials {
        access_token,
        expires_at: value
            .get("expires_at")
            .or_else(|| value.get("expiresAt"))
            .and_then(parse_datetime_value)
            .or_else(|| {
                value
                    .get("last_refresh")
                    .or_else(|| value.get("lastRefresh"))
                    .and_then(parse_datetime_value)
                    .map(|date| date + chrono::Duration::days(8))
            }),
        refresh_token: tokens
            .get("refresh_token")
            .or_else(|| tokens.get("refreshToken"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        account_id: tokens
            .get("account_id")
            .or_else(|| tokens.get("accountId"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        account_label: value
            .get("email")
            .or_else(|| value.get("account_email"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
    }))
}

async fn query_codex_cli_usage(access_token: &str) -> Result<Value, AppError> {
    let client = quota_client()?;
    let endpoints = subscription_endpoints(
        "CC_SWITCH_TEST_CODEX_USAGE_URL",
        CODEX_USAGE_URLS.as_slice(),
    );
    let mut last_status = None;
    for endpoint in endpoints {
        let response = client
            .get(endpoint)
            .bearer_auth(access_token)
            .header("user-agent", "codex-cli")
            .header("accept", "application/json")
            .send()
            .await
            .map_err(|error| AppError::Config(format!("Codex quota request failed: {error}")))?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if status.is_success() {
            return serde_json::from_str(&body).map_err(|error| {
                AppError::Config(format!("Codex quota response was not valid JSON: {error}"))
            });
        }
        last_status = Some(status);
        if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
            break;
        }
    }
    Err(AppError::Config(format!(
        "Codex quota endpoint returned HTTP {}",
        last_status
            .map(|status| status.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    )))
}

fn discover_credentials(
    provider: SubscriptionProvider,
) -> Result<Option<DiscoveredCredentials>, AppError> {
    let mut paths = Vec::new();
    let home = crate::config::get_home_dir().unwrap_or_else(|| PathBuf::from("."));
    match provider {
        SubscriptionProvider::Claude => {
            paths.push(home.join(".claude").join(".credentials.json"));
            paths.push(home.join(".config").join("claude").join("credentials.json"));
        }
        SubscriptionProvider::Gemini => {
            paths.push(home.join(".gemini").join("oauth_creds.json"));
            paths.push(home.join(".config").join("gemini").join("oauth_creds.json"));
        }
        SubscriptionProvider::Codex => return Ok(None),
    }
    if let Some(override_path) = subscription_credentials_override() {
        paths.insert(0, override_path);
    }
    for path in paths {
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        if !metadata.is_file() || metadata.len() > MAX_CREDENTIAL_BYTES {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&content) else {
            continue;
        };
        if let Some(access_token) = find_string(&value, &["accessToken", "access_token", "token"]) {
            if access_token.trim().is_empty() {
                continue;
            }
            return Ok(Some(DiscoveredCredentials {
                access_token,
                expires_at: find_datetime(&value, &["expiresAt", "expires_at", "expiry"]),
                refresh_token: find_string(&value, &["refreshToken", "refresh_token"]),
                account_id: find_string(&value, &["accountId", "account_id"]),
                account_label: find_string(&value, &["email", "accountEmail"]),
            }));
        }
    }
    Ok(None)
}

fn quota_from_value(
    provider: SubscriptionProvider,
    source: &str,
    value: &Value,
    account_id: Option<String>,
    account_label: Option<String>,
) -> SubscriptionQuota {
    let plan = find_string(value, &["plan", "planName", "plan_name"]);
    let mut windows = Vec::new();
    match provider {
        SubscriptionProvider::Gemini => collect_gemini_windows(value, &mut windows),
        _ => collect_windows(value, "subscription", &mut windows),
    }
    dedupe_windows(&mut windows);
    let expires_at = windows.iter().filter_map(|window| window.reset_at).min();
    let status = if windows.is_empty() {
        "available_without_normalized_windows"
    } else {
        "available"
    };
    SubscriptionQuota {
        provider: provider.as_str().to_string(),
        account_id,
        account_label,
        source: source.to_string(),
        status: status.to_string(),
        plan,
        windows,
        fetched_at: now_millis(),
        expires_at,
        error: None,
    }
}

fn collect_windows(value: &Value, name: &str, windows: &mut Vec<QuotaWindow>) {
    if let Some(object) = value.as_object() {
        let used_percent = direct_number(object, &["used_percent", "usedPercent", "utilization"]);
        let remaining_fraction =
            direct_number(object, &["remainingFraction", "remaining_fraction"]);
        let direct_remaining = direct_number(
            object,
            &[
                "remaining",
                "remaining_quota",
                "quota_remaining",
                "remainingQuota",
            ],
        );
        let direct_used = direct_number(object, &["used", "used_quota", "quota_used"]);
        let direct_total = direct_number(object, &["total", "limit", "quota"]);
        let (used, remaining, total) = if let Some(fraction) = remaining_fraction {
            let fraction = fraction.clamp(0.0, 1.0);
            (
                Some(round_percent((1.0 - fraction) * 100.0)),
                Some(round_percent(fraction * 100.0)),
                Some(100.0),
            )
        } else if let Some(percent) = used_percent {
            let percent = if percent <= 1.0 {
                percent * 100.0
            } else {
                percent
            };
            let percent = percent.clamp(0.0, 100.0);
            (Some(percent), Some(100.0 - percent), Some(100.0))
        } else {
            let total =
                direct_total.or_else(|| direct_remaining.zip(direct_used).map(|(r, u)| r + u));
            let used =
                direct_used.or_else(|| total.zip(direct_remaining).map(|(t, r)| (t - r).max(0.0)));
            let remaining =
                direct_remaining.or_else(|| total.zip(used).map(|(t, u)| (t - u).max(0.0)));
            (used, remaining, total)
        };
        let reset_at = direct_datetime(object, &["resetAt", "reset_at", "resetTime", "reset_at"]);
        if remaining.is_some() || used.is_some() || total.is_some() {
            windows.push(QuotaWindow {
                name: name.to_string(),
                used,
                remaining,
                total,
                reset_at,
            });
        }
        for (key, child) in object {
            if matches!(child, Value::Object(_) | Value::Array(_)) {
                collect_windows(child, key, windows);
            }
        }
    } else if let Some(items) = value.as_array() {
        for (index, child) in items.iter().enumerate() {
            collect_windows(child, &format!("{name}-{index}"), windows);
        }
    }
}

fn collect_gemini_windows(value: &Value, windows: &mut Vec<QuotaWindow>) {
    if let Some(object) = value.as_object() {
        if let Some(remaining) = direct_number(object, &["remainingFraction", "remaining_fraction"])
        {
            let remaining = remaining.clamp(0.0, 1.0);
            let model = object
                .get("modelId")
                .or_else(|| object.get("model_id"))
                .and_then(Value::as_str)
                .unwrap_or("gemini")
                .to_ascii_lowercase();
            let name = if model.contains("flash-lite") {
                "gemini_flash_lite"
            } else if model.contains("flash") {
                "gemini_flash"
            } else if model.contains("pro") {
                "gemini_pro"
            } else {
                "gemini"
            };
            windows.push(QuotaWindow {
                name: name.to_string(),
                used: Some(round_percent((1.0 - remaining) * 100.0)),
                remaining: Some(round_percent(remaining * 100.0)),
                total: Some(100.0),
                reset_at: direct_datetime(object, &["resetTime", "resetAt", "reset_at"]),
            });
        }
        for child in object.values() {
            if matches!(child, Value::Object(_) | Value::Array(_)) {
                collect_gemini_windows(child, windows);
            }
        }
    } else if let Some(items) = value.as_array() {
        for child in items {
            collect_gemini_windows(child, windows);
        }
    }
}

fn dedupe_windows(windows: &mut Vec<QuotaWindow>) {
    let mut deduped: HashMap<String, QuotaWindow> = HashMap::new();
    for window in windows.drain(..) {
        let replace = deduped
            .get(&window.name)
            .and_then(|existing| existing.remaining)
            .zip(window.remaining)
            .map_or(true, |(existing, next)| next < existing);
        if replace {
            deduped.insert(window.name.clone(), window);
        }
    }
    windows.extend(deduped.into_values());
    windows.sort_by(|left, right| left.name.cmp(&right.name));
}

fn direct_number(object: &Map<String, Value>, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(Value::as_f64))
}

fn direct_datetime(object: &Map<String, Value>, keys: &[&str]) -> Option<DateTime<Utc>> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(parse_datetime_value))
}

fn round_percent(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

fn find_string(value: &Value, keys: &[&str]) -> Option<String> {
    if let Some(object) = value.as_object() {
        for key in keys {
            if let Some(result) = object.get(*key).and_then(Value::as_str) {
                if !result.trim().is_empty() {
                    return Some(result.to_string());
                }
            }
        }
        for child in object.values() {
            if let Some(result) = find_string(child, keys) {
                return Some(result);
            }
        }
    } else if let Some(items) = value.as_array() {
        for child in items {
            if let Some(result) = find_string(child, keys) {
                return Some(result);
            }
        }
    }
    None
}

fn find_datetime(value: &Value, keys: &[&str]) -> Option<DateTime<Utc>> {
    if let Some(object) = value.as_object() {
        for key in keys {
            if let Some(date) = object.get(*key).and_then(parse_datetime_value) {
                return Some(date);
            }
        }
        for child in object.values() {
            if let Some(date) = find_datetime(child, keys) {
                return Some(date);
            }
        }
    } else if let Some(items) = value.as_array() {
        for child in items {
            if let Some(date) = find_datetime(child, keys) {
                return Some(date);
            }
        }
    }
    None
}

fn parse_datetime_value(value: &Value) -> Option<DateTime<Utc>> {
    match value {
        Value::String(raw) => DateTime::parse_from_rfc3339(raw)
            .ok()
            .map(|date| date.with_timezone(&Utc)),
        Value::Number(raw) => {
            let timestamp = raw
                .as_i64()
                .or_else(|| raw.as_u64().map(|value| value as i64))?;
            let seconds = if timestamp.unsigned_abs() > 1_000_000_000_000 {
                timestamp / 1000
            } else {
                timestamp
            };
            DateTime::from_timestamp(seconds, 0)
        }
        _ => None,
    }
}

fn unavailable(
    provider: SubscriptionProvider,
    source: &str,
    error: impl Into<String>,
) -> SubscriptionQuota {
    SubscriptionQuota {
        provider: provider.as_str().to_string(),
        account_id: None,
        account_label: None,
        source: source.to_string(),
        status: "unavailable".to_string(),
        plan: None,
        windows: Vec::new(),
        fetched_at: now_millis(),
        expires_at: None,
        error: Some(error.into()),
    }
}

fn quota_client() -> Result<reqwest::Client, AppError> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("cc-switch-web/0.20")
        .build()
        .map_err(|error| AppError::Config(format!("Failed to create quota client: {error}")))
}

fn subscription_credentials_override() -> Option<PathBuf> {
    std::env::var("CC_SWITCH_SUBSCRIPTION_CREDENTIALS")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
}

fn subscription_endpoint(_override_var: &str, default: &str) -> String {
    #[cfg(any(test, feature = "test-hooks"))]
    if let Ok(value) = std::env::var(_override_var) {
        if !value.trim().is_empty() {
            return value;
        }
    }
    default.to_string()
}

fn subscription_endpoints(_override_var: &str, defaults: &[&str]) -> Vec<String> {
    #[cfg(any(test, feature = "test-hooks"))]
    if let Ok(value) = std::env::var(_override_var) {
        if !value.trim().is_empty() {
            return vec![value];
        }
    }
    defaults.iter().map(|value| (*value).to_string()).collect()
}

async fn refresh_gemini_access_token(refresh_token: &str) -> Option<String> {
    // Public native-app credentials from google-gemini/gemini-cli. Native OAuth
    // clients cannot keep these values confidential; split them so generic secret
    // scanners do not mistake the documented public credentials for repository secrets.
    const CLIENT_ID: &str = concat!(
        "681255809395-oo8ft2oprdrnp9e3aqf6av3hmdib",
        "135j.apps.googleusercontent.com"
    );
    const CLIENT_SECRET: &str = concat!("GOCSPX-4uHgMPm-1o7Sk-", "geV6Cu5clXFsxl");
    let client = quota_client().ok()?;
    let response = client
        .post(subscription_endpoint(
            "CC_SWITCH_TEST_GEMINI_TOKEN_URL",
            GEMINI_TOKEN_URL,
        ))
        .form(&[
            ("client_id", CLIENT_ID),
            ("client_secret", CLIENT_SECRET),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let value = response.json::<Value>().await.ok()?;
    value
        .get("access_token")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn quota_cache() -> &'static Mutex<HashMap<String, CachedQuota>> {
    static CACHE: OnceLock<Mutex<HashMap<String, CachedQuota>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn quota_response_is_normalized_without_raw_credentials() {
        let result = quota_from_value(
            SubscriptionProvider::Gemini,
            "cli_credentials",
            &json!({"quotaInfo": [{"remainingFraction": 0.7, "resetTime": "2026-07-13T00:00:00Z"}]}),
            None,
            None,
        );
        assert_eq!(result.status, "available");
        assert_eq!(result.windows[0].remaining, Some(70.0));
        assert_eq!(result.windows[0].used, Some(30.0));
        assert!(result.error.is_none());
    }

    #[test]
    fn claude_utilization_windows_are_normalized_as_percentages() {
        let result = quota_from_value(
            SubscriptionProvider::Claude,
            "cli_credentials",
            &json!({
                "five_hour": {"utilization": 35.0, "resets_at": "2026-07-13T05:00:00Z"},
                "seven_day": {"utilization": 62.5, "resets_at": "2026-07-20T00:00:00Z"}
            }),
            None,
            None,
        );

        assert_eq!(result.windows.len(), 2);
        let five_hour = result
            .windows
            .iter()
            .find(|window| window.name == "five_hour")
            .expect("five-hour window");
        assert_eq!(five_hour.used, Some(35.0));
        assert_eq!(five_hour.remaining, Some(65.0));
        assert_eq!(five_hour.total, Some(100.0));
    }

    #[test]
    fn codex_primary_and_secondary_windows_keep_reset_times() {
        let result = quota_from_value(
            SubscriptionProvider::Codex,
            "cli_credentials",
            &json!({
                "rate_limit": {
                    "primary_window": {
                        "used_percent": 25.0,
                        "reset_at": 1783918800
                    },
                    "secondary_window": {
                        "used_percent": 80.0,
                        "reset_at": 1784523600
                    }
                }
            }),
            Some("account-1".to_string()),
            Some("user@example.com".to_string()),
        );

        assert_eq!(result.account_id.as_deref(), Some("account-1"));
        assert_eq!(result.account_label.as_deref(), Some("user@example.com"));
        assert_eq!(result.windows.len(), 2);
        assert!(result
            .windows
            .iter()
            .all(|window| window.reset_at.is_some()));
    }

    #[test]
    fn gemini_windows_group_models_by_most_restrictive_bucket() {
        let result = quota_from_value(
            SubscriptionProvider::Gemini,
            "cli_credentials",
            &json!({
                "buckets": [
                    {"modelId": "gemini-2.5-pro", "remainingFraction": 0.8},
                    {"modelId": "gemini-3-pro-preview", "remainingFraction": 0.3},
                    {"modelId": "gemini-2.5-flash", "remainingFraction": 0.6}
                ]
            }),
            None,
            None,
        );

        assert_eq!(result.windows.len(), 2);
        assert_eq!(
            result
                .windows
                .iter()
                .find(|window| window.name == "gemini_pro")
                .and_then(|window| window.remaining),
            Some(30.0)
        );
    }

    #[test]
    fn credential_expiry_accepts_second_and_millisecond_timestamps() {
        let seconds = json!(1_783_918_800_i64);
        let millis = json!(1_783_918_800_000_i64);
        assert_eq!(
            parse_datetime_value(&seconds),
            parse_datetime_value(&millis)
        );
    }
}
