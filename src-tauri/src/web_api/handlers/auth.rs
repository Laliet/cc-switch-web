#![cfg(feature = "web-server")]

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::{
    auth::{
        ManagedAuthAccount, ManagedAuthAccountInput, ManagedAuthDevicePoll,
        ManagedAuthDevicePollResult, ManagedAuthDeviceSession, ManagedAuthDeviceStart,
        ManagedAuthProvider, ManagedAuthUsage,
    },
    services::AuthService,
    store::AppState,
};

use super::{ApiError, ApiResult};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAuthQuery {
    pub provider: Option<ManagedAuthProvider>,
}

#[derive(Debug, Deserialize)]
pub struct AuthAccountPath {
    pub provider: ManagedAuthProvider,
    pub account_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthAccountQuery {
    pub provider: ManagedAuthProvider,
    pub account_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageQuery {
    pub provider: ManagedAuthProvider,
    pub account_id: Option<String>,
}

pub async fn list_accounts(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListAuthQuery>,
) -> ApiResult<Vec<ManagedAuthAccount>> {
    Ok(Json(
        AuthService::list_accounts(&state, query.provider).map_err(ApiError::from)?,
    ))
}

pub async fn import_account(
    State(state): State<Arc<AppState>>,
    Json(input): Json<ManagedAuthAccountInput>,
) -> ApiResult<ManagedAuthAccount> {
    Ok(Json(
        AuthService::import_account(&state, input).map_err(ApiError::from)?,
    ))
}

pub async fn set_default(
    State(state): State<Arc<AppState>>,
    Path(path): Path<AuthAccountPath>,
) -> ApiResult<bool> {
    AuthService::set_default(&state, path.provider, &path.account_id).map_err(ApiError::from)?;
    Ok(Json(true))
}

pub async fn set_default_by_query(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuthAccountQuery>,
) -> ApiResult<bool> {
    AuthService::set_default(&state, query.provider, &query.account_id).map_err(ApiError::from)?;
    Ok(Json(true))
}

pub async fn delete_account(
    State(state): State<Arc<AppState>>,
    Path(path): Path<AuthAccountPath>,
) -> ApiResult<bool> {
    Ok(Json(
        AuthService::delete_account(&state, path.provider, &path.account_id)
            .map_err(ApiError::from)?,
    ))
}

pub async fn delete_account_by_query(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuthAccountQuery>,
) -> ApiResult<bool> {
    Ok(Json(
        AuthService::delete_account(&state, query.provider, &query.account_id)
            .map_err(ApiError::from)?,
    ))
}

pub async fn logout_account(
    State(state): State<Arc<AppState>>,
    Path(path): Path<AuthAccountPath>,
) -> ApiResult<bool> {
    Ok(Json(
        AuthService::logout_account(&state, path.provider, &path.account_id)
            .map_err(ApiError::from)?,
    ))
}

pub async fn logout_account_by_query(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuthAccountQuery>,
) -> ApiResult<bool> {
    Ok(Json(
        AuthService::logout_account(&state, query.provider, &query.account_id)
            .map_err(ApiError::from)?,
    ))
}

pub async fn start_device_login(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ManagedAuthDeviceStart>,
) -> ApiResult<ManagedAuthDeviceSession> {
    Ok(Json(
        AuthService::start_device_login(&state, request)
            .await
            .map_err(ApiError::from)?,
    ))
}

pub async fn poll_device_login(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ManagedAuthDevicePoll>,
) -> ApiResult<ManagedAuthDevicePollResult> {
    Ok(Json(
        AuthService::poll_device_login(&state, request)
            .await
            .map_err(ApiError::from)?,
    ))
}

pub async fn query_usage(
    State(state): State<Arc<AppState>>,
    Query(query): Query<UsageQuery>,
) -> ApiResult<ManagedAuthUsage> {
    Ok(Json(
        AuthService::query_usage(&state, query.provider, query.account_id.as_deref())
            .await
            .map_err(ApiError::from)?,
    ))
}
