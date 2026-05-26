#![cfg(any(feature = "web-server", feature = "desktop"))]

pub mod adapters;
pub mod live;
pub mod server;
pub mod service;
pub mod types;
pub mod usage;

pub use server::{
    clear_recent_logs, recent_logs, recent_logs_for_state, start_from_saved_settings, start_proxy,
    status, status_for_state, stop_proxy, test_settings,
};
pub use service::ProxyService;
pub use types::{ProxyRecentLog, ProxyStatus, ProxyTakeoverResult, ProxyTestResult};
