use crate::{
    app_config::AppType,
    database::StreamCheckLogFilters,
    services::stream_check::{
        StreamCheckConfig, StreamCheckLog, StreamCheckResult, StreamCheckService,
    },
    store::AppState,
};
use serde::Deserialize;
use std::str::FromStr;
use tauri::State;

#[tauri::command(rename_all = "camelCase")]
pub async fn stream_check_provider(
    state: State<'_, AppState>,
    app_type: String,
    provider_id: String,
) -> Result<StreamCheckResult, String> {
    let app_type = AppType::from_str(&app_type).map_err(|e| e.to_string())?;
    StreamCheckService::ensure_app_supported(&app_type).map_err(|e| e.to_string())?;
    let providers = crate::services::ProviderService::list(state.inner(), app_type.clone())
        .map_err(|e| e.to_string())?;
    let provider = providers
        .get(&provider_id)
        .ok_or_else(|| format!("Provider {provider_id} not found"))?;
    let config = StreamCheckService::get_config(state.inner()).map_err(|e| e.to_string())?;
    let result = StreamCheckService::check_with_retry(&app_type, provider, &config).await;
    StreamCheckService::record_result(state.inner(), &app_type, provider, &result)
        .map_err(|e| e.to_string())?;
    Ok(result)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn stream_check_all_providers(
    state: State<'_, AppState>,
    app_type: String,
    proxy_targets_only: Option<bool>,
) -> Result<Vec<(String, StreamCheckResult)>, String> {
    let app_type = AppType::from_str(&app_type).map_err(|e| e.to_string())?;
    StreamCheckService::ensure_app_supported(&app_type).map_err(|e| e.to_string())?;
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
        StreamCheckService::record_result(state.inner(), &app_type, &provider, &result)
            .map_err(|e| e.to_string())?;
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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamCheckLogQuery {
    pub app_type: Option<String>,
    pub provider_id: Option<String>,
    pub status: Option<String>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

fn into_filters(query: StreamCheckLogQuery) -> StreamCheckLogFilters {
    StreamCheckLogFilters {
        app_type: query.app_type.filter(|value| !value.trim().is_empty()),
        provider_id: query.provider_id.filter(|value| !value.trim().is_empty()),
        status: query.status.filter(|value| !value.trim().is_empty()),
        since: query.since,
        until: query.until,
        limit: query.limit.unwrap_or(100),
        offset: query.offset.unwrap_or(0),
    }
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_stream_check_logs(
    state: tauri::State<'_, AppState>,
    query: Option<StreamCheckLogQuery>,
) -> Result<Vec<StreamCheckLog>, String> {
    StreamCheckService::list_logs(state.inner(), into_filters(query.unwrap_or_default()))
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_latest_stream_check_logs(
    state: tauri::State<'_, AppState>,
    app_type: Option<String>,
) -> Result<Vec<StreamCheckLog>, String> {
    StreamCheckService::latest_logs(state.inner(), app_type.as_deref()).map_err(|e| e.to_string())
}
