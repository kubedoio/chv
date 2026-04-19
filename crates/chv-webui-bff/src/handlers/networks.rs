use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

pub async fn list_networks(
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let page = payload
        .get("page")
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .max(1);
    let page_size = payload
        .get("page_size")
        .and_then(|v| v.as_u64())
        .unwrap_or(50)
        .clamp(1, 200);
    let offset = (page - 1) * page_size;
    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM networks")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to count networks: {}", e)))?;
    let total_pages = (total_count as u64).div_ceil(page_size);

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
            COALESCE(alert_counts.alerts, 0) AS alerts,
            COALESCE(nds.dhcp_enabled, 1) AS dhcp_enabled,
            COALESCE(nds.ipam_mode, 'internal') AS ipam_mode,
            COALESCE(nds.is_default, 0) AS is_default
        FROM networks n
        LEFT JOIN network_observed_state nos ON n.network_id = nos.network_id
        LEFT JOIN network_desired_state nds ON n.network_id = nds.network_id
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
                "dhcp_enabled": r.dhcp_enabled != 0,
                "ipam_mode": r.ipam_mode,
                "is_default": r.is_default != 0,
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
        "filters": {
            "applied": {}
        },
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
            COALESCE(nds.gateway, '') AS gateway,
            COALESCE(nds.dhcp_enabled, 1) AS dhcp_enabled,
            COALESCE(nds.ipam_mode, 'internal') AS ipam_mode,
            COALESCE(nds.is_default, 0) AS is_default
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
                          COALESCE(vos.runtime_status, 'unknown') AS runtime_status,
                          nv.ip_address, nv.mac_address
                   FROM vm_nic_desired_state nv
                   JOIN vms v ON nv.vm_id = v.vm_id
                   LEFT JOIN vm_observed_state vos ON v.vm_id = vos.vm_id
                   WHERE nv.network_id = ?"#,
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
                        "ip_address": vm.ip_address,
                        "mac_address": vm.mac_address,
                    })
                })
                .collect();

            Ok(Json(json!({
                "detail": {
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
                    "dhcp_enabled": r.dhcp_enabled != 0,
                    "ipam_mode": r.ipam_mode,
                    "is_default": r.is_default != 0,
                }
            })))
        }
        None => Err(BffError::NotFound(format!(
            "network {} not found",
            network_id
        ))),
    }
}

pub async fn create_network(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;
    let name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing name".into()))?
        .to_string();

    let cidr = payload
        .get("cidr")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let gateway = payload
        .get("gateway")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let _bridge_name = payload
        .get("bridge_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let dhcp_enabled = payload
        .get("dhcp_enabled")
        .and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|i| i != 0)))
        .unwrap_or(true);

    let ipam_mode = payload
        .get("ipam_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("internal")
        .to_string();

    let is_default = payload
        .get("is_default")
        .and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|i| i != 0)))
        .unwrap_or(false);

    let network_id = chv_common::gen_short_id();

    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| BffError::Internal(format!("failed to begin transaction: {}", e)))?;

    sqlx::query(
        r#"
        INSERT INTO networks (network_id, display_name, network_class, created_at, updated_at)
        VALUES (?, ?, 'bridge', strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&network_id)
    .bind(&name)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert network: {}", e)))?;

    sqlx::query(
        r#"
        INSERT INTO network_desired_state (
            network_id, desired_generation, desired_status, cidr, gateway,
            dhcp_enabled, ipam_mode, is_default, requested_at, updated_at
        )
        VALUES (?, 1, 'Pending', ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&network_id)
    .bind(&cidr)
    .bind(&gateway)
    .bind(if dhcp_enabled { 1 } else { 0 })
    .bind(&ipam_mode)
    .bind(if is_default { 1 } else { 0 })
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert network_desired_state: {}", e)))?;

    tx.commit()
        .await
        .map_err(|e| BffError::Internal(format!("failed to commit transaction: {}", e)))?;

    Ok(Json(json!({
        "network_id": network_id,
        "name": name,
        "cidr": cidr,
        "gateway": gateway,
        "dhcp_enabled": dhcp_enabled,
        "ipam_mode": ipam_mode,
        "is_default": is_default,
    })))
}

pub async fn delete_network(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;
    let network_id = payload
        .get("network_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing network_id".into()))?
        .to_string();

    let attached_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM vm_nic_desired_state WHERE network_id = ?"
    )
    .bind(&network_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to count attached vms: {}", e)))?;

    if attached_count > 0 {
        return Err(BffError::BadRequest(format!(
            "cannot delete network {}: {} VM(s) still attached",
            network_id, attached_count
        )));
    }

    let exists = sqlx::query_scalar::<_, String>("SELECT network_id FROM networks WHERE network_id = ?")
        .bind(&network_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to check network existence: {}", e)))?;

    if exists.is_none() {
        return Err(BffError::NotFound(format!("network {} not found", network_id)));
    }

    sqlx::query("DELETE FROM networks WHERE network_id = ?")
        .bind(&network_id)
        .execute(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to delete network: {}", e)))?;

    Ok(Json(json!({
        "deleted": true,
        "network_id": network_id,
    })))
}

pub async fn update_network(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;
    let network_id = payload
        .get("network_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing network_id".into()))?
        .to_string();

    let exists = sqlx::query_scalar::<_, String>("SELECT network_id FROM networks WHERE network_id = ?")
        .bind(&network_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to check network existence: {}", e)))?;

    if exists.is_none() {
        return Err(BffError::NotFound(format!("network {} not found", network_id)));
    }

    let name = payload.get("name").and_then(|v| v.as_str());
    let cidr = payload.get("cidr").and_then(|v| v.as_str());
    let gateway = payload.get("gateway").and_then(|v| v.as_str());
    let dhcp_enabled = payload.get("dhcp_enabled").and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|i| i != 0))).map(|d| if d { 1 } else { 0 });
    let ipam_mode = payload.get("ipam_mode").and_then(|v| v.as_str());
    let is_default = payload.get("is_default").and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|i| i != 0))).map(|d| if d { 1 } else { 0 });

    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| BffError::Internal(format!("failed to begin transaction: {}", e)))?;

    if let Some(name) = name {
        sqlx::query("UPDATE networks SET display_name = ? WHERE network_id = ?")
            .bind(name)
            .bind(&network_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| BffError::Internal(format!("failed to update network name: {}", e)))?;
    }

    if cidr.is_some() || gateway.is_some() || dhcp_enabled.is_some() || ipam_mode.is_some() || is_default.is_some() {
        sqlx::query(
            r#"
            UPDATE network_desired_state
            SET
                cidr = COALESCE(?, cidr),
                gateway = COALESCE(?, gateway),
                dhcp_enabled = COALESCE(?, dhcp_enabled),
                ipam_mode = COALESCE(?, ipam_mode),
                is_default = COALESCE(?, is_default),
                desired_generation = desired_generation + 1,
                updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
            WHERE network_id = ?
            "#,
        )
        .bind(cidr)
        .bind(gateway)
        .bind(dhcp_enabled)
        .bind(ipam_mode)
        .bind(is_default)
        .bind(&network_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| BffError::Internal(format!("failed to update network desired state: {}", e)))?;
    }

    tx.commit()
        .await
        .map_err(|e| BffError::Internal(format!("failed to commit transaction: {}", e)))?;

    get_network(State(state), axum::Json(json!({ "network_id": network_id }))).await
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
    dhcp_enabled: i32,
    ipam_mode: String,
    is_default: i32,
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
    dhcp_enabled: i32,
    ipam_mode: String,
    is_default: i32,
}

#[derive(sqlx::FromRow)]
struct AttachedVmRow {
    vm_id: String,
    display_name: String,
    runtime_status: String,
    ip_address: Option<String>,
    mac_address: Option<String>,
}

pub async fn mutate_network(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;
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

    let force = payload
        .get("force")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

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
