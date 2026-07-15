#![cfg(any(feature = "web-server", feature = "desktop"))]

pub mod adapters;
pub mod body_filter;
pub mod cache_injector;
pub mod copilot_optimizer;
pub mod gemini_schema;
pub mod gemini_shadow;
pub mod live;
pub mod server;
pub mod service;
pub mod thinking_budget_rectifier;
pub mod thinking_optimizer;
pub mod thinking_rectifier;
pub mod types;
pub mod usage;

pub use server::{
    clear_recent_logs, parse_proxy_app, recent_logs, recent_logs_for_state, reset_provider_circuit,
    start_from_saved_settings, start_proxy, status, status_for_state, stop_proxy, test_settings,
};
pub use service::ProxyService;
pub use types::{ProxyRecentLog, ProxyStatus, ProxyTakeoverResult, ProxyTestResult};
