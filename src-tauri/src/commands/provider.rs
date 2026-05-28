use std::collections::HashMap;
use tauri::State;

use crate::app_config::AppType;
use crate::error::AppError;
use crate::provider::{Provider, UniversalProvider};
use crate::services::{EndpointLatency, ProviderService, ProviderSortUpdate, SpeedtestService};
use crate::store::AppState;
use std::str::FromStr;

fn parse_provider_app_type(app: &str) -> Result<AppType, String> {
    AppType::from_str(app).map_err(|e| e.to_string())
}

/// 获取所有供应商
#[tauri::command]
pub fn get_providers(
    state: State<'_, AppState>,
    app: String,
) -> Result<HashMap<String, Provider>, String> {
    let app_type = parse_provider_app_type(&app)?;
    ProviderService::list(state.inner(), app_type).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_universal_providers(
    state: State<'_, AppState>,
) -> Result<HashMap<String, UniversalProvider>, String> {
    ProviderService::list_universal(state.inner()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_universal_provider(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<UniversalProvider>, String> {
    ProviderService::get_universal(state.inner(), &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn upsert_universal_provider(
    state: State<'_, AppState>,
    provider: UniversalProvider,
) -> Result<bool, String> {
    ProviderService::upsert_universal(state.inner(), provider).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_universal_provider(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    ProviderService::delete_universal(state.inner(), &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn sync_universal_provider_to_apps(
    state: State<'_, AppState>,
    id: String,
) -> Result<bool, String> {
    ProviderService::sync_universal_to_apps(state.inner(), &id).map_err(|e| e.to_string())
}

/// 获取当前供应商ID
#[tauri::command]
pub fn get_current_provider(state: State<'_, AppState>, app: String) -> Result<String, String> {
    let app_type = parse_provider_app_type(&app)?;
    ProviderService::current(state.inner(), app_type).map_err(|e| e.to_string())
}

/// 获取备用供应商 ID
#[tauri::command]
pub fn get_backup_provider(
    state: State<'_, AppState>,
    app: String,
) -> Result<Option<String>, String> {
    let app_type = parse_provider_app_type(&app)?;
    ProviderService::backup(state.inner(), app_type).map_err(|e| e.to_string())
}

/// 设置备用供应商 ID
#[tauri::command]
pub fn set_backup_provider(
    state: State<'_, AppState>,
    app: String,
    id: Option<String>,
) -> Result<bool, String> {
    let app_type = parse_provider_app_type(&app)?;
    ProviderService::set_backup(state.inner(), app_type, id).map_err(|e| e.to_string())?;
    Ok(true)
}

/// 添加供应商
#[tauri::command]
pub fn add_provider(
    state: State<'_, AppState>,
    app: String,
    provider: Provider,
) -> Result<bool, String> {
    let app_type = parse_provider_app_type(&app)?;
    ProviderService::add(state.inner(), app_type, provider).map_err(|e| e.to_string())
}

/// 更新供应商
#[tauri::command]
pub fn update_provider(
    state: State<'_, AppState>,
    app: String,
    provider: Provider,
) -> Result<bool, String> {
    let app_type = parse_provider_app_type(&app)?;
    ProviderService::update(state.inner(), app_type, provider).map_err(|e| e.to_string())
}

/// 删除供应商
#[tauri::command]
pub fn delete_provider(
    state: State<'_, AppState>,
    app: String,
    id: String,
) -> Result<bool, String> {
    let app_type = parse_provider_app_type(&app)?;
    ProviderService::delete(state.inner(), app_type, &id)
        .map(|_| true)
        .map_err(|e| e.to_string())
}

/// 切换供应商
fn switch_provider_internal(state: &AppState, app_type: AppType, id: &str) -> Result<(), AppError> {
    ProviderService::switch(state, app_type, id)
}

#[cfg_attr(not(feature = "test-hooks"), doc(hidden))]
pub fn switch_provider_test_hook(
    state: &AppState,
    app_type: AppType,
    id: &str,
) -> Result<(), AppError> {
    switch_provider_internal(state, app_type, id)
}

#[tauri::command]
pub fn switch_provider(
    state: State<'_, AppState>,
    app: String,
    id: String,
) -> Result<bool, String> {
    let app_type = parse_provider_app_type(&app)?;
    switch_provider_internal(&state, app_type, &id)
        .map(|_| true)
        .map_err(|e| e.to_string())
}

fn import_default_config_internal(state: &AppState, app_type: AppType) -> Result<(), AppError> {
    ProviderService::import_default_config(state, app_type)
}

#[cfg_attr(not(feature = "test-hooks"), doc(hidden))]
pub fn import_default_config_test_hook(
    state: &AppState,
    app_type: AppType,
) -> Result<(), AppError> {
    import_default_config_internal(state, app_type)
}

/// 导入当前配置为默认供应商
#[tauri::command]
pub fn import_default_config(state: State<'_, AppState>, app: String) -> Result<bool, String> {
    let app_type = parse_provider_app_type(&app)?;
    import_default_config_internal(&state, app_type)
        .map(|_| true)
        .map_err(Into::into)
}

/// 查询供应商用量
#[allow(non_snake_case)]
#[tauri::command]
pub async fn queryProviderUsage(
    state: State<'_, AppState>,
    #[allow(non_snake_case)] providerId: String, // 使用 camelCase 匹配前端
    app: String,
) -> Result<crate::provider::UsageResult, String> {
    let app_type = parse_provider_app_type(&app)?;
    ProviderService::query_usage(state.inner(), app_type, &providerId)
        .await
        .map_err(|e| e.to_string())
}

/// 测试用量脚本（使用当前编辑器中的脚本，不保存）
#[allow(non_snake_case)]
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn testUsageScript(
    state: State<'_, AppState>,
    #[allow(non_snake_case)] providerId: String,
    app: String,
    #[allow(non_snake_case)] scriptCode: String,
    timeout: Option<u64>,
    #[allow(non_snake_case)] apiKey: Option<String>,
    #[allow(non_snake_case)] baseUrl: Option<String>,
    #[allow(non_snake_case)] accessToken: Option<String>,
    #[allow(non_snake_case)] userId: Option<String>,
) -> Result<crate::provider::UsageResult, String> {
    let app_type = parse_provider_app_type(&app)?;
    ProviderService::test_usage_script(
        state.inner(),
        app_type,
        &providerId,
        &scriptCode,
        timeout.unwrap_or(10),
        apiKey.as_deref(),
        baseUrl.as_deref(),
        accessToken.as_deref(),
        userId.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())
}

/// 读取当前生效的配置内容
#[tauri::command]
pub fn read_live_provider_settings(
    _state: State<'_, AppState>,
    app: String,
) -> Result<serde_json::Value, String> {
    let app_type = parse_provider_app_type(&app)?;
    ProviderService::read_live_settings(app_type).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_opencode_live_provider_ids() -> Result<Vec<String>, String> {
    crate::opencode_config::get_providers()
        .map(|providers| providers.keys().cloned().collect())
        .map_err(|e| e.to_string())
}

/// 测试第三方/自定义供应商端点的网络延迟
#[tauri::command]
pub async fn test_api_endpoints(
    urls: Vec<String>,
    #[allow(non_snake_case)] timeoutSecs: Option<u64>,
) -> Result<Vec<EndpointLatency>, String> {
    SpeedtestService::test_endpoints(urls, timeoutSecs)
        .await
        .map_err(|e| e.to_string())
}

/// 获取自定义端点列表
#[tauri::command]
pub fn get_custom_endpoints(
    state: State<'_, AppState>,
    app: String,
    #[allow(non_snake_case)] providerId: String,
) -> Result<Vec<crate::settings::CustomEndpoint>, String> {
    let app_type = parse_provider_app_type(&app)?;
    ProviderService::get_custom_endpoints(state.inner(), app_type, &providerId)
        .map_err(|e| e.to_string())
}

/// 添加自定义端点
#[tauri::command]
pub fn add_custom_endpoint(
    state: State<'_, AppState>,
    app: String,
    #[allow(non_snake_case)] providerId: String,
    url: String,
) -> Result<(), String> {
    let app_type = parse_provider_app_type(&app)?;
    ProviderService::add_custom_endpoint(state.inner(), app_type, &providerId, url)
        .map_err(|e| e.to_string())
}

/// 删除自定义端点
#[tauri::command]
pub fn remove_custom_endpoint(
    state: State<'_, AppState>,
    app: String,
    #[allow(non_snake_case)] providerId: String,
    url: String,
) -> Result<(), String> {
    let app_type = parse_provider_app_type(&app)?;
    ProviderService::remove_custom_endpoint(state.inner(), app_type, &providerId, url)
        .map_err(|e| e.to_string())
}

/// 更新端点最后使用时间
#[tauri::command]
pub fn update_endpoint_last_used(
    state: State<'_, AppState>,
    app: String,
    #[allow(non_snake_case)] providerId: String,
    url: String,
) -> Result<(), String> {
    let app_type = parse_provider_app_type(&app)?;
    ProviderService::update_endpoint_last_used(state.inner(), app_type, &providerId, url)
        .map_err(|e| e.to_string())
}

/// 更新多个供应商的排序
#[tauri::command]
pub fn update_providers_sort_order(
    state: State<'_, AppState>,
    app: String,
    updates: Vec<ProviderSortUpdate>,
) -> Result<bool, String> {
    let app_type = parse_provider_app_type(&app)?;
    ProviderService::update_sort_order(state.inner(), app_type, updates).map_err(|e| e.to_string())
}

/// 查询 opencode.json 是否已启用标准 OMO 插件。
#[tauri::command]
pub fn get_omo_plugin_status() -> Result<bool, String> {
    crate::opencode_config::has_standard_omo_plugin().map_err(|e| e.to_string())
}

/// 查询 opencode.json 是否已启用 OMO Slim 插件。
#[tauri::command]
pub fn get_omo_slim_plugin_status() -> Result<bool, String> {
    crate::opencode_config::has_slim_omo_plugin().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn disable_current_omo(state: State<'_, AppState>) -> Result<bool, String> {
    ProviderService::disable_current_omo(state.inner()).map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn disable_current_omo_slim(state: State<'_, AppState>) -> Result<bool, String> {
    ProviderService::disable_current_omo_slim(state.inner()).map_err(|e| e.to_string())?;
    Ok(true)
}
