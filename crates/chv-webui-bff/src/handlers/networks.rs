use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

pub async fn list_networks(
    State(state): State<AppState>,
    _payload: axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let rows = sqlx::query_as::<_, NetworkRow>(
        r#"
        SELECT
            n.network_id,
            n.display_name AS name,
            COALESCE(nos.exposure_status, 'private') AS exposure,
            COALESCE(nos.health_status, 'unknown') AS health,
            0::int AS attached_vms,
            COALESCE(last_task.operation_type, '') AS last_task,
            COALESCE(alert_counts.alerts, 0)::int AS alerts
        FROM networks n
        LEFT JOIN network_observed_state nos ON n.network_id = nos.network_id
        LEFT JOIN LATERAL (
            SELECT operation_type
            FROM operations
            WHERE resource_kind = 'network' AND resource_id = n.network_id
            ORDER BY requested_at DESC
            LIMIT 1
        ) last_task ON true
        LEFT JOIN (
            SELECT resource_id, COUNT(*) AS alerts
            FROM alerts
            WHERE status != 'resolved' AND resource_kind = 'network'
            GROUP BY resource_id
        ) alert_counts ON n.network_id = alert_counts.resource_id
        ORDER BY n.network_id
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to list networks: {}", e)))?;

    let items: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "network_id": r.network_id,
                "name": r.name,
                "scope": "fleet",
                "health": r.health,
                "attached_vms": r.attached_vms,
                "exposure": r.exposure,
                "policy": "default",
                "last_task": r.last_task,
                "alerts": r.alerts,
            })
        })
        .collect();

    Ok(Json(json!({
        "items": items,
        "page": {
            "page": 1,
            "page_size": 50,
            "total_items": items.len() as u64,
        },
        "filters": null,
    })))
}

pub async fn get_network(
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let network_id = payload
        .get("network_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing network_id".into()))?;

    let row = sqlx::query_as::<_, NetworkDetailRow>(
        r#"
        SELECT
            n.network_id,
            n.display_name AS name,
            COALESCE(nos.exposure_status, 'private') AS exposure,
            COALESCE(nos.health_status, 'unknown') AS health,
            COALESCE(last_task.operation_type, '') AS last_task,
            COALESCE(alert_counts.alerts, 0)::int AS alerts,
            n.created_at::text AS created_at
        FROM networks n
        LEFT JOIN network_observed_state nos ON n.network_id = nos.network_id
        LEFT JOIN LATERAL (
            SELECT operation_type
            FROM operations
            WHERE resource_kind = 'network' AND resource_id = n.network_id
            ORDER BY requested_at DESC
            LIMIT 1
        ) last_task ON true
        LEFT JOIN (
            SELECT resource_id, COUNT(*) AS alerts
            FROM alerts
            WHERE status != 'resolved' AND resource_kind = 'network'
            GROUP BY resource_id
        ) alert_counts ON n.network_id = alert_counts.resource_id
        WHERE n.network_id = $1
        "#,
    )
    .bind(network_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to get network: {}", e)))?;

    match row {
        Some(r) => Ok(Json(json!({
            "network_id": r.network_id,
            "name": r.name,
            "scope": "fleet",
            "health": r.health,
            "exposure": r.exposure,
            "policy": "default",
            "cidr": "10.0.0.0/24",
            "gateway": "10.0.0.1",
            "attached_vms": [],
            "created_at": r.created_at.unwrap_or_default(),
            "last_task": r.last_task,
            "alerts": r.alerts,
        }))),
        None => Err(BffError::NotFound(format!("network {} not found", network_id))),
    }
}

#[derive(sqlx::FromRow)]
struct NetworkRow {
    network_id: String,
    name: String,
    exposure: String,
    health: String,
    attached_vms: i32,
    last_task: String,
    alerts: i32,
}

#[derive(sqlx::FromRow)]
struct NetworkDetailRow {
    network_id: String,
    name: String,
    exposure: String,
    health: String,
    last_task: String,
    alerts: i32,
    created_at: Option<String>,
}
