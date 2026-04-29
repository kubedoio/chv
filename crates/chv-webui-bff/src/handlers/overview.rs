use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

pub async fn get_overview(
    crate::auth::BearerToken(_claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    _payload: axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let cache_key = "overview";
    if let Some(cached) = state.cache.get(cache_key).await {
        return Ok(Json(serde_json::from_str(&cached).map_err(|e| BffError::Internal(e.to_string()))?));
    }

    let nodes_total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM nodes")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

    let nodes_degraded = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM node_observed_state WHERE health_status IN ('degraded', 'warning', 'critical')"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let maintenance_nodes = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM node_desired_state WHERE desired_state = 'Maintenance'",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let vms_total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM vms")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

    let vms_running = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM vm_observed_state WHERE runtime_status = 'Running'",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let active_tasks = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM operations WHERE status IN ('Pending', 'Accepted', 'Running')",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let unresolved_alerts =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM alerts WHERE status != 'resolved'")
            .fetch_one(&state.pool)
            .await
            .unwrap_or(0);

    let networks_total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM networks")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

    let networks_healthy = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM network_observed_state WHERE health_status = 'healthy'",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let alerts_rows = sqlx::query_as::<_, AlertRow>(
        r#"
        SELECT
            message AS summary,
            'Cluster' AS scope,
            severity
        FROM alerts
        WHERE status != 'resolved'
        ORDER BY opened_at DESC
        LIMIT 5
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let alerts: Vec<Value> = alerts_rows
        .into_iter()
        .map(|r| {
            json!({
                "summary": r.summary,
                "scope": r.scope,
                "severity": r.severity.to_lowercase(),
            })
        })
        .collect();

    let tasks_rows = sqlx::query_as::<_, RecentTaskRow>(
        r#"
        SELECT
            operation_id AS task_id,
            status,
            operation_type AS summary,
            resource_kind,
            resource_id,
            operation_type AS operation,
            CAST(strftime('%s', requested_at) AS INTEGER) * 1000 AS started_unix_ms
        FROM operations
        ORDER BY requested_at DESC
        LIMIT 5
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let recent_tasks: Vec<Value> = tasks_rows
        .into_iter()
        .map(|r| {
            json!({
                "task_id": r.task_id,
                "status": r.status,
                "summary": r.summary,
                "resource_kind": r.resource_kind,
                "resource_id": r.resource_id.unwrap_or_default(),
                "operation": r.operation,
                "started_unix_ms": r.started_unix_ms.unwrap_or(0),
            })
        })
        .collect();

    let response = Json(json!({
        "clusters_total": 0,
        "clusters_healthy": 0,
        "clusters_degraded": 0,
        "nodes_total": nodes_total,
        "nodes_degraded": nodes_degraded,
        "vms_running": vms_running,
        "vms_total": vms_total,
        "networks_total": networks_total,
        "networks_healthy": networks_healthy,
        "active_tasks": active_tasks,
        "unresolved_alerts": unresolved_alerts,
        "maintenance_nodes": maintenance_nodes,
        "capacity_hotspots": 0,
        "alerts": alerts,
        "recent_tasks": recent_tasks,
        "state": "ready",
    }));
    if let Ok(json) = serde_json::to_string(&response.0) {
        state.cache.set_with_ttl(cache_key, json, Some(std::time::Duration::from_secs(10))).await;
    }
    Ok(response)
}

#[derive(sqlx::FromRow)]
struct AlertRow {
    summary: String,
    scope: String,
    severity: String,
}

#[derive(sqlx::FromRow)]
struct RecentTaskRow {
    task_id: String,
    status: String,
    summary: String,
    resource_kind: String,
    resource_id: Option<String>,
    operation: String,
    started_unix_ms: Option<i64>,
}
