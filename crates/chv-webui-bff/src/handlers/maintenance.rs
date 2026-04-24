use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

pub async fn get_maintenance(
    crate::auth::BearerToken(_claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    _payload: axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let nodes = sqlx::query_as::<_, MaintenanceNodeRow>(
        r#"
        SELECT
            n.node_id,
            n.display_name AS name,
            COALESCE(nos.observed_state, 'Unknown') AS observed_state,
            COALESCE(nds.desired_state, 'Unknown') AS desired_state,
            COALESCE(nds.state_reason, '') AS reason,
            COALESCE(
                (SELECT operation_id FROM operations
                 WHERE resource_kind = 'node' AND resource_id = n.node_id
                   AND status IN ('Pending', 'Accepted', 'Running')
                 ORDER BY requested_at DESC LIMIT 1),
                ''
            ) AS task_id
        FROM nodes n
        LEFT JOIN node_desired_state nds ON n.node_id = nds.node_id
        LEFT JOIN node_observed_state nos ON n.node_id = nos.node_id
        WHERE nds.desired_state = 'Maintenance' OR nds.scheduling_paused = true
        ORDER BY n.node_id
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to get maintenance nodes: {}", e)))?;

    let windows = sqlx::query_as::<_, MaintenanceWindowRow>(
        r#"
        SELECT
            maintenance_window_id AS window_id,
            reason AS name,
            scope_id AS cluster,
            window_status AS status,
            starts_at AS start_time,
            ends_at AS end_time
        FROM maintenance_windows
        WHERE window_status IN ('active', 'scheduled')
        ORDER BY starts_at
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to get maintenance windows: {}", e)))?;

    let pending_actions = nodes
        .iter()
        .filter(|n| n.observed_state == "Draining")
        .count() as u32;

    let nodes_json: Vec<Value> = nodes
        .into_iter()
        .map(|r| {
            let state_str = if r.desired_state == "Maintenance" {
                "in_maintenance"
            } else if r.observed_state == "Draining" {
                "draining"
            } else {
                "scheduled"
            };
            json!({
                "node_id": r.node_id,
                "name": r.name,
                "cluster": "",
                "state": state_str,
                "task_id": r.task_id,
            })
        })
        .collect();

    let windows_json: Vec<Value> = windows
        .into_iter()
        .map(|r| {
            json!({
                "window_id": r.window_id,
                "name": r.name,
                "cluster": r.cluster,
                "status": r.status,
                "start_time": r.start_time,
                "end_time": r.end_time,
                "affected_nodes": 0,
            })
        })
        .collect();

    Ok(Json(json!({
        "windows": windows_json,
        "nodes": nodes_json,
        "pending_actions": pending_actions,
    })))
}

#[derive(sqlx::FromRow)]
struct MaintenanceNodeRow {
    node_id: String,
    name: String,
    observed_state: String,
    desired_state: String,
    #[allow(dead_code)]
    reason: String,
    task_id: String,
}

#[derive(sqlx::FromRow)]
struct MaintenanceWindowRow {
    window_id: String,
    name: String,
    cluster: String,
    status: String,
    start_time: String,
    end_time: String,
}
