use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

pub async fn list_nodes(
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, BffError> {
    let page = payload.get("page").and_then(|v| v.as_u64()).unwrap_or(1).max(1);
    let page_size = payload.get("page_size").and_then(|v| v.as_u64()).unwrap_or(50).min(200).max(1);
    let offset = (page - 1) * page_size;
    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM nodes")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to count nodes: {}", e)))?;
    let total_pages = (total_count as u64 + page_size - 1) / page_size;

    let rows = sqlx::query_as::<_, NodeRow>(
        r#"
        SELECT
            n.node_id,
            n.display_name AS name,
            COALESCE(nos.observed_state, 'Unknown') AS state,
            COALESCE(nos.health_status, 'unknown') AS health,
            COALESCE(CAST(ni.cpu_count AS TEXT), '') AS cpu,
            CASE WHEN ni.memory_bytes IS NULL THEN ''
                 WHEN ni.memory_bytes >= 1073741824 THEN printf('%.1f GiB', CAST(ni.memory_bytes AS REAL)/1073741824.0)
                 WHEN ni.memory_bytes >= 1048576 THEN printf('%.1f MiB', CAST(ni.memory_bytes AS REAL)/1048576.0)
                 WHEN ni.memory_bytes >= 1024 THEN printf('%.1f KiB', CAST(ni.memory_bytes AS REAL)/1024.0)
                 ELSE printf('%d B', ni.memory_bytes) END AS memory,
            CASE WHEN ni.disk_bytes IS NULL THEN ''
                 WHEN ni.disk_bytes >= 1073741824 THEN printf('%.1f GiB', CAST(ni.disk_bytes AS REAL)/1073741824.0)
                 WHEN ni.disk_bytes >= 1048576 THEN printf('%.1f MiB', CAST(ni.disk_bytes AS REAL)/1048576.0)
                 WHEN ni.disk_bytes >= 1024 THEN printf('%.1f KiB', CAST(ni.disk_bytes AS REAL)/1024.0)
                 ELSE printf('%d B', ni.disk_bytes) END AS storage,
            '' AS network,
            COALESCE(n.agent_version, '') AS version,
            COALESCE(nds.desired_state = 'Maintenance', false) AS maintenance,
            COALESCE(nds.scheduling_paused, false) AS scheduling_paused,
            COALESCE(task_counts.active_tasks, 0) AS active_tasks,
            COALESCE(alert_counts.alerts, 0) AS alerts
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
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(page_size as i64)
    .bind(offset as i64)
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
            "page": page,
            "page_size": page_size,
            "total_items": total_count,
            "total_pages": total_pages,
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
            COALESCE(nos.observed_state, 'Unknown') AS state,
            COALESCE(nos.health_status, 'unknown') AS health,
            COALESCE(CAST(ni.cpu_count AS TEXT), '') AS cpu,
            CASE WHEN ni.memory_bytes IS NULL THEN ''
                 WHEN ni.memory_bytes >= 1073741824 THEN printf('%.1f GiB', CAST(ni.memory_bytes AS REAL)/1073741824.0)
                 WHEN ni.memory_bytes >= 1048576 THEN printf('%.1f MiB', CAST(ni.memory_bytes AS REAL)/1048576.0)
                 WHEN ni.memory_bytes >= 1024 THEN printf('%.1f KiB', CAST(ni.memory_bytes AS REAL)/1024.0)
                 ELSE printf('%d B', ni.memory_bytes) END AS memory,
            CASE WHEN ni.disk_bytes IS NULL THEN ''
                 WHEN ni.disk_bytes >= 1073741824 THEN printf('%.1f GiB', CAST(ni.disk_bytes AS REAL)/1073741824.0)
                 WHEN ni.disk_bytes >= 1048576 THEN printf('%.1f MiB', CAST(ni.disk_bytes AS REAL)/1048576.0)
                 WHEN ni.disk_bytes >= 1024 THEN printf('%.1f KiB', CAST(ni.disk_bytes AS REAL)/1024.0)
                 ELSE printf('%d B', ni.disk_bytes) END AS storage,
            '' AS network,
            COALESCE(n.agent_version, '') AS version,
            COALESCE(nds.desired_state = 'Maintenance', false) AS maintenance,
            COALESCE(nds.scheduling_paused, false) AS scheduling_paused,
            COALESCE(task_counts.active_tasks, 0) AS active_tasks,
            COALESCE(alert_counts.alerts, 0) AS alerts
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
        Some(r) => {
            let hosted_vms = sqlx::query_as::<_, HostedVmRow>(
                r#"
                SELECT
                    v.vm_id,
                    v.display_name AS name,
                    COALESCE(vds.desired_power_state, vos.runtime_status, 'Unknown') AS power_state,
                    COALESCE(vos.health_status, 'unknown') AS health,
                    COALESCE(CAST(vds.cpu_count AS TEXT), '') AS cpu,
                    CASE WHEN vds.memory_bytes IS NULL THEN ''
                         WHEN vds.memory_bytes >= 1073741824 THEN printf('%.1f GiB', CAST(vds.memory_bytes AS REAL)/1073741824.0)
                         WHEN vds.memory_bytes >= 1048576 THEN printf('%.1f MiB', CAST(vds.memory_bytes AS REAL)/1048576.0)
                         WHEN vds.memory_bytes >= 1024 THEN printf('%.1f KiB', CAST(vds.memory_bytes AS REAL)/1024.0)
                         ELSE printf('%d B', vds.memory_bytes) END AS memory
                FROM vms v
                LEFT JOIN vm_desired_state vds ON v.vm_id = vds.vm_id
                LEFT JOIN vm_observed_state vos ON v.vm_id = vos.vm_id
                WHERE v.node_id = $1
                ORDER BY v.vm_id
                "#,
            )
            .bind(node_id)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| BffError::Internal(format!("failed to get hosted vms: {}", e)))?;

            let hosted_vms_json: Vec<Value> = hosted_vms
                .into_iter()
                .map(|vm| {
                    json!({
                        "vm_id": vm.vm_id,
                        "name": vm.name,
                        "power_state": vm.power_state,
                        "health": vm.health,
                        "cpu": vm.cpu,
                        "memory": vm.memory,
                    })
                })
                .collect();

            let recent_tasks = sqlx::query_as::<_, RecentTaskRow>(
                r#"
                SELECT
                    operation_id AS task_id,
                    status,
                    operation_type AS summary,
                    operation_type AS operation,
                    CAST(strftime('%s', requested_at) AS INTEGER) * 1000 AS started_unix_ms
                FROM operations
                WHERE resource_kind = 'node' AND resource_id = $1
                ORDER BY requested_at DESC
                LIMIT 5
                "#,
            )
            .bind(node_id)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| BffError::Internal(format!("failed to get recent tasks: {}", e)))?;

            let tasks_json: Vec<Value> = recent_tasks
                .into_iter()
                .map(|t| {
                    json!({
                        "task_id": t.task_id,
                        "status": t.status,
                        "summary": t.summary,
                        "operation": t.operation,
                        "started_unix_ms": t.started_unix_ms,
                    })
                })
                .collect();

            let configuration: Vec<Value> = vec![
                json!({"label": "Node ID", "value": r.node_id.clone()}),
                json!({"label": "Version", "value": r.version.clone()}),
                json!({"label": "CPU", "value": r.cpu.clone()}),
                json!({"label": "Memory", "value": r.memory.clone()}),
                json!({"label": "Storage backend", "value": "zfs"}),
            ];

            let sections = vec![
                json!({"id": "summary", "label": "Summary"}),
                json!({"id": "vms", "label": "VMs", "count": hosted_vms_json.len()}),
                json!({"id": "tasks", "label": "Tasks", "count": tasks_json.len()}),
                json!({"id": "configuration", "label": "Configuration"}),
            ];

            Ok(Json(json!({
                "state": "ready",
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
                    "maintenance": r.maintenance,
                    "scheduling": !r.scheduling_paused,
                },
                "sections": sections,
                "hostedVms": hosted_vms_json,
                "recentTasks": tasks_json,
                "configuration": configuration,
            })))
        }
        None => Err(BffError::NotFound(format!("node {} not found", node_id))),
    }
}

pub async fn mutate_node(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let node_id = payload
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing node_id".into()))?
        .to_string();

    let action = payload
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing action".into()))?
        .to_string();

    let response = state.mutations.mutate_node(node_id, action, claims.username).await?;

    Ok(Json(json!({
        "accepted": response.accepted,
        "task_id": response.task_id,
        "node_id": response.node_id,
        "summary": response.summary,
    })))
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

#[derive(sqlx::FromRow)]
struct HostedVmRow {
    vm_id: String,
    name: String,
    power_state: String,
    health: String,
    cpu: String,
    memory: String,
}

#[derive(sqlx::FromRow)]
struct RecentTaskRow {
    task_id: String,
    status: String,
    summary: String,
    operation: String,
    started_unix_ms: Option<i64>,
}
