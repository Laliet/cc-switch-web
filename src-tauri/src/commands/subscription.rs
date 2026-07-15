#![allow(non_snake_case)]

use tauri::State;

use crate::services::{SubscriptionProvider, SubscriptionQuota, SubscriptionService};
use crate::store::AppState;

#[tauri::command]
pub async fn query_subscription_quota(
    state: State<'_, AppState>,
    provider: String,
    accountId: Option<String>,
    force: Option<bool>,
) -> Result<SubscriptionQuota, String> {
    let provider = SubscriptionProvider::parse(&provider).map_err(|e| e.to_string())?;
    SubscriptionService::query(
        &state.inner().db_state(),
        provider,
        accountId.as_deref(),
        force.unwrap_or(false),
    )
    .await
    .map_err(|e| e.to_string())
}
