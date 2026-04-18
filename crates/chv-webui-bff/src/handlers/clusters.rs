use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

pub async fn list_clusters(
    State(state): State<AppState>,
    _payload: axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let row = sqlx::query_as::<_, ClusterRow>(
        r#"
        SELECT
            COUNT(*) AS node_count,
            SUM(CASE WHEN nos.health_status IN ('degraded', 'warning', 'critical') THEN 1 ELSE 0 END) AS degraded_count,
            SUM(CASE WHEN nds.desired_state = 'Maintenance' THEN 1 ELSE 0 END) AS maintenance_count,
            (SELECT agent_version FROM nodes GROUP BY agent_version ORDER BY COUNT(*) DESC LIMIT 1) AS version,
            CASE WHEN COUNT(DISTINCT n.agent_version) > 1 THEN 1 ELSE 0 END AS version_skew,
            (SELECT COUNT(*) FROM operations WHERE status IN ('Pending', 'Accepted', 'Running')) AS active_tasks,
            (SELECT COUNT(*) FROM alerts WHERE status != 'resolved') AS alerts
        FROM nodes n
        LEFT JOIN node_observed_state nos ON n.node_id = nos.node_id
        LEFT JOIN node_desired_state nds ON n.node_id = nds.node_id
        "#,
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to list clusters: {}", e)))?;

    let state_str = if row.degraded_count > 0 {
        "degraded"
    } else if row.maintenance_count > 0 {
        "warning"
    } else {
        "healthy"
    };

    let top_issue = if row.version_skew.unwrap_or(false) {
        Some("Version skew")
    } else if row.degraded_count > 0 {
        Some("Node health degraded")
    } else {
        None
    };

    let items = vec![json!({
        "cluster_id": "c-fleet-1",
        "name": "default-fleet",
        "datacenter": "Primary",
        "node_count": row.node_count,
        "state": state_str,
        "maintenance": row.maintenance_count > 0,
        "version": row.version.unwrap_or_else(|| "unknown".into()),
        "version_skew": row.version_skew.unwrap_or(false),
        "cpu_percent": 55,
        "memory_percent": 58,
        "storage_percent": 45,
        "active_tasks": row.active_tasks,
        "alerts": row.alerts,
        "top_issue": top_issue,
    })];

    Ok(Json(json!({
        "items": items,
        "page": {
            "page": 1,
            "page_size": 50,
            "total_items": items.len() as u64,
        },
        "filters": {
            "applied": {}
        },
    })))
}

#[derive(sqlx::FromRow)]
struct ClusterRow {
    node_count: i64,
    degraded_count: i64,
    maintenance_count: i64,
    version: Option<String>,
    version_skew: Option<bool>,
    active_tasks: i64,
    alerts: i64,
}
