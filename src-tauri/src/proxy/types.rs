use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::{app_config::AppType, provider::Provider};

pub const PROXY_BODY_LIMIT_BYTES: usize = 32 * 1024 * 1024;
pub const PROXY_MANAGED_TOKEN: &str = "PROXY_MANAGED";

fn default_true() -> bool {
    true
}

fn default_cache_ttl() -> String {
    "1h".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub thinking_optimizer: bool,
    #[serde(default = "default_true")]
    pub cache_injection: bool,
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl: String,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            thinking_optimizer: true,
            cache_injection: true,
            cache_ttl: default_cache_ttl(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyStatus {
    pub running: bool,
    pub address: String,
    pub port: u16,
    pub listen_url: Option<String>,
    pub active_connections: u64,
    pub total_requests: u64,
    pub success_requests: u64,
    pub failed_requests: u64,
    pub success_rate: f64,
    pub uptime_seconds: u64,
    pub active_targets: Vec<ProxyActiveTarget>,
    pub takeover: ProxyTakeoverStatus,
    pub bind_app: String,
    pub last_request_at: Option<String>,
    pub last_error: Option<String>,
    pub failover_count: u64,
    pub last_failover_at: Option<String>,
    pub last_failover_from: Option<String>,
    pub last_failover_to: Option<String>,
    #[serde(default)]
    pub provider_health: Vec<ProxyProviderHealth>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyActiveTarget {
    pub app_type: String,
    pub provider_id: String,
    pub provider_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyProviderHealth {
    pub app_type: String,
    pub provider_id: String,
    pub state: String,
    pub failure_count: u64,
    pub recovery_success_count: u64,
    pub window_requests: u64,
    pub window_failures: u64,
    pub last_failure_seconds_ago: Option<u64>,
    pub opened_seconds_ago: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProxyTakeoverStatus {
    pub claude: bool,
    pub codex: bool,
    pub gemini: bool,
    pub opencode: bool,
    pub omo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyTestResult {
    pub success: bool,
    pub message: String,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyTakeoverResult {
    pub app: String,
    pub enabled: bool,
    pub status: ProxyStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyRecentLog {
    pub at: String,
    pub app: String,
    pub method: String,
    pub path: String,
    pub status: Option<u16>,
    pub duration_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TargetProvider {
    pub app: AppType,
    pub provider: Provider,
}

#[derive(Debug, Clone, Default)]
pub struct ProxyStats {
    pub started_at: Option<Instant>,
    pub active_connections: u64,
    pub total_requests: u64,
    pub success_requests: u64,
    pub failed_requests: u64,
    pub last_request_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_error: Option<String>,
    pub failover_count: u64,
    pub last_failover_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_failover_from: Option<String>,
    pub last_failover_to: Option<String>,
}

impl ProxyStats {
    pub fn uptime(&self) -> Duration {
        self.started_at
            .map(|started| started.elapsed())
            .unwrap_or_default()
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            (self.success_requests as f64 / self.total_requests as f64 * 1000.0).round() / 10.0
        }
    }
}
