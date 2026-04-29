use axum::{
    extract::{Path, State},
    response::Json,
};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

#[derive(sqlx::FromRow)]
struct QuotaRow {
    user_id: String,
    max_vms: Option<i64>,
    max_cpu: Option<i64>,
    max_memory_bytes: Option<i64>,
    max_storage_bytes: Option<i64>,
}

pub async fn list_quotas(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;

    let rows = sqlx::query_as::<_, QuotaRow>(
        "SELECT user_id, max_vms, max_cpu, max_memory_bytes, max_storage_bytes FROM quotas ORDER BY user_id"
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to list quotas: {}", e)))?;

    let items: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "user_id": r.user_id,
                "max_vms": r.max_vms,
                "max_cpu": r.max_cpu,
                "max_memory_gb": r.max_memory_bytes.map(|b| b / (1024 * 1024 * 1024)),
                "max_storage_gb": r.max_storage_bytes.map(|b| b / (1024 * 1024 * 1024)),
            })
        })
        .collect();

    Ok(Json(json!({ "items": items })))
}

pub async fn get_quota(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;

    let row = sqlx::query_as::<_, QuotaRow>(
        "SELECT user_id, max_vms, max_cpu, max_memory_bytes, max_storage_bytes FROM quotas WHERE user_id = ?"
    )
    .bind(&user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to get quota: {}", e)))?;

    let row =
        row.ok_or_else(|| BffError::NotFound(format!("quota for user {} not found", user_id)))?;

    Ok(Json(json!({
        "user_id": row.user_id,
        "max_vms": row.max_vms,
        "max_cpu": row.max_cpu,
        "max_memory_gb": row.max_memory_bytes.map(|b| b / (1024 * 1024 * 1024)),
        "max_storage_gb": row.max_storage_bytes.map(|b| b / (1024 * 1024 * 1024)),
    })))
}

pub async fn get_my_quota(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
) -> Result<Json<Value>, BffError> {
    let user_id = &claims.sub;

    let row = sqlx::query_as::<_, QuotaRow>(
        "SELECT user_id, max_vms, max_cpu, max_memory_bytes, max_storage_bytes FROM quotas WHERE user_id = ?"
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to get quota: {}", e)))?;

    let quota = match row {
        Some(r) => json!({
            "user_id": r.user_id,
            "max_vms": r.max_vms,
            "max_cpu": r.max_cpu,
            "max_memory_gb": r.max_memory_bytes.map(|b| b / (1024 * 1024 * 1024)),
            "max_storage_gb": r.max_storage_bytes.map(|b| b / (1024 * 1024 * 1024)),
        }),
        None => json!({
            "user_id": user_id,
            "max_vms": null,
            "max_cpu": null,
            "max_memory_gb": null,
            "max_storage_gb": null,
        }),
    };

    Ok(Json(quota))
}

pub async fn create_quota(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;

    let user_id = payload
        .get("user_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing user_id".into()))?;

    let max_vms = payload.get("max_vms").and_then(|v| v.as_i64());
    let max_cpu = payload.get("max_cpu").and_then(|v| v.as_i64());
    let max_memory_bytes = payload
        .get("max_memory_gb")
        .and_then(|v| v.as_i64())
        .map(|gb| gb * 1024 * 1024 * 1024);
    let max_storage_bytes = payload
        .get("max_storage_gb")
        .and_then(|v| v.as_i64())
        .map(|gb| gb * 1024 * 1024 * 1024);

    sqlx::query(
        r#"
        INSERT INTO quotas (user_id, max_vms, max_cpu, max_memory_bytes, max_storage_bytes, updated_at)
        VALUES (?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        ON CONFLICT (user_id) DO UPDATE SET
            max_vms = excluded.max_vms,
            max_cpu = excluded.max_cpu,
            max_memory_bytes = excluded.max_memory_bytes,
            max_storage_bytes = excluded.max_storage_bytes,
            updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
        "#,
    )
    .bind(user_id)
    .bind(max_vms)
    .bind(max_cpu)
    .bind(max_memory_bytes)
    .bind(max_storage_bytes)
    .execute(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to create quota: {}", e)))?;
    state.cache.invalidate("overview").await;


    Ok(Json(json!({
        "user_id": user_id,
        "max_vms": max_vms,
        "max_cpu": max_cpu,
        "max_memory_gb": max_memory_bytes.map(|b| b / (1024 * 1024 * 1024)),
        "max_storage_gb": max_storage_bytes.map(|b| b / (1024 * 1024 * 1024)),
    })))
}

pub async fn update_quota(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;

    let exists: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM quotas WHERE user_id = ?")
        .bind(&user_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("db error: {}", e)))?;

    if !exists {
        return Err(BffError::NotFound(format!(
            "quota for user {} not found",
            user_id
        )));
    }

    let max_vms = payload.get("max_vms").and_then(|v| v.as_i64());
    let max_cpu = payload.get("max_cpu").and_then(|v| v.as_i64());
    let max_memory_bytes = payload
        .get("max_memory_gb")
        .and_then(|v| v.as_i64())
        .map(|gb| gb * 1024 * 1024 * 1024);
    let max_storage_bytes = payload
        .get("max_storage_gb")
        .and_then(|v| v.as_i64())
        .map(|gb| gb * 1024 * 1024 * 1024);

    sqlx::query(
        r#"
        UPDATE quotas SET
            max_vms = ?,
            max_cpu = ?,
            max_memory_bytes = ?,
            max_storage_bytes = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
        WHERE user_id = ?
        "#,
    )
    .bind(max_vms)
    .bind(max_cpu)
    .bind(max_memory_bytes)
    .bind(max_storage_bytes)
    .bind(&user_id)
    .execute(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to update quota: {}", e)))?;
    state.cache.invalidate("overview").await;


    Ok(Json(json!({
        "user_id": user_id,
        "max_vms": max_vms,
        "max_cpu": max_cpu,
        "max_memory_gb": max_memory_bytes.map(|b| b / (1024 * 1024 * 1024)),
        "max_storage_gb": max_storage_bytes.map(|b| b / (1024 * 1024 * 1024)),
    })))
}

pub async fn delete_quota(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;

    sqlx::query("DELETE FROM quotas WHERE user_id = ?")
        .bind(&user_id)
        .execute(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to delete quota: {}", e)))?;
    state.cache.invalidate("overview").await;


    Ok(Json(json!({
        "deleted": true,
        "user_id": user_id,
    })))
}

pub async fn get_usage(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
) -> Result<Json<Value>, BffError> {
    let user_id = &claims.sub;

    // Count VMs owned by user
    let vm_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM vms v
           JOIN vm_desired_state vds ON v.vm_id = vds.vm_id
           WHERE vds.requested_by = ?"#,
    )
    .bind(user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to count vms: {}", e)))?;

    // Sum CPU and memory for user's VMs
    let usage_row = sqlx::query_as::<_, (Option<i64>, Option<i64>, Option<i64>)>(
        r#"SELECT
             COALESCE(SUM(vds.cpu_count), 0),
             COALESCE(SUM(vds.memory_bytes), 0),
             COALESCE(SUM(vol.capacity_bytes), 0)
           FROM vm_desired_state vds
           LEFT JOIN volume_desired_state vd ON vd.attached_vm_id = vds.vm_id
           LEFT JOIN volumes vol ON vol.volume_id = vd.volume_id
           WHERE vds.requested_by = ?"#,
    )
    .bind(user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to compute usage: {}", e)))?;

    let cpu_cores = usage_row.0.unwrap_or(0);
    let memory_bytes = usage_row.1.unwrap_or(0);
    let storage_bytes = usage_row.2.unwrap_or(0);

    // Fetch quota
    let quota_row = sqlx::query_as::<_, QuotaRow>(
        "SELECT user_id, max_vms, max_cpu, max_memory_bytes, max_storage_bytes FROM quotas WHERE user_id = ?"
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to get quota: {}", e)))?;

    let quota = quota_row.map(|r| {
        json!({
            "max_vms": r.max_vms,
            "max_cpu": r.max_cpu,
            "max_memory_gb": r.max_memory_bytes.map(|b| b / (1024 * 1024 * 1024)),
            "max_storage_gb": r.max_storage_bytes.map(|b| b / (1024 * 1024 * 1024)),
        })
    });

    Ok(Json(json!({
        "usage": {
            "vms": vm_count,
            "cpu_cores": cpu_cores,
            "memory_mb": memory_bytes / (1024 * 1024),
            "disk_gb": storage_bytes / (1024 * 1024 * 1024),
        },
        "quota": quota,
    })))
}

pub async fn check_quota(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let user_id = &claims.sub;

    let requested_vms = payload
        .get("requested_vms")
        .and_then(|v| v.as_i64())
        .unwrap_or(1);
    let requested_cpu = payload
        .get("requested_cpu")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let requested_memory_bytes = payload
        .get("requested_memory_gb")
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
        * 1024
        * 1024
        * 1024;
    let requested_storage_bytes = payload
        .get("requested_storage_gb")
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
        * 1024
        * 1024
        * 1024;

    let quota_row = sqlx::query_as::<_, QuotaRow>(
        "SELECT user_id, max_vms, max_cpu, max_memory_bytes, max_storage_bytes FROM quotas WHERE user_id = ?"
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to get quota: {}", e)))?;

    let quota = match quota_row {
        Some(r) => r,
        None => {
            return Ok(Json(json!({
                "allowed": true,
                "reason": "no quota set",
            })));
        }
    };

    // Count current usage
    let vm_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM vms v
           JOIN vm_desired_state vds ON v.vm_id = vds.vm_id
           WHERE vds.requested_by = ?"#,
    )
    .bind(user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to count vms: {}", e)))?;

    let usage_row = sqlx::query_as::<_, (Option<i64>, Option<i64>, Option<i64>)>(
        r#"SELECT
             COALESCE(SUM(vds.cpu_count), 0),
             COALESCE(SUM(vds.memory_bytes), 0),
             COALESCE(SUM(vol.capacity_bytes), 0)
           FROM vm_desired_state vds
           LEFT JOIN volume_desired_state vd ON vd.attached_vm_id = vds.vm_id
           LEFT JOIN volumes vol ON vol.volume_id = vd.volume_id
           WHERE vds.requested_by = ?"#,
    )
    .bind(user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to compute usage: {}", e)))?;

    let current_cpu = usage_row.0.unwrap_or(0);
    let current_memory = usage_row.1.unwrap_or(0);
    let current_storage = usage_row.2.unwrap_or(0);

    let mut violations = Vec::new();

    if let Some(max) = quota.max_vms {
        if vm_count + requested_vms > max {
            violations.push(format!(
                "VM quota exceeded: {} + {} > {}",
                vm_count, requested_vms, max
            ));
        }
    }
    if let Some(max) = quota.max_cpu {
        if current_cpu + requested_cpu > max {
            violations.push(format!(
                "CPU quota exceeded: {} + {} > {}",
                current_cpu, requested_cpu, max
            ));
        }
    }
    if let Some(max) = quota.max_memory_bytes {
        if current_memory + requested_memory_bytes > max {
            violations.push("Memory quota exceeded".to_string());
        }
    }
    if let Some(max) = quota.max_storage_bytes {
        if current_storage + requested_storage_bytes > max {
            violations.push("Storage quota exceeded".to_string());
        }
    }

    let allowed = violations.is_empty();

    Ok(Json(json!({
        "allowed": allowed,
        "violations": violations,
        "current_usage": {
            "vms": vm_count,
            "cpu_cores": current_cpu,
            "memory_mb": current_memory / (1024 * 1024),
            "disk_gb": current_storage / (1024 * 1024 * 1024),
        },
    })))
}
