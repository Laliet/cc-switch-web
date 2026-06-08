#![cfg(feature = "web-server")]

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::{
    database::ModelPricingRecord,
    services::usage_stats::{
        DailyStats, DataSourceSummary, LogFilters, ModelStats, PaginatedLogs, ProviderLimitStatus,
        ProviderStats, RequestLogDetail, SessionSyncResult, UsageDataExtent, UsageStatsFilters,
        UsageSummary, UsageSummaryByApp,
    },
    store::AppState,
};

use super::{ApiError, ApiResult};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageQuery {
    pub start_date: Option<i64>,
    pub end_date: Option<i64>,
    pub app_type: Option<String>,
    pub provider_id: Option<String>,
    pub model: Option<String>,
}

impl UsageQuery {
    fn filters(self) -> UsageStatsFilters {
        UsageStatsFilters {
            app_type: self.app_type,
            provider_id: self.provider_id,
            model: self.model,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestLogsPayload {
    pub filters: LogFilters,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PricingPayload {
    pub model_id: Option<String>,
    pub display_name: String,
    pub input_cost_per_million: String,
    pub output_cost_per_million: String,
    pub cache_read_cost_per_million: String,
    pub cache_creation_cost_per_million: String,
}

pub async fn summary(
    State(state): State<Arc<AppState>>,
    Query(query): Query<UsageQuery>,
) -> ApiResult<UsageSummary> {
    let start_date = query.start_date;
    let end_date = query.end_date;
    let filters = query.filters();
    Ok(Json(
        state
            .db
            .get_usage_summary_with_filters(start_date, end_date, &filters)
            .map_err(ApiError::from)?,
    ))
}

pub async fn summary_by_app(
    State(state): State<Arc<AppState>>,
    Query(query): Query<UsageQuery>,
) -> ApiResult<Vec<UsageSummaryByApp>> {
    Ok(Json(
        state
            .db
            .get_usage_summary_by_app(query.start_date, query.end_date)
            .map_err(ApiError::from)?,
    ))
}

pub async fn trends(
    State(state): State<Arc<AppState>>,
    Query(query): Query<UsageQuery>,
) -> ApiResult<Vec<DailyStats>> {
    let start_date = query.start_date;
    let end_date = query.end_date;
    let filters = query.filters();
    Ok(Json(
        state
            .db
            .get_daily_trends_with_filters(start_date, end_date, &filters)
            .map_err(ApiError::from)?,
    ))
}

pub async fn providers(
    State(state): State<Arc<AppState>>,
    Query(query): Query<UsageQuery>,
) -> ApiResult<Vec<ProviderStats>> {
    let start_date = query.start_date;
    let end_date = query.end_date;
    let filters = query.filters();
    Ok(Json(
        state
            .db
            .get_provider_stats_with_filters(start_date, end_date, &filters)
            .map_err(ApiError::from)?,
    ))
}

pub async fn models(
    State(state): State<Arc<AppState>>,
    Query(query): Query<UsageQuery>,
) -> ApiResult<Vec<ModelStats>> {
    let start_date = query.start_date;
    let end_date = query.end_date;
    let filters = query.filters();
    Ok(Json(
        state
            .db
            .get_model_stats_with_filters(start_date, end_date, &filters)
            .map_err(ApiError::from)?,
    ))
}

pub async fn logs(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RequestLogsPayload>,
) -> ApiResult<PaginatedLogs> {
    Ok(Json(
        state
            .db
            .get_request_logs(
                &payload.filters,
                payload.page.unwrap_or(0),
                payload.page_size.unwrap_or(20),
            )
            .map_err(ApiError::from)?,
    ))
}

pub async fn detail(
    State(state): State<Arc<AppState>>,
    Path(request_id): Path<String>,
) -> ApiResult<Option<RequestLogDetail>> {
    Ok(Json(
        state
            .db
            .get_request_detail(&request_id)
            .map_err(ApiError::from)?,
    ))
}

pub async fn pricing(State(state): State<Arc<AppState>>) -> ApiResult<Vec<ModelPricingRecord>> {
    Ok(Json(state.db.list_model_pricing().map_err(ApiError::from)?))
}

pub async fn upsert_pricing(
    State(state): State<Arc<AppState>>,
    Path(model_id): Path<String>,
    Json(payload): Json<PricingPayload>,
) -> ApiResult<u64> {
    let record = ModelPricingRecord {
        model_id: payload.model_id.unwrap_or(model_id),
        display_name: payload.display_name,
        input_cost_per_million: payload.input_cost_per_million,
        output_cost_per_million: payload.output_cost_per_million,
        cache_read_cost_per_million: payload.cache_read_cost_per_million,
        cache_creation_cost_per_million: payload.cache_creation_cost_per_million,
    };
    if record.model_id.trim().is_empty() {
        return Err(ApiError::bad_request("model id is required"));
    }
    let updated = state
        .db
        .update_model_pricing_and_backfill(&record)
        .map_err(ApiError::from)?;
    Ok(Json(updated))
}

pub async fn delete_pricing(
    State(state): State<Arc<AppState>>,
    Path(model_id): Path<String>,
) -> ApiResult<bool> {
    Ok(Json(
        state
            .db
            .delete_model_pricing(&model_id)
            .map_err(ApiError::from)?,
    ))
}

pub async fn limits(
    State(state): State<Arc<AppState>>,
    Path((app_type, provider_id)): Path<(String, String)>,
) -> ApiResult<ProviderLimitStatus> {
    Ok(Json(
        state
            .db
            .check_provider_limits(&provider_id, &app_type)
            .map_err(ApiError::from)?,
    ))
}

pub async fn sync_sessions(State(state): State<Arc<AppState>>) -> ApiResult<SessionSyncResult> {
    Ok(Json(state.db.sync_session_usage().map_err(ApiError::from)?))
}

pub async fn data_sources(State(state): State<Arc<AppState>>) -> ApiResult<Vec<DataSourceSummary>> {
    Ok(Json(
        state.db.get_usage_data_sources().map_err(ApiError::from)?,
    ))
}

pub async fn data_extent(
    State(state): State<Arc<AppState>>,
    Query(query): Query<UsageQuery>,
) -> ApiResult<UsageDataExtent> {
    Ok(Json(
        state
            .db
            .get_usage_data_extent(query.app_type.as_deref())
            .map_err(ApiError::from)?,
    ))
}
