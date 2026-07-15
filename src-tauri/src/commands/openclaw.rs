use crate::openclaw_config::{
    self, OpenClawDefaultModel, OpenClawHealthWarning, OpenClawLiveProviderSummary,
    OpenClawLiveStatus, OpenClawWriteOutcome,
};

#[tauri::command]
pub async fn get_openclaw_status() -> Result<OpenClawLiveStatus, String> {
    openclaw_config::get_live_status().map_err(|error| error.to_string())
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
pub async fn get_openclaw_default_model() -> Result<Option<OpenClawDefaultModel>, String> {
    openclaw_config::get_default_model().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_openclaw_default_model(
    model: OpenClawDefaultModel,
) -> Result<OpenClawWriteOutcome, String> {
    openclaw_config::set_default_model(&model).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn clear_openclaw_default_model() -> Result<OpenClawWriteOutcome, String> {
    openclaw_config::clear_default_model().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn scan_openclaw_config_health() -> Result<Vec<OpenClawHealthWarning>, String> {
    openclaw_config::scan_openclaw_config_health().map_err(|error| error.to_string())
}
