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
                    }
                }
                _ => {}
            }
        }
        usage.has_tokens().then_some(usage)
    }

    fn from_codex_response_auto(body: &Value) -> Option<Self> {
        let usage = body.get("usage")?;
        if usage.get("prompt_tokens").is_some() {
            Self::from_openai_response(body)
        } else if usage.get("input_tokens").is_some() {
            Some(Self {
                input_tokens: usage.get("input_tokens")?.as_u64()? as u32,
                output_tokens: usage.get("output_tokens")?.as_u64()? as u32,
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
            })
        } else {
            None
        }
    }

    fn from_codex_stream_events_auto(events: &[Value]) -> Option<Self> {
        for event in events {
            if event.get("type").and_then(|v| v.as_str()) == Some("response.completed") {
                if let Some(response) = event.get("response") {
                    return Self::from_codex_response_auto(response);
                }
            }
        }
        Self::from_openai_stream_events(events)
    }

    fn from_openai_response(body: &Value) -> Option<Self> {
        let usage = body.get("usage")?;
        Some(Self {
            input_tokens: usage.get("prompt_tokens")?.as_u64()? as u32,
            output_tokens: usage.get("completion_tokens")?.as_u64()? as u32,
            cache_read_tokens: usage
                .get("prompt_tokens_details")
                .and_then(|d| d.get("cached_tokens"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            cache_creation_tokens: 0,
            model: body
                .get("model")
                .and_then(|v| v.as_str())
                .map(ToString::to_string),
            message_id: None,
        })
    }

    fn from_openai_stream_events(events: &[Value]) -> Option<Self> {
        events
            .iter()
            .rev()
            .find(|event| event.get("usage").is_some_and(|usage| !usage.is_null()))
            .and_then(Self::from_openai_response)
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
        for event in events {
            if let Some(metadata) = event.get("usageMetadata") {
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
        usage.has_tokens().then_some(usage)
    }

    fn has_tokens(&self) -> bool {
        self.input_tokens > 0
            || self.output_tokens > 0
            || self.cache_read_tokens > 0
            || self.cache_creation_tokens > 0
    }
}
