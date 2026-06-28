use super::types::OptimizerConfig;
use serde_json::{json, Value};

pub fn inject(body: &mut Value, config: &OptimizerConfig) {
    if !config.cache_injection {
        return;
    }

    let existing = count_existing(body);
    upgrade_existing_ttl(body, &config.cache_ttl);

    let mut budget = 4_usize.saturating_sub(existing);
    if budget == 0 {
        log::info!("[OPT] cache: no-op(existing={existing})");
        return;
    }

    let mut injected = Vec::new();

    if budget > 0 {
        if let Some(tools) = body.get_mut("tools").and_then(|t| t.as_array_mut()) {
            if let Some(last) = tools.last_mut() {
                if last.get("cache_control").is_none() {
                    if let Some(obj) = last.as_object_mut() {
                        obj.insert(
                            "cache_control".to_string(),
                            make_cache_control(&config.cache_ttl),
                        );
                    }
                    budget -= 1;
                    injected.push("tools");
                }
            }
        }
    }

    if budget > 0 {
        if body.get("system").and_then(|s| s.as_str()).is_some() {
            let text = body["system"].as_str().unwrap_or_default().to_string();
            body["system"] = json!([{ "type": "text", "text": text }]);
        }

        if let Some(system) = body.get_mut("system").and_then(|s| s.as_array_mut()) {
            if let Some(last) = system.last_mut() {
                if last.get("cache_control").is_none() {
                    if let Some(obj) = last.as_object_mut() {
                        obj.insert(
                            "cache_control".to_string(),
                            make_cache_control(&config.cache_ttl),
                        );
                    }
                    budget -= 1;
                    injected.push("system");
                }
            }
        }
    }

    if budget > 0 {
        if let Some(messages) = body.get_mut("messages").and_then(|m| m.as_array_mut()) {
            if let Some(assistant_msg) = messages
                .iter_mut()
                .rev()
                .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("assistant"))
            {
                if let Some(content) = assistant_msg
                    .get_mut("content")
                    .and_then(|c| c.as_array_mut())
                {
                    if let Some(block) = content.iter_mut().rev().find(|block| {
                        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        block_type != "thinking" && block_type != "redacted_thinking"
                    }) {
                        if block.get("cache_control").is_none() {
                            if let Some(obj) = block.as_object_mut() {
                                obj.insert(
                                    "cache_control".to_string(),
                                    make_cache_control(&config.cache_ttl),
                                );
                            }
                            injected.push("msgs");
                        }
                    }
                }
            }
        }
    }

    log::info!(
        "[OPT] cache: {}bp({},{},pre={existing})",
        injected.len(),
        injected.join("+"),
        config.cache_ttl,
    );
}

fn make_cache_control(ttl: &str) -> Value {
    if ttl == "5m" {
        json!({ "type": "ephemeral" })
    } else {
        json!({ "type": "ephemeral", "ttl": ttl })
    }
}

fn count_existing(body: &Value) -> usize {
    let mut count = 0;

    if let Some(tools) = body.get("tools").and_then(|t| t.as_array()) {
        count += tools
            .iter()
            .filter(|tool| tool.get("cache_control").is_some())
            .count();
    }

    if let Some(system) = body.get("system").and_then(|s| s.as_array()) {
        count += system
            .iter()
            .filter(|block| block.get("cache_control").is_some())
            .count();
    }

    if let Some(messages) = body.get("messages").and_then(|m| m.as_array()) {
        for msg in messages {
            if let Some(content) = msg.get("content").and_then(|c| c.as_array()) {
                count += content
                    .iter()
                    .filter(|block| block.get("cache_control").is_some())
                    .count();
            }
        }
    }

    count
}

fn upgrade_existing_ttl(body: &mut Value, ttl: &str) {
    let upgrade = |value: &mut Value| {
        if let Some(cache_control) = value
            .get_mut("cache_control")
            .and_then(|cache_control| cache_control.as_object_mut())
        {
            if ttl == "5m" {
                cache_control.remove("ttl");
            } else {
                cache_control.insert("ttl".to_string(), json!(ttl));
            }
        }
    };

    if let Some(tools) = body.get_mut("tools").and_then(|t| t.as_array_mut()) {
        for tool in tools {
            upgrade(tool);
        }
    }

    if let Some(system) = body.get_mut("system").and_then(|s| s.as_array_mut()) {
        for block in system {
            upgrade(block);
        }
    }

    if let Some(messages) = body.get_mut("messages").and_then(|m| m.as_array_mut()) {
        for msg in messages {
            if let Some(content) = msg.get_mut("content").and_then(|c| c.as_array_mut()) {
                for block in content {
                    upgrade(block);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> OptimizerConfig {
        OptimizerConfig {
            enabled: true,
            thinking_optimizer: true,
            cache_injection: true,
            cache_ttl: "1h".to_string(),
        }
    }

    #[test]
    fn injects_tools_system_and_last_assistant_non_thinking_breakpoints() {
        let mut body = json!({
            "model": "test",
            "tools": [{ "name": "tool1" }, { "name": "tool2" }],
            "system": [{ "type": "text", "text": "sys prompt" }],
            "messages": [
                { "role": "assistant", "content": [
                    { "type": "thinking", "thinking": "hmm" },
                    { "type": "text", "text": "result" },
                    { "type": "redacted_thinking", "data": "xxx" }
                ]}
            ]
        });

        inject(&mut body, &default_config());

        assert!(body["tools"][1].get("cache_control").is_some());
        assert!(body["system"][0].get("cache_control").is_some());
        assert!(body["messages"][0]["content"][1]
            .get("cache_control")
            .is_some());
        assert!(body["messages"][0]["content"][0]
            .get("cache_control")
            .is_none());
    }

    #[test]
    fn upgrades_existing_ttl_without_exceeding_four_breakpoints() {
        let mut body = json!({
            "tools": [
                { "name": "t1", "cache_control": { "type": "ephemeral", "ttl": "5m" } },
                { "name": "t2", "cache_control": { "type": "ephemeral", "ttl": "5m" } }
            ],
            "system": [
                { "type": "text", "text": "sys", "cache_control": { "type": "ephemeral", "ttl": "5m" } }
            ],
            "messages": [
                { "role": "assistant", "content": [
                    { "type": "text", "text": "ok", "cache_control": { "type": "ephemeral", "ttl": "5m" } }
                ]}
            ]
        });

        inject(&mut body, &default_config());

        assert_eq!(body["tools"][0]["cache_control"]["ttl"], "1h");
        assert_eq!(body["tools"][1]["cache_control"]["ttl"], "1h");
        assert_eq!(body["system"][0]["cache_control"]["ttl"], "1h");
        assert_eq!(
            body["messages"][0]["content"][0]["cache_control"]["ttl"],
            "1h"
        );
    }

    #[test]
    fn ttl_5m_omits_ttl_field() {
        let config = OptimizerConfig {
            cache_ttl: "5m".to_string(),
            ..default_config()
        };
        let mut body = json!({
            "tools": [{ "name": "tool1" }],
            "messages": [{ "role": "user", "content": [{ "type": "text", "text": "hi" }] }]
        });

        inject(&mut body, &config);

        let cache_control = &body["tools"][0]["cache_control"];
        assert_eq!(cache_control["type"], "ephemeral");
        assert!(cache_control.get("ttl").is_none() || cache_control["ttl"].is_null());
    }
}
