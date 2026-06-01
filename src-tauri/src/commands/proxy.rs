use tauri::State;

use crate::{
    app_config::AppType,
    database::ModelPricingRecord,
    proxy::{
        self, ProxyRecentLog, ProxyService, ProxyStatus, ProxyTakeoverResult, ProxyTestResult,
    },
    settings::ProxySettings,
    store::AppState,
};

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailoverQueueResponseItem {
    pub provider_id: String,
    pub provider_name: String,
    pub position: i64,
}

fn parse_proxy_app_type(app: &str) -> Result<AppType, String> {
    AppType::parse_supported(app).map_err(|e| e.to_string())
}

fn validate_failover_provider_ids(
    state: &AppState,
    app_type: &str,
    provider_ids: &[String],
) -> Result<(), crate::AppError> {
    let config = state.load_config()?;
    let app = AppType::parse_supported(app_type)?;
    let Some(manager) = config.get_manager(&app) else {
        return Err(crate::AppError::InvalidInput(format!(
            "No provider manager found for app '{app_type}'"
        )));
    };
    for provider_id in provider_ids {
        if provider_id.trim().is_empty() {
            return Err(crate::AppError::InvalidInput(
                "Failover provider id cannot be empty".to_string(),
            ));
        }
        if !manager.providers.contains_key(provider_id) {
            return Err(crate::AppError::InvalidInput(format!(
                "Provider '{provider_id}' does not exist for app '{app_type}'"
            )));
        }
    }
    Ok(())
}

fn failover_queue_items(
    state: &AppState,
    app_type: &str,
) -> Result<Vec<FailoverQueueResponseItem>, crate::AppError> {
    let config = state.load_config()?;
    let app = AppType::parse_supported(app_type)?;
    let manager = config.get_manager(&app);
    state
        .db
        .list_failover_queue(app_type)?
        .into_iter()
        .map(|item| {
            let provider_name = manager
                .and_then(|manager| manager.providers.get(&item.provider_id))
                .map(|provider| provider.name.clone())
                .unwrap_or_else(|| item.provider_id.clone());
            Ok(FailoverQueueResponseItem {
                provider_id: item.provider_id,
                provider_name,
                position: item.position,
            })
        })
        .collect()
}

#[tauri::command]
pub async fn proxy_status(state: State<'_, AppState>) -> Result<ProxyStatus, String> {
    Ok(proxy::status_for_state(&state.inner().db_state()).await)
}

#[tauri::command]
pub fn proxy_config(state: State<'_, AppState>) -> Result<ProxySettings, String> {
    ProxyService::config(state.inner()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_proxy_config(
    state: State<'_, AppState>,
    settings: ProxySettings,
) -> Result<ProxySettings, String> {
    ProxyService::save_config_and_update_runtime(state.inner(), settings)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_proxy_settings(
    state: State<'_, AppState>,
    settings: ProxySettings,
) -> Result<bool, String> {
    ProxyService::save_config_and_update_runtime(state.inner(), settings)
        .await
        .map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub async fn start_proxy(
    state: State<'_, AppState>,
    settings: ProxySettings,
) -> Result<ProxyStatus, String> {
    ProxyService::start(state.inner().db_state(), settings)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_proxy(state: State<'_, AppState>) -> Result<ProxyStatus, String> {
    ProxyService::stop(state.inner().db_state())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn test_proxy(
    state: State<'_, AppState>,
    settings: ProxySettings,
) -> Result<ProxyTestResult, String> {
    proxy::test_settings(state.inner().db_state(), settings)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_proxy_takeover(
    state: State<'_, AppState>,
    app: String,
    enabled: bool,
) -> Result<ProxyTakeoverResult, String> {
    let app_type = parse_proxy_app_type(&app)?;
    ProxyService::set_takeover(state.inner().db_state(), app_type, enabled)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_proxy(state: State<'_, AppState>) -> Result<ProxyStatus, String> {
    ProxyService::restore(state.inner().db_state())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn recover_stale_proxy_takeover(
    state: State<'_, AppState>,
) -> Result<ProxyStatus, String> {
    ProxyService::recover_stale_takeover(state.inner().db_state())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn proxy_recent_logs(state: State<'_, AppState>) -> Result<Vec<ProxyRecentLog>, String> {
    Ok(proxy::recent_logs_for_state(state.inner()).await)
}

#[tauri::command]
pub fn list_model_pricing(state: State<'_, AppState>) -> Result<Vec<ModelPricingRecord>, String> {
    state.db.list_model_pricing().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn upsert_model_pricing(
    state: State<'_, AppState>,
    record: ModelPricingRecord,
) -> Result<bool, String> {
    state
        .db
        .upsert_model_pricing(&record)
        .map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn delete_model_pricing(state: State<'_, AppState>, model_id: String) -> Result<bool, String> {
    state
        .db
        .delete_model_pricing(&model_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_failover_queue(
    state: State<'_, AppState>,
    app: String,
) -> Result<Vec<FailoverQueueResponseItem>, String> {
    let app_type = parse_proxy_app_type(&app)?;
    failover_queue_items(state.inner(), app_type.as_str()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn replace_failover_queue(
    state: State<'_, AppState>,
    app: String,
    provider_ids: Vec<String>,
) -> Result<Vec<FailoverQueueResponseItem>, String> {
    let app_type = parse_proxy_app_type(&app)?;
    validate_failover_provider_ids(state.inner(), app_type.as_str(), &provider_ids)
        .map_err(|e| e.to_string())?;
    state
        .db
        .replace_failover_queue(app_type.as_str(), &provider_ids)
        .map_err(|e| e.to_string())?;
    failover_queue_items(state.inner(), app_type.as_str()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_failover_provider(
    state: State<'_, AppState>,
    app: String,
    provider_id: String,
) -> Result<Vec<FailoverQueueResponseItem>, String> {
    let app_type = parse_proxy_app_type(&app)?;
    validate_failover_provider_ids(
        state.inner(),
        app_type.as_str(),
        std::slice::from_ref(&provider_id),
    )
    .map_err(|e| e.to_string())?;
    state
        .db
        .add_failover_provider(app_type.as_str(), &provider_id)
        .map_err(|e| e.to_string())?;
    failover_queue_items(state.inner(), app_type.as_str()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_failover_provider(
    state: State<'_, AppState>,
    app: String,
    provider_id: String,
) -> Result<Vec<FailoverQueueResponseItem>, String> {
    let app_type = parse_proxy_app_type(&app)?;
    state
        .db
        .remove_failover_provider(app_type.as_str(), &provider_id)
        .map_err(|e| e.to_string())?;
    failover_queue_items(state.inner(), app_type.as_str()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_failover_queue(
    state: State<'_, AppState>,
    app: String,
) -> Result<Vec<FailoverQueueResponseItem>, String> {
    let app_type = parse_proxy_app_type(&app)?;
    state
        .db
        .clear_failover_queue(app_type.as_str())
        .map_err(|e| e.to_string())?;
    Ok(Vec::new())
}
