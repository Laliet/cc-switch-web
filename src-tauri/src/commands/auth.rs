use tauri::State;

use crate::{
    auth::{
        ManagedAuthAccount, ManagedAuthAccountInput, ManagedAuthDevicePoll,
        ManagedAuthDevicePollResult, ManagedAuthDeviceSession, ManagedAuthDeviceStart,
        ManagedAuthProvider, ManagedAuthUsage,
    },
    services::AuthService,
    store::AppState,
};

#[tauri::command(rename_all = "camelCase")]
pub fn list_managed_auth_accounts(
    state: State<'_, AppState>,
    provider: Option<ManagedAuthProvider>,
) -> Result<Vec<ManagedAuthAccount>, String> {
    AuthService::list_accounts(&state.inner().db_state(), provider).map_err(String::from)
}

#[tauri::command(rename_all = "camelCase")]
pub fn import_managed_auth_account(
    state: State<'_, AppState>,
    input: ManagedAuthAccountInput,
) -> Result<ManagedAuthAccount, String> {
    AuthService::import_account(&state.inner().db_state(), input).map_err(String::from)
}

#[tauri::command(rename_all = "camelCase")]
pub fn set_default_managed_auth_account(
    state: State<'_, AppState>,
    provider: ManagedAuthProvider,
    account_id: String,
) -> Result<bool, String> {
    AuthService::set_default(&state.inner().db_state(), provider, &account_id)
        .map(|_| true)
        .map_err(String::from)
}

#[tauri::command(rename_all = "camelCase")]
pub fn delete_managed_auth_account(
    state: State<'_, AppState>,
    provider: ManagedAuthProvider,
    account_id: String,
) -> Result<bool, String> {
    AuthService::delete_account(&state.inner().db_state(), provider, &account_id)
        .map_err(String::from)
}

#[tauri::command(rename_all = "camelCase")]
pub fn logout_managed_auth_account(
    state: State<'_, AppState>,
    provider: ManagedAuthProvider,
    account_id: String,
) -> Result<bool, String> {
    AuthService::logout_account(&state.inner().db_state(), provider, &account_id)
        .map_err(String::from)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn start_managed_auth_device_login(
    state: State<'_, AppState>,
    request: ManagedAuthDeviceStart,
) -> Result<ManagedAuthDeviceSession, String> {
    AuthService::start_device_login(&state.inner().db_state(), request)
        .await
        .map_err(String::from)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn poll_managed_auth_device_login(
    state: State<'_, AppState>,
    request: ManagedAuthDevicePoll,
) -> Result<ManagedAuthDevicePollResult, String> {
    AuthService::poll_device_login(&state.inner().db_state(), request)
        .await
        .map_err(String::from)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn query_managed_auth_usage(
    state: State<'_, AppState>,
    provider: ManagedAuthProvider,
    account_id: Option<String>,
) -> Result<ManagedAuthUsage, String> {
    AuthService::query_usage(&state.inner().db_state(), provider, account_id.as_deref())
        .await
        .map_err(String::from)
}
