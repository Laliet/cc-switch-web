use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_creation_tokens: u32,
    pub model: Option<String>,
    #[serde(skip)]
    pub message_id: Option<String>,
}

impl TokenUsage {
    pub fn from_response(app_type: &str, body: &Value) -> Option<Self> {
        match app_type {
            "claude" => Self::from_claude_response(body),
            "codex" | "opencode" => Self::from_codex_response_auto(body),
            "gemini" => Self::from_gemini_response(body),
            _ => Self::from_openai_response(body),
        }
    }

    pub fn from_stream_events(app_type: &str, events: &[Value]) -> Option<Self> {
        match app_type {
            "claude" => Self::from_claude_stream_events(events),
            "codex" | "opencode" => Self::from_codex_stream_events_auto(events),
            "gemini" => Self::from_gemini_stream_chunks(events),
            _ => Self::from_openai_stream_events(events),
        }
    }

    fn from_claude_response(body: &Value) -> Option<Self> {
        let usage = body.get("usage")?;
        Some(Self {
            input_tokens: usage.get("input_tokens")?.as_u64()? as u32,
            output_tokens: usage.get("output_tokens")?.as_u64()? as u32,
            cache_read_tokens: usage
                .get("cache_read_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            cache_creation_tokens: usage
                .get("cache_creation_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            model: body
                .get("model")
                .and_then(|v| v.as_str())
                .map(ToString::to_string),
            message_id: body
                .get("id")
                .and_then(|v| v.as_str())
                .map(ToString::to_string),
        })
    }

    fn from_claude_stream_events(events: &[Value]) -> Option<Self> {
        let mut usage = Self::default();
        let mut saw_usage_fields = false;
        for event in events {
            match event.get("type").and_then(|v| v.as_str()) {
                Some("message_start") => {
                    if let Some(message) = event.get("message") {
                        if usage.model.is_none() {
                            usage.model = message
                                .get("model")
                                .and_then(|v| v.as_str())
                                .map(ToString::to_string);
                        }
                        if usage.message_id.is_none() {
                            usage.message_id = message
                                .get("id")
                                .and_then(|v| v.as_str())
                                .map(ToString::to_string);
                        }
                        if let Some(msg_usage) = message.get("usage") {
                            saw_usage_fields |= has_any_usage_field(
                                msg_usage,
                                &[
                                    "input_tokens",
                                    "output_tokens",
                                    "cache_read_input_tokens",
                                    "cache_creation_input_tokens",
                                ],
                            );
                            usage.input_tokens = msg_usage
                                .get("input_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0)
                                as u32;
                            usage.cache_read_tokens = msg_usage
                                .get("cache_read_input_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0)
                                as u32;
                            usage.cache_creation_tokens = msg_usage
                                .get("cache_creation_input_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0)
                                as u32;
                        }
                    }
                }
                Some("message_delta") => {
                    if let Some(delta_usage) = event.get("usage") {
                        saw_usage_fields |= has_any_usage_field(
                            delta_usage,
                            &[
                                "input_tokens",
                                "output_tokens",
                                "cache_read_input_tokens",
                                "cache_creation_input_tokens",
                            ],
                        );
                        if let Some(output) =
                            delta_usage.get("output_tokens").and_then(|v| v.as_u64())
                        {
                            usage.output_tokens = output as u32;
                        }
                        if usage.input_tokens == 0 {
                            usage.input_tokens = delta_usage
                                .get("input_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0)
                                as u32;
                        }
                        if usage.cache_read_tokens == 0 {
                            usage.cache_read_tokens = delta_usage
                                .get("cache_read_input_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0)
                                as u32;
                        }
                        if usage.cache_creation_tokens == 0 {
                            usage.cache_creation_tokens = delta_usage
                                .get("cache_creation_input_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0)
                                as u32;
                        }
                    }
                }
                _ => {}
            }
        }
        (saw_usage_fields || usage.has_tokens()).then_some(usage)
    }

    fn from_codex_response_auto(body: &Value) -> Option<Self> {
        let usage = body.get("usage")?;
        if usage.get("prompt_tokens").is_some() {
            Self::from_openai_response(body)
        } else if usage.get("input_tokens").is_some() {
            let input_tokens = usage.get("input_tokens").and_then(|v| v.as_u64())? as u32;
            let output_tokens = usage
                .get("output_tokens")
                .and_then(|v| v.as_u64())
                .or_else(|| usage.get("completion_tokens").and_then(|v| v.as_u64()))
                .unwrap_or(0) as u32;
            let parsed = Self {
                input_tokens,
                output_tokens,
                cache_read_tokens: usage
                    .get("cache_read_input_tokens")
                    .and_then(|v| v.as_u64())
                    .or_else(|| {
                        usage
                            .get("input_tokens_details")
                            .and_then(|d| d.get("cached_tokens"))
                            .and_then(|v| v.as_u64())
                    })
                    .unwrap_or(0) as u32,
                cache_creation_tokens: usage
                    .get("cache_creation_input_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                model: body
                    .get("model")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string),
                message_id: None,
            };
            Some(parsed)
        } else {
            None
        }
    }

    fn from_codex_stream_events_auto(events: &[Value]) -> Option<Self> {
        for event in events {
            if event.get("type").and_then(|v| v.as_str()) == Some("response.completed") {
                if let Some(response) = event.get("response") {
                    if let Some(usage) = Self::from_codex_response_auto(response) {
                        return Some(usage);
                    }
                }
            }
        }
        Self::from_openai_stream_events(events)
    }

    fn from_openai_response(body: &Value) -> Option<Self> {
        let usage = body.get("usage")?;
        let input_tokens = usage
            .get("prompt_tokens")
            .and_then(|v| v.as_u64())
            .or_else(|| usage.get("input_tokens").and_then(|v| v.as_u64()))?
            as u32;
        let output_tokens = usage
            .get("completion_tokens")
            .and_then(|v| v.as_u64())
            .or_else(|| usage.get("output_tokens").and_then(|v| v.as_u64()))
            .unwrap_or(0) as u32;
        let parsed = Self {
            input_tokens,
            output_tokens,
            cache_read_tokens: usage
                .get("prompt_tokens_details")
                .and_then(|d| d.get("cached_tokens"))
                .and_then(|v| v.as_u64())
                .or_else(|| {
                    usage
                        .get("input_tokens_details")
                        .and_then(|d| d.get("cached_tokens"))
                        .and_then(|v| v.as_u64())
                })
                .or_else(|| {
                    usage
                        .get("cache_read_input_tokens")
                        .and_then(|v| v.as_u64())
                })
                .unwrap_or(0) as u32,
            cache_creation_tokens: usage
                .get("cache_creation_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            model: body
                .get("model")
                .and_then(|v| v.as_str())
                .map(ToString::to_string),
            message_id: None,
        };
        Some(parsed)
    }

    fn from_openai_stream_events(events: &[Value]) -> Option<Self> {
        events.iter().rev().find_map(|event| {
            if event.get("usage").is_some_and(|usage| !usage.is_null()) {
                return Self::from_openai_response(event);
            }
            None
        })
    }

    fn from_gemini_response(body: &Value) -> Option<Self> {
        let usage = body.get("usageMetadata")?;
        let input_tokens = usage.get("promptTokenCount")?.as_u64()? as u32;
        let total_tokens = usage.get("totalTokenCount")?.as_u64()? as u32;
        Some(Self {
            input_tokens,
            output_tokens: total_tokens.saturating_sub(input_tokens),
            cache_read_tokens: usage
                .get("cachedContentTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            cache_creation_tokens: 0,
            model: body
                .get("modelVersion")
                .and_then(|v| v.as_str())
                .map(ToString::to_string),
            message_id: None,
        })
    }

    fn from_gemini_stream_chunks(events: &[Value]) -> Option<Self> {
        let mut usage = Self::default();
        let mut total_tokens = 0u32;
        let mut saw_usage_fields = false;
        for event in events {
            if let Some(metadata) = event.get("usageMetadata") {
                saw_usage_fields |= has_any_usage_field(
                    metadata,
                    &[
                        "promptTokenCount",
                        "totalTokenCount",
                        "cachedContentTokenCount",
                    ],
                );
                usage.input_tokens = metadata
                    .get("promptTokenCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                total_tokens = metadata
                    .get("totalTokenCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                usage.cache_read_tokens = metadata
                    .get("cachedContentTokenCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
            }
            if usage.model.is_none() {
                usage.model = event
                    .get("modelVersion")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);
            }
        }
        usage.output_tokens = total_tokens.saturating_sub(usage.input_tokens);
        (saw_usage_fields || usage.has_tokens()).then_some(usage)
    }

    fn has_tokens(&self) -> bool {
        self.input_tokens > 0
            || self.output_tokens > 0
            || self.cache_read_tokens > 0
            || self.cache_creation_tokens > 0
    }
}

fn has_any_usage_field(usage: &Value, fields: &[&str]) -> bool {
    fields.iter().any(|field| usage.get(*field).is_some())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::TokenUsage;

    #[test]
    fn parses_openai_responses_usage_with_partial_fields() {
        let body = json!({
            "model": "gpt-5.1-codex",
            "usage": {
                "input_tokens": 120,
                "input_tokens_details": { "cached_tokens": 40 }
            }
        });

        let usage = TokenUsage::from_response("codex", &body).expect("usage");
        assert_eq!(usage.input_tokens, 120);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.cache_read_tokens, 40);
        assert_eq!(usage.model.as_deref(), Some("gpt-5.1-codex"));
    }

    #[test]
    fn ignores_null_openai_responses_usage_without_panicking() {
        let body = json!({
            "model": "gpt-5.1-codex",
            "usage": null
        });

        assert!(TokenUsage::from_response("codex", &body).is_none());
    }

    #[test]
    fn ignores_empty_openai_responses_usage_without_panicking() {
        let body = json!({
            "model": "gpt-5.1-codex",
            "usage": {}
        });

        assert!(TokenUsage::from_response("codex", &body).is_none());
    }

    #[test]
    fn distinguishes_missing_usage_from_valid_all_zero_usage() {
        assert!(TokenUsage::from_response("codex", &json!({ "model": "gpt-5.1" })).is_none());

        let codex = TokenUsage::from_response(
            "codex",
            &json!({
                "model": "gpt-5.1",
                "usage": { "input_tokens": 0, "output_tokens": 0 }
            }),
        )
        .expect("zero Codex usage remains observable");
        assert_eq!(codex.input_tokens, 0);
        assert_eq!(codex.output_tokens, 0);
        assert_eq!(codex.model.as_deref(), Some("gpt-5.1"));

        let claude = TokenUsage::from_response(
            "claude",
            &json!({
                "model": "claude-sonnet-4-6",
                "usage": { "input_tokens": 0, "output_tokens": 0 }
            }),
        )
        .expect("zero Claude usage remains observable");
        assert_eq!(claude.input_tokens, 0);
        assert_eq!(claude.output_tokens, 0);

        let gemini = TokenUsage::from_response(
            "gemini",
            &json!({
                "modelVersion": "gemini-2.5-pro",
                "usageMetadata": { "promptTokenCount": 0, "totalTokenCount": 0 }
            }),
        )
        .expect("zero Gemini usage remains observable");
        assert_eq!(gemini.input_tokens, 0);
        assert_eq!(gemini.output_tokens, 0);
    }

    #[test]
    fn stream_usage_keeps_explicit_zero_fields_but_ignores_empty_objects() {
        let zero = TokenUsage::from_stream_events(
            "claude",
            &[json!({
                "type": "message_delta",
                "usage": { "input_tokens": 0, "output_tokens": 0 }
            })],
        )
        .expect("explicit zero stream usage");
        assert_eq!(zero.input_tokens, 0);
        assert_eq!(zero.output_tokens, 0);

        assert!(
            TokenUsage::from_stream_events("gemini", &[json!({ "usageMetadata": {} })]).is_none()
        );
    }

    #[test]
    fn openai_stream_usage_skips_trailing_empty_usage_event() {
        let events = vec![
            json!({
                "usage": {
                    "input_tokens": 12,
                    "output_tokens": 3,
                    "input_tokens_details": { "cached_tokens": 2 }
                }
            }),
            json!({ "usage": {} }),
        ];

        let usage = TokenUsage::from_stream_events("claude-desktop", &events).expect("usage");
        assert_eq!(usage.input_tokens, 12);
        assert_eq!(usage.output_tokens, 3);
        assert_eq!(usage.cache_read_tokens, 2);
    }

    #[test]
    fn claude_stream_usage_reads_cache_tokens_from_message_delta() {
        let events = vec![
            json!({
                "type": "message_start",
                "message": {
                    "id": "msg_1",
                    "model": "claude-sonnet-4-6",
                    "usage": {
                        "input_tokens": 0
                    }
                }
            }),
            json!({
                "type": "message_delta",
                "usage": {
                    "input_tokens": 12,
                    "output_tokens": 3,
                    "cache_read_input_tokens": 2,
                    "cache_creation_input_tokens": 4
                }
            }),
        ];

        let usage = TokenUsage::from_stream_events("claude", &events).expect("usage");
        assert_eq!(usage.input_tokens, 12);
        assert_eq!(usage.output_tokens, 3);
        assert_eq!(usage.cache_read_tokens, 2);
        assert_eq!(usage.cache_creation_tokens, 4);
        assert_eq!(usage.model.as_deref(), Some("claude-sonnet-4-6"));
        assert_eq!(usage.message_id.as_deref(), Some("msg_1"));
    }

    #[test]
    fn codex_stream_usage_skips_empty_completed_response_and_uses_later_valid_event() {
        let events = vec![
            json!({
                "type": "response.completed",
                "response": {
                    "usage": null
                }
            }),
            json!({
                "type": "response.completed",
                "response": {
                    "model": "gpt-5.1-codex",
                    "usage": {
                        "input_tokens": 20,
                        "output_tokens": 5
                    }
                }
            }),
        ];

        let usage = TokenUsage::from_stream_events("codex", &events).expect("usage");
        assert_eq!(usage.input_tokens, 20);
        assert_eq!(usage.output_tokens, 5);
        assert_eq!(usage.model.as_deref(), Some("gpt-5.1-codex"));
    }
}
