use tauri::State;

use crate::{
    database::ModelPricingRecord,
    services::usage_stats::{
        DailyStats, DataSourceSummary, LogFilters, ModelStats, PaginatedLogs, ProviderLimitStatus,
        ProviderStats, RequestLogDetail, SessionSyncResult, UsageDataExtent, UsageStatsFilters,
        UsageSummary, UsageSummaryByApp,
    },
    store::AppState,
};

#[tauri::command]
pub fn get_usage_summary(
    state: State<'_, AppState>,
    start_date: Option<i64>,
    end_date: Option<i64>,
    app_type: Option<String>,
    provider_id: Option<String>,
    model: Option<String>,
) -> Result<UsageSummary, String> {
    let filters = UsageStatsFilters {
        app_type,
        provider_id,
        model,
    };
    state
        .db
        .get_usage_summary_with_filters(start_date, end_date, &filters)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_usage_summary_by_app(
    state: State<'_, AppState>,
    start_date: Option<i64>,
    end_date: Option<i64>,
) -> Result<Vec<UsageSummaryByApp>, String> {
    state
        .db
        .get_usage_summary_by_app(start_date, end_date)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_usage_trends(
    state: State<'_, AppState>,
    start_date: Option<i64>,
    end_date: Option<i64>,
    app_type: Option<String>,
    provider_id: Option<String>,
    model: Option<String>,
) -> Result<Vec<DailyStats>, String> {
    let filters = UsageStatsFilters {
        app_type,
        provider_id,
        model,
    };
    state
        .db
        .get_daily_trends_with_filters(start_date, end_date, &filters)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_provider_stats(
    state: State<'_, AppState>,
    start_date: Option<i64>,
    end_date: Option<i64>,
    app_type: Option<String>,
    provider_id: Option<String>,
    model: Option<String>,
) -> Result<Vec<ProviderStats>, String> {
    let filters = UsageStatsFilters {
        app_type,
        provider_id,
        model,
    };
    state
        .db
        .get_provider_stats_with_filters(start_date, end_date, &filters)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_model_stats(
    state: State<'_, AppState>,
    start_date: Option<i64>,
    end_date: Option<i64>,
    app_type: Option<String>,
    provider_id: Option<String>,
    model: Option<String>,
) -> Result<Vec<ModelStats>, String> {
    let filters = UsageStatsFilters {
        app_type,
        provider_id,
        model,
    };
    state
        .db
        .get_model_stats_with_filters(start_date, end_date, &filters)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_request_logs(
    state: State<'_, AppState>,
    filters: LogFilters,
    page: u32,
    page_size: u32,
) -> Result<PaginatedLogs, String> {
    state
        .db
        .get_request_logs(&filters, page, page_size)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_request_detail(
    state: State<'_, AppState>,
    request_id: String,
) -> Result<Option<RequestLogDetail>, String> {
    state
        .db
        .get_request_detail(&request_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_model_pricing(state: State<'_, AppState>) -> Result<Vec<ModelPricingRecord>, String> {
    state.db.list_model_pricing().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn update_model_pricing(
    state: State<'_, AppState>,
    model_id: String,
    display_name: String,
    input_cost: String,
    output_cost: String,
    cache_read_cost: String,
    cache_creation_cost: String,
) -> Result<u64, String> {
    let record = ModelPricingRecord {
        model_id,
        display_name,
        input_cost_per_million: input_cost,
        output_cost_per_million: output_cost,
        cache_read_cost_per_million: cache_read_cost,
        cache_creation_cost_per_million: cache_creation_cost,
    };
    state
        .db
        .update_model_pricing_and_backfill(&record)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn check_provider_limits(
    state: State<'_, AppState>,
    provider_id: String,
    app_type: String,
) -> Result<ProviderLimitStatus, String> {
    state
        .db
        .check_provider_limits(&provider_id, &app_type)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn sync_session_usage(state: State<'_, AppState>) -> Result<SessionSyncResult, String> {
    state.db.sync_session_usage().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_usage_data_sources(
    state: State<'_, AppState>,
) -> Result<Vec<DataSourceSummary>, String> {
    state
        .db
        .get_usage_data_sources()
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_usage_data_extent(
    state: State<'_, AppState>,
    app_type: Option<String>,
) -> Result<UsageDataExtent, String> {
    state
        .db
        .get_usage_data_extent(app_type.as_deref())
        .map_err(|err| err.to_string())
}
