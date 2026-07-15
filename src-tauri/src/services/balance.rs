use crate::provider::{UsageData, UsageResult};
use reqwest::{Client, StatusCode};
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BalanceProvider {
    DeepSeek,
    StepFun,
    SiliconFlowCn,
    SiliconFlowEn,
    OpenRouter,
    Novita,
}

fn detect_provider(base_url: &str) -> Option<BalanceProvider> {
    let url = base_url.to_ascii_lowercase();
    if url.contains("api.deepseek.com") {
        Some(BalanceProvider::DeepSeek)
    } else if url.contains("api.stepfun.ai") || url.contains("api.stepfun.com") {
        Some(BalanceProvider::StepFun)
    } else if url.contains("api.siliconflow.cn") {
        Some(BalanceProvider::SiliconFlowCn)
    } else if url.contains("api.siliconflow.com") {
        Some(BalanceProvider::SiliconFlowEn)
    } else if url.contains("openrouter.ai") {
        Some(BalanceProvider::OpenRouter)
    } else if url.contains("api.novita.ai") {
        Some(BalanceProvider::Novita)
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

fn amount(value: &Value, field: &str) -> Option<f64> {
    value.get(field).and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_str().and_then(|raw| raw.parse().ok()))
    })
}

async fn get_json(url: &str, api_key: &str) -> Result<Value, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| "Failed to create HTTP client".to_string())?;
    let response = client
        .get(url)
        .bearer_auth(api_key)
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

fn data(
    plan_name: &str,
    remaining: f64,
    total: Option<f64>,
    used: Option<f64>,
    unit: &str,
) -> UsageData {
    UsageData {
        plan_name: Some(plan_name.to_string()),
        remaining: Some(remaining),
        total,
        used,
        unit: Some(unit.to_string()),
        is_valid: Some(remaining > 0.0),
        invalid_message: (remaining <= 0.0).then(|| "No balance remaining".to_string()),
        extra: None,
    }
}

fn parse_balance(provider: BalanceProvider, body: &Value) -> Result<Vec<UsageData>, String> {
    match provider {
        BalanceProvider::DeepSeek => {
            let available = body
                .get("is_available")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let entries = body
                .get("balance_infos")
                .and_then(Value::as_array)
                .ok_or_else(|| "Missing balance_infos in response".to_string())?;
            Ok(entries
                .iter()
                .map(|entry| {
                    let unit = entry
                        .get("currency")
                        .and_then(Value::as_str)
                        .unwrap_or("CNY");
                    let mut item = data(
                        unit,
                        amount(entry, "total_balance").unwrap_or(0.0),
                        None,
                        None,
                        unit,
                    );
                    item.is_valid = Some(available);
                    item.invalid_message = (!available).then(|| "Insufficient balance".to_string());
                    item
                })
                .collect())
        }
        BalanceProvider::StepFun => Ok(vec![data(
            "StepFun",
            amount(body, "balance").unwrap_or(0.0),
            None,
            None,
            "CNY",
        )]),
        BalanceProvider::SiliconFlowCn | BalanceProvider::SiliconFlowEn => {
            let account = body
                .get("data")
                .ok_or_else(|| "Missing data in response".to_string())?;
            let is_cn = provider == BalanceProvider::SiliconFlowCn;
            Ok(vec![data(
                if is_cn {
                    "SiliconFlow"
                } else {
                    "SiliconFlow (EN)"
                },
                amount(account, "totalBalance").unwrap_or(0.0),
                None,
                None,
                if is_cn { "CNY" } else { "USD" },
            )])
        }
        BalanceProvider::OpenRouter => {
            let account = body.get("data").unwrap_or(body);
            let total = amount(account, "total_credits").unwrap_or(0.0);
            let used = amount(account, "total_usage").unwrap_or(0.0);
            Ok(vec![data(
                "OpenRouter",
                total - used,
                Some(total),
                Some(used),
                "USD",
            )])
        }
        BalanceProvider::Novita => Ok(vec![data(
            "Novita AI",
            amount(body, "availableBalance").unwrap_or(0.0) / 10_000.0,
            None,
            None,
            "USD",
        )]),
    }
}

pub async fn get_balance(base_url: &str, api_key: &str) -> UsageResult {
    if api_key.trim().is_empty() {
        return error("API key is empty");
    }
    let Some(provider) = detect_provider(base_url) else {
        return error("Unknown balance provider");
    };
    let url = match provider {
        BalanceProvider::DeepSeek => "https://api.deepseek.com/user/balance",
        BalanceProvider::StepFun => "https://api.stepfun.com/v1/accounts",
        BalanceProvider::SiliconFlowCn => "https://api.siliconflow.cn/v1/user/info",
        BalanceProvider::SiliconFlowEn => "https://api.siliconflow.com/v1/user/info",
        BalanceProvider::OpenRouter => "https://openrouter.ai/api/v1/credits",
        BalanceProvider::Novita => "https://api.novita.ai/v3/user/balance",
    };
    let body = match get_json(url, api_key).await {
        Ok(body) => body,
        Err(message) => return error(message),
    };
    match parse_balance(provider, &body) {
        Ok(items) => UsageResult {
            success: true,
            data: (!items.is_empty()).then_some(items),
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
    fn detects_all_upstream_balance_hosts() {
        assert_eq!(
            detect_provider("https://api.deepseek.com"),
            Some(BalanceProvider::DeepSeek)
        );
        assert_eq!(
            detect_provider("https://api.stepfun.ai/v1"),
            Some(BalanceProvider::StepFun)
        );
        assert_eq!(
            detect_provider("https://api.siliconflow.cn/v1"),
            Some(BalanceProvider::SiliconFlowCn)
        );
        assert_eq!(
            detect_provider("https://api.siliconflow.com/v1"),
            Some(BalanceProvider::SiliconFlowEn)
        );
        assert_eq!(
            detect_provider("https://openrouter.ai/api/v1"),
            Some(BalanceProvider::OpenRouter)
        );
        assert_eq!(
            detect_provider("https://api.novita.ai/v3"),
            Some(BalanceProvider::Novita)
        );
    }

    #[test]
    fn parses_openrouter_and_novita_units() {
        let openrouter = parse_balance(
            BalanceProvider::OpenRouter,
            &json!({"data": {"total_credits": 10, "total_usage": 3.5}}),
        )
        .expect("openrouter balance");
        assert_eq!(openrouter[0].remaining, Some(6.5));
        let novita = parse_balance(
            BalanceProvider::Novita,
            &json!({"availableBalance": 125000}),
        )
        .expect("novita balance");
        assert_eq!(novita[0].remaining, Some(12.5));
    }
}
