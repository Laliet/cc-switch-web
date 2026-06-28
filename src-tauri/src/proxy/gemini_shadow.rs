use serde_json::Value;
use std::{
    collections::{HashMap, VecDeque},
    sync::RwLock,
};

#[derive(Debug, Clone, PartialEq)]
pub struct GeminiToolCallMeta {
    pub id: Option<String>,
    pub name: String,
    pub args: Value,
    pub thought_signature: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeminiAssistantTurn {
    pub assistant_content: Value,
    pub tool_calls: Vec<GeminiToolCallMeta>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GeminiShadowKey {
    provider_id: String,
    session_id: String,
}

#[derive(Debug)]
pub struct GeminiShadowStore {
    max_sessions: usize,
    max_turns_per_session: usize,
    sessions: RwLock<HashMap<GeminiShadowKey, VecDeque<GeminiAssistantTurn>>>,
    order: RwLock<VecDeque<GeminiShadowKey>>,
}

impl Default for GeminiShadowStore {
    fn default() -> Self {
        Self::with_limits(200, 64)
    }
}

impl GeminiShadowStore {
    pub fn with_limits(max_sessions: usize, max_turns_per_session: usize) -> Self {
        Self {
            max_sessions: max_sessions.max(1),
            max_turns_per_session: max_turns_per_session.max(1),
            sessions: RwLock::new(HashMap::new()),
            order: RwLock::new(VecDeque::new()),
        }
    }

    pub fn record_assistant_turn(
        &self,
        provider_id: impl Into<String>,
        session_id: impl Into<String>,
        assistant_content: Value,
        tool_calls: Vec<GeminiToolCallMeta>,
    ) {
        let key = GeminiShadowKey {
            provider_id: provider_id.into(),
            session_id: session_id.into(),
        };

        {
            let mut sessions = self.sessions.write().expect("gemini shadow lock poisoned");
            let turns = sessions.entry(key.clone()).or_default();
            turns.push_back(GeminiAssistantTurn {
                assistant_content,
                tool_calls,
            });
            while turns.len() > self.max_turns_per_session {
                turns.pop_front();
            }
        }

        {
            let mut order = self
                .order
                .write()
                .expect("gemini shadow order lock poisoned");
            if let Some(index) = order.iter().position(|existing| existing == &key) {
                order.remove(index);
            }
            order.push_back(key);
            while order.len() > self.max_sessions {
                if let Some(oldest) = order.pop_front() {
                    self.sessions
                        .write()
                        .expect("gemini shadow lock poisoned")
                        .remove(&oldest);
                }
            }
        }
    }

    pub fn get_session_turns(
        &self,
        provider_id: &str,
        session_id: &str,
    ) -> Vec<GeminiAssistantTurn> {
        let key = GeminiShadowKey {
            provider_id: provider_id.to_string(),
            session_id: session_id.to_string(),
        };
        let turns: Vec<GeminiAssistantTurn> = self
            .sessions
            .read()
            .expect("gemini shadow lock poisoned")
            .get(&key)
            .map(|turns| turns.iter().cloned().collect())
            .unwrap_or_default();

        if !turns.is_empty() {
            let mut order = self
                .order
                .write()
                .expect("gemini shadow order lock poisoned");
            if let Some(index) = order.iter().position(|existing| existing == &key) {
                order.remove(index);
            }
            order.push_back(key);
        }

        turns
    }
}
