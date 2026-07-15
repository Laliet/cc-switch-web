use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThinkingRectifyResult {
    pub applied: bool,
    pub removed_thinking_blocks: usize,
    pub removed_redacted_thinking_blocks: usize,
    pub removed_signature_fields: usize,
}

pub fn should_rectify_thinking_signature(error_message: Option<&str>, enabled: bool) -> bool {
    if !enabled {
        return false;
    }
    let Some(message) = error_message else {
        return false;
    };
    let lower = message.to_lowercase();

    if lower.contains("invalid")
        && lower.contains("signature")
        && lower.contains("thinking")
        && lower.contains("block")
    {
        return true;
    }
    if lower.contains("thought signature")
        && (lower.contains("not valid") || lower.contains("invalid"))
    {
        return true;
    }
    if lower.contains("must start with a thinking block") {
        return true;
    }
    if lower.contains("expected")
        && (lower.contains("thinking") || lower.contains("redacted_thinking"))
        && lower.contains("found")
        && lower.contains("tool_use")
    {
        return true;
    }
    if lower.contains("signature") && lower.contains("field required") {
        return true;
    }
    if lower.contains("signature") && lower.contains("extra inputs are not permitted") {
        return true;
    }
    if (lower.contains("thinking") || lower.contains("redacted_thinking"))
        && lower.contains("cannot be modified")
    {
        return true;
    }
    lower.contains("非法请求")
        || lower.contains("illegal request")
        || lower.contains("invalid request")
}

pub fn rectify_anthropic_request(body: &mut Value) -> ThinkingRectifyResult {
    let mut result = ThinkingRectifyResult::default();

    let Some(messages) = body.get_mut("messages").and_then(Value::as_array_mut) else {
        return result;
    };

    for message in messages.iter_mut() {
        let Some(content) = message.get_mut("content").and_then(Value::as_array_mut) else {
            continue;
        };
        let mut new_content = Vec::with_capacity(content.len());
        let mut changed = false;

        for block in content.iter() {
            match block.get("type").and_then(Value::as_str) {
                Some("thinking") => {
                    result.removed_thinking_blocks += 1;
                    changed = true;
                    continue;
                }
                Some("redacted_thinking") => {
                    result.removed_redacted_thinking_blocks += 1;
                    changed = true;
                    continue;
                }
                _ => {}
            }

            if block.get("signature").is_some() {
                let mut block = block.clone();
                if let Some(object) = block.as_object_mut() {
                    object.remove("signature");
                    result.removed_signature_fields += 1;
                    changed = true;
                }
                new_content.push(block);
            } else {
                new_content.push(block.clone());
            }
        }

        if changed {
            result.applied = true;
            *content = new_content;
        }
    }

    let messages_snapshot = body
        .get("messages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let removed_historical_thinking =
        result.removed_thinking_blocks > 0 || result.removed_redacted_thinking_blocks > 0;
    if removed_historical_thinking || should_remove_top_level_thinking(body, &messages_snapshot) {
        if let Some(object) = body.as_object_mut() {
            if object.remove("thinking").is_some() {
                result.applied = true;
            }
        }
    }

    result
}

fn should_remove_top_level_thinking(body: &Value, messages: &[Value]) -> bool {
    let thinking_enabled = body
        .get("thinking")
        .and_then(|thinking| thinking.get("type"))
        .and_then(Value::as_str)
        == Some("enabled");
    if !thinking_enabled {
        return false;
    }

    let Some(last_assistant_content) = messages
        .iter()
        .rev()
        .find(|message| message.get("role").and_then(Value::as_str) == Some("assistant"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_array)
        .filter(|content| !content.is_empty())
    else {
        return false;
    };

    let first_block_type = last_assistant_content
        .first()
        .and_then(|block| block.get("type"))
        .and_then(Value::as_str);
    if first_block_type == Some("thinking") || first_block_type == Some("redacted_thinking") {
        return false;
    }

    last_assistant_content
        .iter()
        .any(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
}
