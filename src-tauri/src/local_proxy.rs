#![cfg(any(feature = "web-server", feature = "desktop"))]

pub use crate::proxy::{
    start_from_saved_settings, start_proxy, status, stop_proxy, test_settings, ProxyStatus,
    ProxyTestResult,
};
