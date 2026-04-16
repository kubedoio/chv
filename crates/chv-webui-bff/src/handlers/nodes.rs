use axum::{extract::State, response::Json};
use serde_json::json;

use crate::router::AppState;
use crate::BffError;

pub async fn list_nodes(State(state): State<AppState>) -> Result<Json<serde_json::Value>, BffError> {
    let rows = sqlx::query_as::<_, NodeRow>(
        r#"
        SELECT
            n.node_id,
            n.display_name AS name,
            COALESCE(nos.observed_state::text, 'Unknown') AS state,
            COALESCE(nos.health_status, 'unknown') AS health,
            COALESCE(ni.cpu_count::text, '') AS cpu,
            COALESCE(pg_size_pretty(ni.memory_bytes), '') AS memory,
            COALESCE(pg_size_pretty(ni.disk_bytes), '') AS storage,
            '' AS network,
            COALESCE(n.agent_version, '') AS version,
            COALESCE(nds.desired_state::text = 'Maintenance', false) AS maintenance,
            COALESCE(nds.scheduling_paused, false) AS scheduling_paused,
            COALESCE(task_counts.active_tasks, 0)::int AS active_tasks,
            COALESCE(alert_counts.alerts, 0)::int AS alerts
        FROM nodes n
        LEFT JOIN node_observed_state nos ON n.node_id = nos.node_id
        LEFT JOIN node_desired_state nds ON n.node_id = nds.node_id
        LEFT JOIN node_inventory ni ON n.node_id = ni.node_id
        LEFT JOIN (
            SELECT resource_id, COUNT(*) AS active_tasks
            FROM operations
            WHERE resource_kind = 'node'
              AND status IN ('Pending', 'Accepted', 'Running')
            GROUP BY resource_id
        ) task_counts ON n.node_id = task_counts.resource_id
        LEFT JOIN (
            SELECT node_id, COUNT(*) AS alerts
            FROM alerts
            WHERE status != 'resolved'
            GROUP BY node_id
        ) alert_counts ON n.node_id = alert_counts.node_id
        ORDER BY n.node_id
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to list nodes: {}", e)))?;

    let items: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "node_id": r.node_id,
                "name": r.name,
                "cluster": "",
                "state": r.state,
                "health": r.health,
                "cpu": r.cpu,
                "memory": r.memory,
                "storage": r.storage,
                "network": r.network,
                "version": r.version,
                "maintenance": r.maintenance,
                "active_tasks": r.active_tasks,
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

pub async fn get_node(
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, BffError> {
    let node_id = payload
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing node_id".into()))?;

    let row = sqlx::query_as::<_, NodeRow>(
        r#"
        SELECT
            n.node_id,
            n.display_name AS name,
            COALESCE(nos.observed_state::text, 'Unknown') AS state,
            COALESCE(nos.health_status, 'unknown') AS health,
            COALESCE(ni.cpu_count::text, '') AS cpu,
            COALESCE(pg_size_pretty(ni.memory_bytes), '') AS memory,
            COALESCE(pg_size_pretty(ni.disk_bytes), '') AS storage,
            '' AS network,
            COALESCE(n.agent_version, '') AS version,
            COALESCE(nds.desired_state::text = 'Maintenance', false) AS maintenance,
            COALESCE(nds.scheduling_paused, false) AS scheduling_paused,
            COALESCE(task_counts.active_tasks, 0)::int AS active_tasks,
            COALESCE(alert_counts.alerts, 0)::int AS alerts
        FROM nodes n
        LEFT JOIN node_observed_state nos ON n.node_id = nos.node_id
        LEFT JOIN node_desired_state nds ON n.node_id = nds.node_id
        LEFT JOIN node_inventory ni ON n.node_id = ni.node_id
        LEFT JOIN (
            SELECT resource_id, COUNT(*) AS active_tasks
            FROM operations
            WHERE resource_kind = 'node'
              AND status IN ('Pending', 'Accepted', 'Running')
            GROUP BY resource_id
        ) task_counts ON n.node_id = task_counts.resource_id
        LEFT JOIN (
            SELECT node_id, COUNT(*) AS alerts
            FROM alerts
            WHERE status != 'resolved'
            GROUP BY node_id
        ) alert_counts ON n.node_id = alert_counts.node_id
        WHERE n.node_id = $1
        "#,
    )
    .bind(node_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to get node: {}", e)))?;

    match row {
        Some(r) => Ok(Json(json!({
            "summary": {
                "node_id": r.node_id,
                "name": r.name,
                "cluster": "",
                "state": r.state,
                "health": r.health,
                "version": r.version,
                "cpu": r.cpu,
                "memory": r.memory,
                "storage": r.storage,
                "network": r.network,
                "recent_tasks": [],
            }
        }))),
        None => Err(BffError::NotFound(format!("node {} not found", node_id))),
    }
}

#[derive(sqlx::FromRow)]
struct NodeRow {
    node_id: String,
    name: String,
    state: String,
    health: String,
    cpu: String,
    memory: String,
    storage: String,
    network: String,
    version: String,
    maintenance: bool,
    scheduling_paused: bool,
    active_tasks: i32,
    alerts: i32,
}
