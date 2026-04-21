use axum::{extract::State, response::Json};
use serde_json::{json, Value};
use sqlx::Row;

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

    // Top consumers: latest metric per running VM, ordered by memory used desc
    let top_consumers_rows = sqlx::query(
        r#"
        SELECT
            m.vm_id,
            v.display_name,
            m.cpu_percent,
            m.memory_bytes_used,
            m.memory_bytes_total,
            m.disk_bytes_read,
            m.disk_bytes_written,
            m.net_bytes_rx,
            m.net_bytes_tx
        FROM vm_metrics m
        JOIN (
            SELECT vm_id, MAX(collected_at) AS latest
            FROM vm_metrics
            GROUP BY vm_id
        ) latest ON m.vm_id = latest.vm_id AND m.collected_at = latest.latest
        JOIN vms v ON v.vm_id = m.vm_id
        JOIN vm_observed_state s ON s.vm_id = m.vm_id AND s.runtime_status = 'Running'
        ORDER BY m.memory_bytes_used DESC
        LIMIT 10
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let mut top_consumers = Vec::new();
    for row in top_consumers_rows {
        top_consumers.push(json!({
            "vm_id": row.try_get::<String, _>("vm_id").unwrap_or_default(),
            "display_name": row.try_get::<String, _>("display_name").unwrap_or_default(),
            "cpu_percent": row.try_get::<f64, _>("cpu_percent").unwrap_or(0.0),
            "memory_bytes_used": row.try_get::<i64, _>("memory_bytes_used").unwrap_or(0),
            "memory_bytes_total": row.try_get::<i64, _>("memory_bytes_total").unwrap_or(0),
            "disk_bytes_read": row.try_get::<i64, _>("disk_bytes_read").unwrap_or(0),
            "disk_bytes_written": row.try_get::<i64, _>("disk_bytes_written").unwrap_or(0),
            "net_bytes_rx": row.try_get::<i64, _>("net_bytes_rx").unwrap_or(0),
            "net_bytes_tx": row.try_get::<i64, _>("net_bytes_tx").unwrap_or(0),
        }));
    }

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
        "top_consumers": top_consumers
    })))
}
