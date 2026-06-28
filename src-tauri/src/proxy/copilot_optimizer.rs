use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopilotClassification {
    pub initiator: &'static str,
    pub is_warmup: bool,
    pub is_compact: bool,
    pub is_subagent: bool,
}

pub fn classify_request(
    body: &Value,
    has_anthropic_beta: bool,
    compact_detection: bool,
    subagent_detection: bool,
) -> CopilotClassification {
    let is_compact = compact_detection && is_compact_request(body);
    let is_subagent = subagent_detection && detect_subagent(body);
    let messages = match body.get("messages").and_then(Value::as_array) {
        Some(messages) if !messages.is_empty() => messages,
        _ => {
            return CopilotClassification {
                initiator: "user",
                is_warmup: is_warmup_request(body, has_anthropic_beta, false),
                is_compact: false,
                is_subagent,
            }
        }
    };

    let Some(last_message) = messages.last() else {
        return CopilotClassification {
            initiator: "user",
            is_warmup: false,
            is_compact,
            is_subagent,
        };
    };
    let role = last_message
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if role != "user" {
        return CopilotClassification {
            initiator: if is_subagent { "agent" } else { "user" },
            is_warmup: false,
            is_compact,
            is_subagent,
        };
    }

    let user_initiated = match last_message.get("content") {
        Some(Value::Array(blocks)) => !blocks
            .iter()
            .any(|block| block.get("type").and_then(Value::as_str) == Some("tool_result")),
        Some(Value::String(_)) => true,
        _ => false,
    };
    let initiator = if is_subagent || !user_initiated || is_compact {
        "agent"
    } else {
        "user"
    };

    CopilotClassification {
        initiator,
        is_warmup: initiator == "user" && is_warmup_request(body, has_anthropic_beta, is_compact),
        is_compact,
        is_subagent,
    }
}

pub fn deterministic_request_id(body: &Value, session_id: &str) -> Option<String> {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        return None;
    }
    let content = find_last_user_content(body)
        .or_else(|| serde_json::to_string(body).ok())
        .filter(|value| !value.is_empty())?;
    let mut hasher = Sha256::new();
    hasher.update(session_id.as_bytes());
    hasher.update(content.as_bytes());
    Some(uuid_v4_like_from_hash(hasher.finalize().as_slice()))
}

pub fn deterministic_interaction_id(session_id: &str) -> Option<String> {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        return None;
    }
    let mut hasher = Sha256::new();
    hasher.update(b"interaction:");
    hasher.update(session_id.as_bytes());
    Some(uuid_v4_like_from_hash(hasher.finalize().as_slice()))
}

pub fn prepare_body_for_copilot(body: Value) -> Value {
    strip_thinking_blocks(sanitize_orphan_tool_results(merge_tool_results(body)))
}

pub fn merge_tool_results(mut body: Value) -> Value {
    let Some(messages) = body.get_mut("messages").and_then(Value::as_array_mut) else {
        return body;
    };

    for message in messages.iter_mut() {
        if message.get("role").and_then(Value::as_str) != Some("user") {
            continue;
        }
        let Some(blocks) = message.get("content").and_then(Value::as_array) else {
            continue;
        };

        let mut tool_results = Vec::new();
        let mut text_blocks = Vec::new();
        let mut valid = true;
        for block in blocks {
            match block.get("type").and_then(Value::as_str) {
                Some("tool_result") => tool_results.push(block.clone()),
                Some("text") => text_blocks.push(block.clone()),
                _ => {
                    valid = false;
                    break;
                }
            }
        }
        if !valid || tool_results.is_empty() || text_blocks.is_empty() {
            continue;
        }

        message["content"] = Value::Array(merge_text_blocks_into_tool_results(
            tool_results,
            text_blocks,
        ));
    }

    let Some(messages) = body.get("messages").and_then(Value::as_array) else {
        return body;
    };
    if messages.len() <= 1 {
        return body;
    }

    let mut merged = Vec::with_capacity(messages.len());
    let mut index = 0usize;
    while index < messages.len() {
        if is_tool_result_only_message(&messages[index]) {
            let mut content = Vec::new();
            while index < messages.len() && is_tool_result_only_message(&messages[index]) {
                if let Some(blocks) = messages[index].get("content").and_then(Value::as_array) {
                    content.extend(blocks.iter().cloned());
                }
                index += 1;
            }
            if !content.is_empty() {
                merged.push(serde_json::json!({
                    "role": "user",
                    "content": content
                }));
            }
        } else {
            merged.push(messages[index].clone());
            index += 1;
        }
    }
    body["messages"] = Value::Array(merged);
    body
}

pub fn sanitize_orphan_tool_results(mut body: Value) -> Value {
    let Some(messages) = body.get_mut("messages").and_then(Value::as_array_mut) else {
        return body;
    };
    if messages.len() < 2 {
        return body;
    }

    for index in 1..messages.len() {
        if messages[index].get("role").and_then(Value::as_str) != Some("user") {
            continue;
        }

        let previous_tool_use_ids =
            if messages[index - 1].get("role").and_then(Value::as_str) == Some("assistant") {
                messages[index - 1]
                    .get("content")
                    .and_then(Value::as_array)
                    .map(|blocks| {
                        blocks
                            .iter()
                            .filter(|block| {
                                block.get("type").and_then(Value::as_str) == Some("tool_use")
                            })
                            .filter_map(|block| block.get("id").and_then(Value::as_str))
                            .map(ToString::to_string)
                            .collect::<HashSet<_>>()
                    })
                    .unwrap_or_default()
            } else {
                HashSet::new()
            };

        let Some(blocks) = messages[index]
            .get_mut("content")
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        for block in blocks.iter_mut() {
            if block.get("type").and_then(Value::as_str) != Some("tool_result") {
                continue;
            }
            let tool_use_id = block
                .get("tool_use_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if tool_use_id.is_empty() || !previous_tool_use_ids.contains(tool_use_id) {
                let content = tool_result_content_as_text(block.get("content"));
                *block = serde_json::json!({
                    "type": "text",
                    "text": format!("[Tool result for {tool_use_id}]: {content}")
                });
            }
        }
    }

    body
}

pub fn strip_thinking_blocks(mut body: Value) -> Value {
    let Some(messages) = body.get_mut("messages").and_then(Value::as_array_mut) else {
        return body;
    };

    for message in messages {
        if message.get("role").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let Some(blocks) = message.get_mut("content").and_then(Value::as_array_mut) else {
            continue;
        };
        blocks.retain(|block| {
            !matches!(
                block.get("type").and_then(Value::as_str),
                Some("thinking") | Some("redacted_thinking")
            )
        });
    }

    body
}

fn is_warmup_request(body: &Value, has_anthropic_beta: bool, is_compact: bool) -> bool {
    if !has_anthropic_beta || is_compact {
        return false;
    }
    !matches!(
        body.get("tools").and_then(Value::as_array),
        Some(tools) if !tools.is_empty()
    )
}

fn is_compact_request(body: &Value) -> bool {
    if extract_system_text(body)
        .starts_with("You are a helpful AI assistant tasked with summarizing conversations")
    {
        return true;
    }

    let Some(messages) = body.get("messages").and_then(Value::as_array) else {
        return false;
    };
    let Some(last_message) = messages.last() else {
        return false;
    };
    if last_message.get("role").and_then(Value::as_str) != Some("user") {
        return false;
    }

    let text = extract_text_from_message(last_message);
    text.contains("CRITICAL: Respond with TEXT ONLY. Do NOT call any tools.")
        || (text.contains("Pending Tasks:") && text.contains("Current Work:"))
}

fn detect_subagent(body: &Value) -> bool {
    if extract_system_text(body).contains("__SUBAGENT_MARKER__") {
        return true;
    }
    if let Some(messages) = body.get("messages").and_then(Value::as_array) {
        for message in messages {
            if message.get("role").and_then(Value::as_str) != Some("user") {
                continue;
            }
            if extract_text_from_message(message).contains("__SUBAGENT_MARKER__") {
                return true;
            }
        }
    }
    body.pointer("/metadata/user_id")
        .and_then(Value::as_str)
        .is_some_and(|user_id| user_id.contains("_agent_"))
}

fn extract_system_text(body: &Value) -> String {
    match body.get("system") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

fn find_last_user_content(body: &Value) -> Option<String> {
    let messages = body.get("messages").and_then(Value::as_array)?;
    for message in messages.iter().rev() {
        if message.get("role").and_then(Value::as_str) != Some("user") {
            continue;
        }
        let content = message.get("content")?;
        if let Some(text) = content.as_str() {
            return Some(text.to_string());
        }
        if let Some(blocks) = content.as_array() {
            let filtered = blocks
                .iter()
                .filter(|block| block.get("type").and_then(Value::as_str) != Some("tool_result"))
                .map(|block| {
                    let mut block = block.clone();
                    if let Some(obj) = block.as_object_mut() {
                        obj.remove("cache_control");
                    }
                    block
                })
                .collect::<Vec<_>>();
            if !filtered.is_empty() {
                return serde_json::to_string(&filtered).ok();
            }
        }
    }
    None
}

fn extract_text_from_message(message: &Value) -> String {
    match message.get("content") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter_map(|block| {
                if block.get("type").and_then(Value::as_str) == Some("text") {
                    block.get("text").and_then(Value::as_str)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

fn merge_text_blocks_into_tool_results(
    mut tool_results: Vec<Value>,
    text_blocks: Vec<Value>,
) -> Vec<Value> {
    if tool_results.len() == text_blocks.len() {
        for (tool_result, text_block) in tool_results.iter_mut().zip(text_blocks.iter()) {
            append_text_to_tool_result(tool_result, text_block);
        }
    } else if let Some(last_tool_result) = tool_results.last_mut() {
        for text_block in &text_blocks {
            append_text_to_tool_result(last_tool_result, text_block);
        }
    }
    tool_results
}

fn append_text_to_tool_result(tool_result: &mut Value, text_block: &Value) {
    let text = text_block
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if text.trim().is_empty() {
        return;
    }

    match tool_result.get_mut("content") {
        Some(Value::String(existing)) => {
            existing.push('\n');
            existing.push_str(text);
        }
        Some(Value::Array(blocks)) => {
            blocks.push(serde_json::json!({ "type": "text", "text": text }));
        }
        _ => {
            tool_result["content"] = Value::String(text.to_string());
        }
    }
}

fn is_tool_result_only_message(message: &Value) -> bool {
    if message.get("role").and_then(Value::as_str) != Some("user") {
        return false;
    }
    matches!(
        message.get("content").and_then(Value::as_array),
        Some(blocks) if !blocks.is_empty()
            && blocks
                .iter()
                .all(|block| block.get("type").and_then(Value::as_str) == Some("tool_result"))
    )
}

fn tool_result_content_as_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        Some(value) => value.to_string(),
        None => String::new(),
    }
}

fn uuid_v4_like_from_hash(hash: &[u8]) -> String {
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&hash[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn classifies_tool_result_turn_as_agent() {
        let body = json!({
            "messages": [
                { "role": "assistant", "content": [{ "type": "tool_use", "id": "toolu_1", "name": "Read", "input": {} }] },
                { "role": "user", "content": [{ "type": "tool_result", "tool_use_id": "toolu_1", "content": "ok" }] }
            ]
        });

        let classification = classify_request(&body, true, true, true);

        assert_eq!(classification.initiator, "agent");
        assert!(!classification.is_warmup);
    }

    #[test]
    fn classifies_subagent_marker_as_agent() {
        let body = json!({
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "text",
                    "text": "<system-reminder>{\"__SUBAGENT_MARKER__\":{\"session_id\":\"s\"}}</system-reminder>"
                }]
            }]
        });

        let classification = classify_request(&body, false, true, true);

        assert_eq!(classification.initiator, "agent");
        assert!(classification.is_subagent);
    }

    #[test]
    fn deterministic_ids_are_stable_uuid_shaped() {
        let body = json!({
            "messages": [{ "role": "user", "content": "hello" }]
        });

        let one = deterministic_request_id(&body, "session-1").expect("request id");
        let two = deterministic_request_id(&body, "session-1").expect("request id");
        let interaction = deterministic_interaction_id("session-1").expect("interaction id");

        assert_eq!(one, two);
        assert_eq!(one.len(), 36);
        assert_eq!(interaction.len(), 36);
        assert_ne!(one, interaction);
    }

    #[test]
    fn prepare_body_merges_tool_result_text_and_strips_thinking() {
        let body = json!({
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        { "type": "thinking", "thinking": "hidden" },
                        { "type": "tool_use", "id": "toolu_1", "name": "Read", "input": {} }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        { "type": "tool_result", "tool_use_id": "toolu_1", "content": "file" },
                        { "type": "text", "text": "continue" }
                    ]
                }
            ]
        });

        let prepared = prepare_body_for_copilot(body);

        assert_eq!(
            prepared["messages"][0]["content"],
            json!([{ "type": "tool_use", "id": "toolu_1", "name": "Read", "input": {} }])
        );
        assert_eq!(
            prepared["messages"][1]["content"],
            json!([{ "type": "tool_result", "tool_use_id": "toolu_1", "content": "file\ncontinue" }])
        );
    }

    #[test]
    fn prepare_body_converts_orphan_tool_result_to_text() {
        let body = json!({
            "messages": [
                { "role": "user", "content": "hello" },
                {
                    "role": "user",
                    "content": [
                        { "type": "tool_result", "tool_use_id": "missing", "content": "orphan" }
                    ]
                }
            ]
        });

        let prepared = prepare_body_for_copilot(body);

        assert_eq!(
            prepared["messages"][1]["content"][0],
            json!({ "type": "text", "text": "[Tool result for missing]: orphan" })
        );
    }
}
