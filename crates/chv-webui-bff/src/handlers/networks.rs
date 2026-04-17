use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

pub async fn list_networks(
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let page = payload.get("page").and_then(|v| v.as_u64()).unwrap_or(1).max(1);
    let page_size = payload.get("page_size").and_then(|v| v.as_u64()).unwrap_or(50).min(200).max(1);
    let offset = (page - 1) * page_size;
    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM networks")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to count networks: {}", e)))?;
    let total_pages = (total_count as u64 + page_size - 1) / page_size;

    let rows = sqlx::query_as::<_, NetworkRow>(
        r#"
        SELECT
            n.network_id,
            n.display_name AS name,
            COALESCE(nos.exposure_status, 'private') AS exposure,
            COALESCE(nos.health_status, 'unknown') AS health,
            (SELECT COUNT(*) FROM vm_nic_desired_state WHERE network_id = n.network_id) AS attached_vms,
            COALESCE(
                (SELECT operation_type FROM operations
                 WHERE resource_kind = 'network' AND resource_id = n.network_id
                 ORDER BY requested_at DESC LIMIT 1),
                ''
            ) AS last_task,
            COALESCE(alert_counts.alerts, 0) AS alerts
        FROM networks n
        LEFT JOIN network_observed_state nos ON n.network_id = nos.network_id
        LEFT JOIN (
            SELECT resource_id, COUNT(*) AS alerts
            FROM alerts
            WHERE status != 'resolved' AND resource_kind = 'network'
            GROUP BY resource_id
        ) alert_counts ON n.network_id = alert_counts.resource_id
        ORDER BY n.network_id
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(page_size as i64)
    .bind(offset as i64)
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
            "page": page,
            "page_size": page_size,
            "total_items": total_count,
            "total_pages": total_pages,
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
            COALESCE(
                (SELECT operation_type FROM operations
                 WHERE resource_kind = 'network' AND resource_id = n.network_id
                 ORDER BY requested_at DESC LIMIT 1),
                ''
            ) AS last_task,
            COALESCE(alert_counts.alerts, 0) AS alerts,
            n.created_at AS created_at,
            COALESCE(nds.cidr, '') AS cidr,
            COALESCE(nds.gateway, '') AS gateway
        FROM networks n
        LEFT JOIN network_observed_state nos ON n.network_id = nos.network_id
        LEFT JOIN network_desired_state nds ON n.network_id = nds.network_id
        LEFT JOIN (
            SELECT resource_id, COUNT(*) AS alerts
            FROM alerts
            WHERE status != 'resolved' AND resource_kind = 'network'
            GROUP BY resource_id
        ) alert_counts ON n.network_id = alert_counts.resource_id
        WHERE n.network_id = ?
        "#,
    )
    .bind(network_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to get network: {}", e)))?;

    match row {
        Some(r) => {
            let attached_vms = sqlx::query_as::<_, AttachedVmRow>(
                r#"SELECT v.vm_id, v.display_name,
                          COALESCE(vos.runtime_status, 'unknown') AS runtime_status
                   FROM vm_nic_desired_state vn
                   JOIN vms v ON vn.vm_id = v.vm_id
                   LEFT JOIN vm_observed_state vos ON v.vm_id = vos.vm_id
                   WHERE vn.network_id = ?"#,
            )
            .bind(&r.network_id)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| BffError::Internal(format!("failed to get attached vms: {}", e)))?;

            let attached_vms_json: Vec<serde_json::Value> = attached_vms
                .iter()
                .map(|vm| {
                    serde_json::json!({
                        "vm_id": vm.vm_id,
                        "display_name": vm.display_name,
                        "runtime_status": vm.runtime_status,
                    })
                })
                .collect();

            Ok(Json(json!({
                "network_id": r.network_id,
                "name": r.name,
                "scope": "fleet",
                "health": r.health,
                "exposure": r.exposure,
                "policy": "default",
                "cidr": r.cidr,
                "gateway": r.gateway,
                "attached_vms": attached_vms_json,
                "created_at": r.created_at.unwrap_or_default(),
                "last_task": r.last_task,
                "alerts": r.alerts,
            })))
        }
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
    cidr: String,
    gateway: String,
}

#[derive(sqlx::FromRow)]
struct AttachedVmRow {
    vm_id: String,
    display_name: String,
    runtime_status: String,
}

pub async fn mutate_network(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let network_id = payload
        .get("network_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing network_id".into()))?
        .to_string();

    let action = payload
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing action".into()))?
        .to_string();

    let force = payload.get("force").and_then(|v| v.as_bool()).unwrap_or(false);

    let response = state
        .mutations
        .mutate_network(network_id, action, force, claims.username)
        .await?;

    Ok(Json(json!({
        "accepted": response.accepted,
        "task_id": response.task_id,
        "network_id": response.network_id,
        "summary": response.summary,
    })))
}
