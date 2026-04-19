use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

pub async fn get_metrics(
    State(state): State<AppState>,
    axum::Json(_payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let vm_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vms")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

    let vm_running: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM vm_observed_state WHERE runtime_status = 'Running'",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let vm_stopped: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM vm_observed_state WHERE runtime_status = 'Stopped'",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let vm_error: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM vm_observed_state WHERE health_status = 'error'",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let node_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM nodes")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

    let node_online: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM node_observed_state WHERE health_status = 'healthy'",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    Ok(Json(json!({
        "vms": {
            "total": vm_total,
            "running": vm_running,
            "stopped": vm_stopped,
            "error": vm_error
        },
        "nodes": {
            "total": node_total,
            "online": node_online
        },
        "top_consumers": []
    })))
}
