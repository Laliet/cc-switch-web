use super::types::OptimizerConfig;
use serde_json::{json, Value};

pub fn optimize(body: &mut Value, config: &OptimizerConfig) {
    if !config.thinking_optimizer {
        return;
    }

    let model = match body.get("model").and_then(|m| m.as_str()) {
        Some(model) => model.to_lowercase(),
        None => return,
    };

    if model.contains("haiku") {
        log::info!("[OPT] thinking: skip(haiku)");
        return;
    }

    if model.contains("opus-4-7") || model.contains("opus-4-6") || model.contains("sonnet-4-6") {
        log::info!("[OPT] thinking: adaptive({model})");
        body["thinking"] = json!({ "type": "adaptive" });
        body["output_config"] = json!({ "effort": "max" });
        append_beta(body, "context-1m-2025-08-07");
        return;
    }

    log::info!("[OPT] thinking: legacy({model})");
    let max_tokens = body
        .get("max_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(16384);
    let budget_target = max_tokens.saturating_sub(1);
    let thinking_type = body
        .get("thinking")
        .and_then(|t| t.get("type"))
        .and_then(|t| t.as_str());

    match thinking_type {
        None | Some("disabled") => {
            body["thinking"] = json!({
                "type": "enabled",
                "budget_tokens": budget_target
            });
            append_beta(body, "interleaved-thinking-2025-05-14");
        }
        Some("enabled") => {
            let current_budget = body
                .get("thinking")
                .and_then(|t| t.get("budget_tokens"))
                .and_then(|b| b.as_u64())
                .unwrap_or(0);
            if current_budget < budget_target {
                body["thinking"]["budget_tokens"] = json!(budget_target);
            }
            append_beta(body, "interleaved-thinking-2025-05-14");
        }
        _ => append_beta(body, "interleaved-thinking-2025-05-14"),
    }
}

fn append_beta(body: &mut Value, beta: &str) {
    match body.get("anthropic_beta") {
        Some(Value::Array(arr)) => {
            if !arr.iter().any(|v| v.as_str() == Some(beta)) {
                body["anthropic_beta"]
                    .as_array_mut()
                    .expect("anthropic_beta array")
                    .push(json!(beta));
            }
        }
        Some(Value::Null) | None => {
            body["anthropic_beta"] = json!([beta]);
        }
        _ => {
            body["anthropic_beta"] = json!([beta]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled_config() -> OptimizerConfig {
        OptimizerConfig {
            enabled: true,
            thinking_optimizer: true,
            cache_injection: true,
            cache_ttl: "1h".to_string(),
        }
    }

    #[test]
    fn adaptive_models_use_adaptive_thinking_and_context_beta() {
        let mut body = json!({
            "model": "anthropic.claude-sonnet-4-6-20250514-v1:0",
            "max_tokens": 16384,
            "thinking": { "type": "enabled", "budget_tokens": 8000 },
            "messages": [{ "role": "user", "content": "hello" }]
        });

        optimize(&mut body, &enabled_config());

        assert_eq!(body["thinking"]["type"], "adaptive");
        assert!(body["thinking"].get("budget_tokens").is_none());
        assert_eq!(body["output_config"]["effort"], "max");
        assert!(body["anthropic_beta"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "context-1m-2025-08-07"));
    }

    #[test]
    fn legacy_models_inject_enabled_thinking_and_interleaved_beta() {
        let mut body = json!({
            "model": "anthropic.claude-sonnet-4-5-20250514-v1:0",
            "max_tokens": 8192,
            "thinking": { "type": "disabled" },
            "messages": [{ "role": "user", "content": "hello" }]
        });

        optimize(&mut body, &enabled_config());

        assert_eq!(body["thinking"]["type"], "enabled");
        assert_eq!(body["thinking"]["budget_tokens"], 8191);
        assert!(body["anthropic_beta"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "interleaved-thinking-2025-05-14"));
    }

    #[test]
    fn haiku_models_are_not_modified() {
        let mut body = json!({
            "model": "anthropic.claude-haiku-4-5-20250514-v1:0",
            "max_tokens": 8192,
            "messages": [{ "role": "user", "content": "hello" }]
        });
        let original = body.clone();

        optimize(&mut body, &enabled_config());

        assert_eq!(body, original);
    }
}
