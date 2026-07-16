#![allow(non_snake_case)]

use std::collections::HashMap;

use tauri::State;

use crate::openclaw_config::{
    self, OpenClawAgentsDefaults, OpenClawDefaultModel, OpenClawEnvConfig, OpenClawHealthWarning,
    OpenClawLiveProviderSummary, OpenClawLiveStatus, OpenClawModelCatalogEntry, OpenClawSection,
    OpenClawToolsConfig, OpenClawWriteOutcome,
};
use crate::services::provider::{
    OpenClawReconciliationOutcome, OpenClawReconciliationPreview, ProviderService,
};
use crate::store::AppState;

#[tauri::command]
pub async fn get_openclaw_status() -> Result<OpenClawLiveStatus, String> {
    openclaw_config::get_live_status().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_openclaw_raw_config() -> Result<OpenClawSection<String>, String> {
    openclaw_config::get_raw_config().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_openclaw_raw_config(
    source: String,
    expectedEtag: Option<String>,
) -> Result<OpenClawWriteOutcome, String> {
    openclaw_config::set_raw_config(&source, expectedEtag.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_openclaw_live_providers() -> Result<Vec<OpenClawLiveProviderSummary>, String> {
    openclaw_config::get_live_provider_summaries().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_openclaw_live_provider(
    provider_id: String,
) -> Result<Option<OpenClawLiveProviderSummary>, String> {
    openclaw_config::get_live_provider_summary(&provider_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn preview_openclaw_provider_reconciliation(
    state: State<'_, AppState>,
) -> Result<OpenClawReconciliationPreview, String> {
    ProviderService::preview_openclaw_provider_reconciliation(state.inner())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn apply_openclaw_provider_reconciliation(
    state: State<'_, AppState>,
    providerIds: Vec<String>,
    updateExisting: bool,
    expectedEtag: Option<String>,
) -> Result<OpenClawReconciliationOutcome, String> {
    ProviderService::apply_openclaw_provider_reconciliation(
        state.inner(),
        &providerIds,
        updateExisting,
        expectedEtag.as_deref(),
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn import_openclaw_providers_from_live(
    state: State<'_, AppState>,
) -> Result<usize, String> {
    ProviderService::import_openclaw_providers_from_live(state.inner())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_openclaw_default_model() -> Result<Option<OpenClawDefaultModel>, String> {
    openclaw_config::get_default_model().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_openclaw_default_model(
    model: OpenClawDefaultModel,
    expectedEtag: Option<String>,
) -> Result<OpenClawWriteOutcome, String> {
    openclaw_config::set_default_model_with_etag(&model, expectedEtag.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn clear_openclaw_default_model(
    expectedEtag: Option<String>,
) -> Result<OpenClawWriteOutcome, String> {
    openclaw_config::clear_default_model_with_etag(expectedEtag.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_openclaw_model_catalog(
) -> Result<OpenClawSection<Option<HashMap<String, OpenClawModelCatalogEntry>>>, String> {
    openclaw_config::get_model_catalog().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_openclaw_model_catalog(
    catalog: HashMap<String, OpenClawModelCatalogEntry>,
    expectedEtag: Option<String>,
) -> Result<OpenClawWriteOutcome, String> {
    openclaw_config::set_model_catalog(&catalog, expectedEtag.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_openclaw_agents_defaults(
) -> Result<OpenClawSection<Option<OpenClawAgentsDefaults>>, String> {
    openclaw_config::get_agents_defaults().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_openclaw_agents_defaults(
    defaults: OpenClawAgentsDefaults,
    expectedEtag: Option<String>,
) -> Result<OpenClawWriteOutcome, String> {
    openclaw_config::set_agents_defaults(&defaults, expectedEtag.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_openclaw_env() -> Result<OpenClawSection<OpenClawEnvConfig>, String> {
    openclaw_config::get_env_config().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_openclaw_env(
    env: OpenClawEnvConfig,
    expectedEtag: Option<String>,
) -> Result<OpenClawWriteOutcome, String> {
    openclaw_config::set_env_config(&env, expectedEtag.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_openclaw_tools() -> Result<OpenClawSection<OpenClawToolsConfig>, String> {
    openclaw_config::get_tools_config().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_openclaw_tools(
    tools: OpenClawToolsConfig,
    expectedEtag: Option<String>,
) -> Result<OpenClawWriteOutcome, String> {
    openclaw_config::set_tools_config(&tools, expectedEtag.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn scan_openclaw_config_health() -> Result<Vec<OpenClawHealthWarning>, String> {
    openclaw_config::scan_openclaw_config_health().map_err(|error| error.to_string())
}
