use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

pub async fn mutate_volume(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let volume_id = payload
        .get("volume_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing volume_id".into()))?
        .to_string();

    let action = payload
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing action".into()))?
        .to_string();

    let force = payload.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
    let resize_bytes = payload.get("resize_bytes").and_then(|v| v.as_u64());

    let response = state
        .mutations
        .mutate_volume(volume_id, action, force, resize_bytes, claims.username)
        .await?;

    Ok(Json(json!({
        "accepted": response.accepted,
        "task_id": response.task_id,
        "volume_id": response.volume_id,
        "summary": response.summary,
    })))
}
