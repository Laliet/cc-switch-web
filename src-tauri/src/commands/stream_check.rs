use crate::{
    app_config::AppType,
    services::stream_check::{StreamCheckConfig, StreamCheckResult, StreamCheckService},
    store::AppState,
};
use std::str::FromStr;
use tauri::State;

#[tauri::command(rename_all = "camelCase")]
pub async fn stream_check_provider(
    state: State<'_, AppState>,
    app_type: String,
    provider_id: String,
) -> Result<StreamCheckResult, String> {
    let app_type = AppType::from_str(&app_type).map_err(|e| e.to_string())?;
    let providers = crate::services::ProviderService::list(state.inner(), app_type.clone())
        .map_err(|e| e.to_string())?;
    let provider = providers
        .get(&provider_id)
        .ok_or_else(|| format!("Provider {provider_id} not found"))?;
    let config = StreamCheckService::get_config(state.inner()).map_err(|e| e.to_string())?;
    Ok(StreamCheckService::check_with_retry(&app_type, provider, &config).await)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn stream_check_all_providers(
    state: State<'_, AppState>,
    app_type: String,
    proxy_targets_only: Option<bool>,
) -> Result<Vec<(String, StreamCheckResult)>, String> {
    let app_type = AppType::from_str(&app_type).map_err(|e| e.to_string())?;
    let providers = crate::services::ProviderService::list(state.inner(), app_type.clone())
        .map_err(|e| e.to_string())?;
    let config = StreamCheckService::get_config(state.inner()).map_err(|e| e.to_string())?;
    let current = if proxy_targets_only.unwrap_or(false) {
        Some(
            crate::services::ProviderService::current(state.inner(), app_type.clone())
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };

    let mut results = Vec::new();
    for (id, provider) in providers {
        if current.as_ref().is_some_and(|current_id| current_id != &id) {
            continue;
        }
        let result = StreamCheckService::check_with_retry(&app_type, &provider, &config).await;
        results.push((id, result));
    }
    Ok(results)
}

#[tauri::command]
pub fn get_stream_check_config(state: State<'_, AppState>) -> Result<StreamCheckConfig, String> {
    StreamCheckService::get_config(state.inner()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_stream_check_config(
    state: State<'_, AppState>,
    config: StreamCheckConfig,
) -> Result<(), String> {
    StreamCheckService::save_config(state.inner(), &config).map_err(|e| e.to_string())
}
