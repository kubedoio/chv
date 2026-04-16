use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

pub async fn list_volumes(
    State(state): State<AppState>,
    _payload: axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let rows = sqlx::query_as::<_, VolumeRow>(
        r#"
        SELECT
            v.volume_id,
            v.display_name AS name,
            v.node_id,
            COALESCE(vos.health_status, 'unknown') AS health,
            COALESCE(vds.desired_status, vos.runtime_status, 'Unknown') AS status,
            COALESCE(pg_size_pretty(v.capacity_bytes), '') AS size,
            COALESCE(vds.attached_vm_id, '') AS attached_vm_id,
            COALESCE(vms.display_name, '') AS attached_vm_name,
            COALESCE(last_task.operation_type, '') AS last_task
        FROM volumes v
        LEFT JOIN volume_desired_state vds ON v.volume_id = vds.volume_id
        LEFT JOIN vms ON vds.attached_vm_id = vms.vm_id
        LEFT JOIN volume_observed_state vos ON v.volume_id = vos.volume_id
        LEFT JOIN LATERAL (
            SELECT operation_type
            FROM operations
            WHERE resource_kind = 'volume' AND resource_id = v.volume_id
            ORDER BY requested_at DESC
            LIMIT 1
        ) last_task ON true
        ORDER BY v.volume_id
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to list volumes: {}", e)))?;

    let items: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "volume_id": r.volume_id,
                "name": r.name,
                "node_id": r.node_id,
                "health": r.health,
                "size": r.size,
                "attached_vm_id": r.attached_vm_id,
                "attached_vm_name": r.attached_vm_name,
                "status": r.status,
                "last_task": r.last_task,
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

pub async fn get_volume(
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let volume_id = payload
        .get("volume_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing volume_id".into()))?;

    let row = sqlx::query_as::<_, VolumeSummaryRow>(
        r#"
        SELECT
            v.volume_id,
            v.display_name AS name,
            v.node_id,
            COALESCE(vos.health_status, 'unknown') AS health,
            COALESCE(pg_size_pretty(v.capacity_bytes), '') AS size,
            COALESCE(vds.desired_status, vos.runtime_status, 'Unknown') AS status,
            COALESCE(vds.attached_vm_id, '') AS attached_vm_id,
            COALESCE(vms.display_name, '') AS attached_vm_name,
            COALESCE(vds.device_name, '') AS device_name,
            COALESCE(vds.read_only, false) AS read_only,
            COALESCE(v.volume_kind, '') AS volume_kind,
            COALESCE(v.storage_class, '') AS storage_class,
            COALESCE(last_task.operation_type, '') AS last_task
        FROM volumes v
        LEFT JOIN volume_desired_state vds ON v.volume_id = vds.volume_id
        LEFT JOIN vms ON vds.attached_vm_id = vms.vm_id
        LEFT JOIN volume_observed_state vos ON v.volume_id = vos.volume_id
        LEFT JOIN LATERAL (
            SELECT operation_type
            FROM operations
            WHERE resource_kind = 'volume' AND resource_id = v.volume_id
            ORDER BY requested_at DESC
            LIMIT 1
        ) last_task ON true
        WHERE v.volume_id = $1
        "#,
    )
    .bind(volume_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to get volume: {}", e)))?;

    match row {
        Some(r) => {
            let recent_tasks = sqlx::query_as::<_, RecentTaskRow>(
                r#"
                SELECT
                    operation_id AS task_id,
                    status::text AS status,
                    operation_type AS summary,
                    operation_type AS operation,
                    EXTRACT(EPOCH FROM requested_at)::bigint * 1000 AS started_unix_ms
                FROM operations
                WHERE resource_kind = 'volume' AND resource_id = $1
                ORDER BY requested_at DESC
                LIMIT 5
                "#,
            )
            .bind(volume_id)
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

            Ok(Json(json!({
                "summary": {
                    "volume_id": r.volume_id,
                    "name": r.name,
                    "node_id": r.node_id,
                    "health": r.health,
                    "size": r.size,
                    "status": r.status,
                    "attached_vm_id": r.attached_vm_id,
                    "attached_vm_name": r.attached_vm_name,
                    "device_name": r.device_name,
                    "read_only": r.read_only,
                    "volume_kind": r.volume_kind,
                    "storage_class": r.storage_class,
                    "last_task": r.last_task,
                    "recent_tasks": tasks_json,
                }
            })))
        }
        None => Err(BffError::NotFound(format!("volume {} not found", volume_id))),
    }
}

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

#[derive(sqlx::FromRow)]
struct VolumeRow {
    volume_id: String,
    name: String,
    node_id: Option<String>,
    health: String,
    size: String,
    attached_vm_id: String,
    attached_vm_name: String,
    status: String,
    last_task: String,
}

#[derive(sqlx::FromRow)]
struct VolumeSummaryRow {
    volume_id: String,
    name: String,
    node_id: Option<String>,
    health: String,
    size: String,
    status: String,
    attached_vm_id: String,
    attached_vm_name: String,
    device_name: String,
    read_only: bool,
    volume_kind: String,
    storage_class: String,
    last_task: String,
}

#[derive(sqlx::FromRow)]
struct RecentTaskRow {
    task_id: String,
    status: String,
    summary: String,
    operation: String,
    started_unix_ms: i64,
}
