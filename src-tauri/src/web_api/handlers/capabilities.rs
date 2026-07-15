#![cfg(feature = "web-server")]

use axum::Json;

use crate::capabilities::RuntimeCapabilities;

use super::ApiResult;

pub async fn get_capabilities() -> ApiResult<RuntimeCapabilities> {
    Ok(Json(RuntimeCapabilities::web()))
}
