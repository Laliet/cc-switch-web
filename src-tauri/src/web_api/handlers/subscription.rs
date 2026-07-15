#![cfg(feature = "web-server")]

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;

use crate::services::{SubscriptionProvider, SubscriptionQuota, SubscriptionService};
use crate::store::AppState;

use super::{ApiError, ApiResult};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuery {
    provider: String,
    account_id: Option<String>,
    force: Option<bool>,
}

pub async fn query_quota(
    State(state): State<Arc<AppState>>,
    Query(query): Query<QuotaQuery>,
) -> ApiResult<SubscriptionQuota> {
    let provider = SubscriptionProvider::parse(&query.provider).map_err(ApiError::from)?;
    Ok(Json(
        SubscriptionService::query(
            &state,
            provider,
            query.account_id.as_deref(),
            query.force.unwrap_or(false),
        )
        .await
        .map_err(ApiError::from)?,
    ))
}
