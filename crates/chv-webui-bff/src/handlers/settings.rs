use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

pub async fn get_settings(
    crate::auth::BearerToken(_claims): crate::auth::BearerToken,
    State(_state): State<AppState>,
    _payload: axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    Ok(Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "build": option_env!("BUILD_SHA").unwrap_or("dev"),
        "environment": option_env!("CHV_ENV").unwrap_or("development"),
        "api_endpoint": "/api/v1",
        "session_ttl_hours": 24,
    })))
}
