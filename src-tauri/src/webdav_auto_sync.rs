use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::store::AppState;

const AUTO_SYNC_DEBOUNCE_MS: u64 = 1_000;
const MAX_AUTO_SYNC_WAIT_MS: u64 = 10_000;

static DB_CHANGE_TX: OnceLock<Sender<String>> = OnceLock::new();
static AUTO_SYNC_SUPPRESS_DEPTH: AtomicUsize = AtomicUsize::new(0);

pub(crate) struct AutoSyncSuppressionGuard;

impl AutoSyncSuppressionGuard {
    pub(crate) fn new() -> Self {
        AUTO_SYNC_SUPPRESS_DEPTH.fetch_add(1, Ordering::SeqCst);
        Self
    }
}

impl Drop for AutoSyncSuppressionGuard {
    fn drop(&mut self) {
        let _ =
            AUTO_SYNC_SUPPRESS_DEPTH.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |depth| {
                Some(depth.saturating_sub(1))
            });
    }
}

pub(crate) fn is_auto_sync_suppressed() -> bool {
    AUTO_SYNC_SUPPRESS_DEPTH.load(Ordering::SeqCst) > 0
}

pub(crate) fn should_trigger_for_table(table: &str) -> bool {
    matches!(
        table.trim().to_ascii_lowercase().as_str(),
        "providers"
            | "provider_endpoints"
            | "mcp_servers"
            | "prompts"
            | "skills"
            | "skill_repos"
            | "skill_states"
            | "settings"
            | "proxy_config"
            | "failover_queue"
            | "universal_providers"
    )
}

pub(crate) fn notify_db_changed(table: &str) {
    if is_auto_sync_suppressed() || !should_trigger_for_table(table) {
        return;
    }
    let Some(sender) = DB_CHANGE_TX.get() else {
        return;
    };
    match sender.try_send(table.to_string()) {
        Ok(()) | Err(TrySendError::Full(_)) => {}
        Err(TrySendError::Closed(_)) => log::warn!("WebDAV auto-sync worker is not available"),
    }
}

pub(crate) fn start_worker(state: Arc<AppState>) {
    if DB_CHANGE_TX.get().is_some() {
        return;
    }
    let (sender, receiver) = channel::<String>(1);
    if DB_CHANGE_TX.set(sender).is_err() {
        return;
    }
    tokio::spawn(run_worker_loop(state, receiver));
}

async fn run_worker_loop(state: Arc<AppState>, mut receiver: Receiver<String>) {
    while let Some(first_table) = receiver.recv().await {
        let started_at = Instant::now();
        let mut merged = 1usize;
        loop {
            let elapsed = started_at.elapsed();
            if elapsed >= Duration::from_millis(MAX_AUTO_SYNC_WAIT_MS) {
                break;
            }
            let wait = Duration::from_millis(AUTO_SYNC_DEBOUNCE_MS)
                .min(Duration::from_millis(MAX_AUTO_SYNC_WAIT_MS) - elapsed);
            match tokio::time::timeout(wait, receiver.recv()).await {
                Ok(Some(_)) => merged += 1,
                Ok(None) => return,
                Err(_) => break,
            }
        }
        log::debug!("WebDAV auto-sync triggered by table={first_table}, merged_changes={merged}");
        if let Err(error) = crate::webdav_sync::auto_sync_upload_if_enabled(&state).await {
            log::warn!("WebDAV change-triggered auto-sync failed: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_configuration_tables_trigger_sync() {
        assert!(should_trigger_for_table("providers"));
        assert!(should_trigger_for_table("universal_providers"));
        assert!(!should_trigger_for_table("proxy_request_logs"));
        assert!(!should_trigger_for_table("provider_health"));
        assert!(!should_trigger_for_table("usage_daily_rollups"));
        assert!(!should_trigger_for_table("session_log_sync"));
    }

    #[test]
    fn suppression_guard_is_scoped_and_nestable() {
        assert!(!is_auto_sync_suppressed());
        let first = AutoSyncSuppressionGuard::new();
        assert!(is_auto_sync_suppressed());
        {
            let _second = AutoSyncSuppressionGuard::new();
            assert!(is_auto_sync_suppressed());
        }
        assert!(is_auto_sync_suppressed());
        drop(first);
        assert!(!is_auto_sync_suppressed());
    }
}
