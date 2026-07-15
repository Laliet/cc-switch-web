use crate::provider::{UsageData, UsageResult};
use reqwest::{Client, StatusCode};
use serde_json::Value;
use std::time::Duration;

const FIVE_HOUR: &str = "five_hour";
const WEEKLY: &str = "weekly_limit";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodingPlanProvider {
    Kimi,
    Zhipu,
    MiniMaxCn,
    MiniMaxEn,
}

#[derive(Debug, Clone, PartialEq)]
struct QuotaTier {
    name: String,
    utilization: f64,
    resets_at: Option<String>,
}

fn detect_provider(base_url: &str) -> Option<CodingPlanProvider> {
    let url = base_url.to_ascii_lowercase();
    if url.contains("api.kimi.com/coding") {
        Some(CodingPlanProvider::Kimi)
    } else if url.contains("open.bigmodel.cn")
        || url.contains("bigmodel.cn")
        || url.contains("api.z.ai")
    {
        Some(CodingPlanProvider::Zhipu)
    } else if url.contains("api.minimaxi.com") {
        Some(CodingPlanProvider::MiniMaxCn)
    } else if url.contains("api.minimax.io") {
        Some(CodingPlanProvider::MiniMaxEn)
    } else {
        None
    }
}

fn error(message: impl Into<String>) -> UsageResult {
    UsageResult {
        success: false,
        data: None,
        error: Some(message.into()),
    }
}

fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|raw| raw.parse().ok()))
}

fn timestamp(value: &Value) -> Option<String> {
    if let Some(raw) = value.as_str() {
        return Some(raw.to_string());
    }
    let raw = value.as_i64()?;
    let milliseconds = if raw < 1_000_000_000_000 {
        raw * 1_000
    } else {
        raw
    };
    chrono::DateTime::from_timestamp(
        milliseconds / 1_000,
        ((milliseconds % 1_000) * 1_000_000) as u32,
    )
    .map(|time| time.to_rfc3339())
}

fn utilization(total: f64, remaining: f64) -> f64 {
    if total <= 0.0 {
        0.0
    } else {
        (((total - remaining).max(0.0) / total) * 100.0).clamp(0.0, 100.0)
    }
}

fn parse_kimi(body: &Value) -> Vec<QuotaTier> {
    let mut tiers = Vec::new();
    if let Some(limits) = body.get("limits").and_then(Value::as_array) {
        for item in limits {
            let Some(detail) = item.get("detail") else {
                continue;
            };
            let total = detail.get("limit").and_then(number).unwrap_or(1.0);
            let remaining = detail.get("remaining").and_then(number).unwrap_or(0.0);
            tiers.push(QuotaTier {
                name: FIVE_HOUR.to_string(),
                utilization: utilization(total, remaining),
                resets_at: detail.get("resetTime").and_then(timestamp),
            });
        }
    }
    if let Some(usage) = body.get("usage") {
        let total = usage.get("limit").and_then(number).unwrap_or(1.0);
        let remaining = usage.get("remaining").and_then(number).unwrap_or(0.0);
        tiers.push(QuotaTier {
            name: WEEKLY.to_string(),
            utilization: utilization(total, remaining),
            resets_at: usage.get("resetTime").and_then(timestamp),
        });
    }
    tiers
}

fn parse_zhipu(body: &Value) -> Result<Vec<QuotaTier>, String> {
    if body.get("success").and_then(Value::as_bool) == Some(false) {
        return Err("Zhipu API returned an error".to_string());
    }
    let data = body
        .get("data")
        .ok_or_else(|| "Missing data in Zhipu response".to_string())?;
    let mut limits = data
        .get("limits")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| {
            item.get("type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind.eq_ignore_ascii_case("TOKENS_LIMIT"))
        })
        .map(|item| {
            let reset = item
                .get("nextResetTime")
                .and_then(Value::as_i64)
                .unwrap_or(i64::MAX);
            let utilization = item.get("percentage").and_then(number).unwrap_or(0.0);
            let resets_at = item.get("nextResetTime").and_then(timestamp);
            (reset, utilization, resets_at)
        })
        .collect::<Vec<_>>();
    limits.sort_by_key(|item| item.0);
    Ok(limits
        .into_iter()
        .take(2)
        .enumerate()
        .map(|(index, (_, utilization, resets_at))| QuotaTier {
            name: if index == 0 { FIVE_HOUR } else { WEEKLY }.to_string(),
            utilization,
            resets_at,
        })
        .collect())
}

fn parse_minimax(body: &Value) -> Result<Vec<QuotaTier>, String> {
    if let Some(response) = body.get("base_resp") {
        let code = response
            .get("status_code")
            .and_then(Value::as_i64)
            .unwrap_or(-1);
        if code != 0 {
            return Err("MiniMax API returned an error".to_string());
        }
    }
    let Some(item) = body
        .get("model_remains")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
    else {
        return Ok(Vec::new());
    };
    let mut tiers = Vec::new();
    let interval_total = item
        .get("current_interval_total_count")
        .and_then(number)
        .unwrap_or(0.0);
    if interval_total > 0.0 {
        tiers.push(QuotaTier {
            name: FIVE_HOUR.to_string(),
            utilization: utilization(
                interval_total,
                item.get("current_interval_usage_count")
                    .and_then(number)
                    .unwrap_or(0.0),
            ),
            resets_at: item.get("end_time").and_then(timestamp),
        });
    }
    let weekly_total = item
        .get("current_weekly_total_count")
        .and_then(number)
        .unwrap_or(0.0);
    if weekly_total > 0.0 {
        tiers.push(QuotaTier {
            name: WEEKLY.to_string(),
            utilization: utilization(
                weekly_total,
                item.get("current_weekly_usage_count")
                    .and_then(number)
                    .unwrap_or(0.0),
            ),
            resets_at: item.get("weekly_end_time").and_then(timestamp),
        });
    }
    Ok(tiers)
}

async fn request_quota(provider: CodingPlanProvider, api_key: &str) -> Result<Value, String> {
    let (url, bearer) = match provider {
        CodingPlanProvider::Kimi => ("https://api.kimi.com/coding/v1/usages", true),
        CodingPlanProvider::Zhipu => ("https://api.z.ai/api/monitor/usage/quota/limit", false),
        CodingPlanProvider::MiniMaxCn => (
            "https://api.minimaxi.com/v1/api/openplatform/coding_plan/remains",
            true,
        ),
        CodingPlanProvider::MiniMaxEn => (
            "https://api.minimax.io/v1/api/openplatform/coding_plan/remains",
            true,
        ),
    };
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| "Failed to create HTTP client".to_string())?;
    let authorization = if bearer {
        format!("Bearer {api_key}")
    } else {
        api_key.to_string()
    };
    let response = client
        .get(url)
        .header("authorization", authorization)
        .header("accept", "application/json")
        .send()
        .await
        .map_err(|_| "Network error".to_string())?;
    let status = response.status();
    if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
        return Err(format!("Authentication failed (HTTP {status})"));
    }
    if !status.is_success() {
        return Err(format!("API error (HTTP {status})"));
    }
    response
        .json()
        .await
        .map_err(|_| "Failed to parse response".to_string())
}

pub async fn get_coding_plan_quota(base_url: &str, api_key: &str) -> UsageResult {
    if api_key.trim().is_empty() {
        return error("API key is empty");
    }
    let Some(provider) = detect_provider(base_url) else {
        return error("Unknown Coding Plan provider");
    };
    let body = match request_quota(provider, api_key).await {
        Ok(body) => body,
        Err(message) => return error(message),
    };
    let tiers = match provider {
        CodingPlanProvider::Kimi => Ok(parse_kimi(&body)),
        CodingPlanProvider::Zhipu => parse_zhipu(&body),
        CodingPlanProvider::MiniMaxCn | CodingPlanProvider::MiniMaxEn => parse_minimax(&body),
    };
    match tiers {
        Ok(tiers) => UsageResult {
            success: true,
            data: Some(
                tiers
                    .into_iter()
                    .map(|tier| UsageData {
                        plan_name: Some(tier.name),
                        total: Some(100.0),
                        used: Some(tier.utilization),
                        remaining: Some((100.0 - tier.utilization).max(0.0)),
                        unit: Some("%".to_string()),
                        is_valid: Some(true),
                        invalid_message: None,
                        extra: tier.resets_at,
                    })
                    .collect(),
            ),
            error: None,
        },
        Err(message) => error(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_coding_plan_hosts() {
        assert_eq!(
            detect_provider("https://api.kimi.com/coding"),
            Some(CodingPlanProvider::Kimi)
        );
        assert_eq!(
            detect_provider("https://open.bigmodel.cn/api/anthropic"),
            Some(CodingPlanProvider::Zhipu)
        );
        assert_eq!(
            detect_provider("https://api.z.ai/api/anthropic"),
            Some(CodingPlanProvider::Zhipu)
        );
        assert_eq!(
            detect_provider("https://api.minimaxi.com/anthropic"),
            Some(CodingPlanProvider::MiniMaxCn)
        );
        assert_eq!(
            detect_provider("https://api.minimax.io/anthropic"),
            Some(CodingPlanProvider::MiniMaxEn)
        );
    }

    #[test]
    fn zhipu_tiers_are_sorted_by_reset_time() {
        let tiers = parse_zhipu(&json!({
            "success": true,
            "data": {"limits": [
                {"type": "TOKENS_LIMIT", "percentage": 53, "nextResetTime": 2_000_000_000_000_i64},
                {"type": "TOKENS_LIMIT", "percentage": 44, "nextResetTime": 1_000_000_000_000_i64},
                {"type": "TIME_LIMIT", "percentage": 7}
            ]}
        }))
        .expect("zhipu quota");
        assert_eq!(tiers.len(), 2);
        assert_eq!(tiers[0].name, FIVE_HOUR);
        assert_eq!(tiers[0].utilization, 44.0);
        assert_eq!(tiers[1].name, WEEKLY);
    }

    #[test]
    fn minimax_remaining_counts_are_converted_to_utilization() {
        let tiers = parse_minimax(&json!({
            "base_resp": {"status_code": 0},
            "model_remains": [{
                "current_interval_total_count": 100,
                "current_interval_usage_count": 25,
                "current_weekly_total_count": 1000,
                "current_weekly_usage_count": 800
            }]
        }))
        .expect("minimax quota");
        assert_eq!(tiers[0].utilization, 75.0);
        assert_eq!(tiers[1].utilization, 20.0);
    }
}
