use std::collections::HashMap;
use tauri::State;

use crate::app_config::AppType;
use crate::error::AppError;
use crate::provider::{ClaudeDesktopMode, Provider, ProviderMeta, UniversalProvider};
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

#[tauri::command]
pub fn get_claude_desktop_default_routes(
) -> Vec<crate::claude_desktop_config::ClaudeDesktopDefaultRoute> {
    crate::claude_desktop_config::default_proxy_routes()
}

#[tauri::command]
pub async fn get_claude_desktop_status(
    state: State<'_, AppState>,
) -> Result<crate::claude_desktop_config::ClaudeDesktopStatus, String> {
    let state_arc = state.inner().db_state();
    let proxy = crate::proxy::status_for_state(&state_arc).await;
    crate::claude_desktop_config::get_status(&state.inner().db, proxy.running)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_claude_desktop_providers_from_claude(
    state: State<'_, AppState>,
) -> Result<usize, String> {
    import_claude_desktop_providers_from_claude_internal(&state).map_err(|e| e.to_string())
}

pub fn import_claude_desktop_providers_from_claude_internal(
    state: &AppState,
) -> Result<usize, AppError> {
    let mut imported = 0usize;
    state.update_config(|config| {
        let claude_providers = config
            .get_manager(&AppType::Claude)
            .map(|manager| manager.providers.clone())
            .unwrap_or_default();
        let desktop_manager = config
            .get_manager_mut(&AppType::ClaudeDesktop)
            .ok_or_else(|| {
                AppError::localized(
                    "provider.app.not_found",
                    "应用配置不存在: claude-desktop",
                    "App configuration not found: claude-desktop",
                )
            })?;

        ensure_claude_desktop_official_provider(desktop_manager);
        for provider in claude_providers.values() {
            if desktop_manager.providers.contains_key(&provider.id) {
                continue;
            }

            let mut desktop_provider = provider.clone();
            let meta = desktop_provider
                .meta
                .get_or_insert_with(ProviderMeta::default);
            if crate::claude_desktop_config::is_compatible_direct_provider(provider)
                && claude_provider_models_are_claude_safe(provider)
            {
                meta.claude_desktop_mode = Some(ClaudeDesktopMode::Direct);
            } else if let Some(routes) =
                crate::claude_desktop_config::suggested_routes_from_claude_provider(provider)
            {
                meta.claude_desktop_mode = Some(ClaudeDesktopMode::Proxy);
                meta.claude_desktop_model_routes = routes;
            } else {
                continue;
            }

            desktop_manager
                .providers
                .insert(desktop_provider.id.clone(), desktop_provider);
            imported += 1;
        }

        if desktop_manager.current.is_empty() {
            desktop_manager.current =
                crate::database::CLAUDE_DESKTOP_OFFICIAL_PROVIDER_ID.to_string();
        }
        Ok(())
    })?;
    Ok(imported)
}

fn ensure_claude_desktop_official_provider(manager: &mut crate::provider::ProviderManager) {
    let official_id = crate::database::CLAUDE_DESKTOP_OFFICIAL_PROVIDER_ID;
    manager
        .providers
        .entry(official_id.to_string())
        .or_insert_with(|| {
            let mut provider = Provider::with_id(
                official_id.to_string(),
                "Claude Desktop Official".to_string(),
                serde_json::json!({"env": {}}),
                Some("https://claude.ai/download".to_string()),
            );
            provider.category = Some("official".to_string());
            provider
        });
}

fn claude_provider_models_are_claude_safe(provider: &Provider) -> bool {
    let Some(env) = provider
        .settings_config
        .get("env")
        .and_then(|value| value.as_object())
    else {
        return true;
    };

    [
        "ANTHROPIC_MODEL",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        "ANTHROPIC_DEFAULT_SONNET_MODEL",
        "ANTHROPIC_DEFAULT_OPUS_MODEL",
    ]
    .into_iter()
    .filter_map(|key| env.get(key).and_then(|value| value.as_str()))
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .all(crate::claude_desktop_config::is_claude_safe_model_id)
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
