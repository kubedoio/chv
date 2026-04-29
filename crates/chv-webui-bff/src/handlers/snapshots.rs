use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::auth::BearerToken;
use crate::router::AppState;
use crate::BffError;

#[derive(sqlx::FromRow)]
struct SnapshotRow {
    snapshot_id: String,
    vm_id: String,
    name: String,
    description: String,
    size_bytes: i64,
    includes_memory: i64,
    snapshot_path: String,
    status: String,
    created_at: String,
}

pub async fn list_vm_snapshots(
    BearerToken(_claims): BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let vm_id = payload
        .get("vm_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing vm_id".into()))?;

    let rows = sqlx::query_as::<_, SnapshotRow>(
        r#"
        SELECT snapshot_id, vm_id, name, description, size_bytes,
               includes_memory, snapshot_path, status, created_at
        FROM vm_snapshots
        WHERE vm_id = ?
        ORDER BY created_at DESC
        "#,
    )
    .bind(vm_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to list snapshots: {}", e)))?;

    let items: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.snapshot_id,
                "snapshot_id": r.snapshot_id,
                "vm_id": r.vm_id,
                "name": r.name,
                "description": r.description,
                "size_bytes": r.size_bytes,
                "includes_memory": r.includes_memory != 0,
                "snapshot_path": r.snapshot_path,
                "status": r.status,
                "created_at": r.created_at,
            })
        })
        .collect();

    Ok(Json(json!({ "items": items })))
}

pub async fn create_snapshot(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;
    let vm_id = payload
        .get("vm_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing vm_id".into()))?
        .to_string();

    if !chv_common::validate_id(&vm_id) {
        return Err(BffError::BadRequest("invalid vm_id format".into()));
    }

    let name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing name".into()))?
        .to_string();

    let description = payload
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let includes_memory = payload
        .get("includes_memory")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Verify the VM exists
    let exists = sqlx::query_scalar::<_, String>("SELECT vm_id FROM vms WHERE vm_id = ?")
        .bind(&vm_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to check vm existence: {}", e)))?;

    if exists.is_none() {
        return Err(BffError::NotFound(format!("vm {} not found", vm_id)));
    }

    let snapshot_id = chv_common::gen_short_id();
    let snapshot_path = state
        .agent_runtime_dir
        .join("vms")
        .join(&vm_id)
        .join("snapshots")
        .join(&snapshot_id);
    let snapshot_path = snapshot_path.to_string_lossy().to_string();

    sqlx::query(
        r#"
        INSERT INTO vm_snapshots
            (snapshot_id, vm_id, name, description, includes_memory, snapshot_path, status)
        VALUES (?, ?, ?, ?, ?, ?, 'creating')
        "#,
    )
    .bind(&snapshot_id)
    .bind(&vm_id)
    .bind(&name)
    .bind(&description)
    .bind(includes_memory as i64)
    .bind(&snapshot_path)
    .execute(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert snapshot: {}", e)))?;

    // Dispatch snapshot operation through control plane
    let response = state
        .mutations
        .snapshot_vm(
            vm_id.clone(),
            snapshot_path.clone(),
            claims.username.clone(),
        )
        .await
        .map_err(|e| BffError::Internal(format!("failed to dispatch snapshot: {:?}", e)))?;

    tracing::info!(
        snapshot_id = %snapshot_id,
        vm_id = %vm_id,
        task_id = %response.task_id,
        "snapshot dispatched"
    );
    state.cache.invalidate("vms:").await;
    state.cache.invalidate("overview").await;


    Ok(Json(json!({
        "snapshot_id": snapshot_id,
        "vm_id": vm_id,
        "name": name,
        "description": description,
        "includes_memory": includes_memory,
        "snapshot_path": snapshot_path,
        "status": "creating",
        "task_id": response.task_id,
    })))
}

pub async fn delete_snapshot(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;
    let snapshot_id = payload
        .get("snapshot_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing snapshot_id".into()))?
        .to_string();

    // Fetch the snapshot to get its vm_id and verify it exists before deleting
    let snapshot = sqlx::query_as::<_, SnapshotRow>(
        "SELECT snapshot_id, vm_id, name, description, size_bytes, \
         includes_memory, snapshot_path, status, created_at \
         FROM vm_snapshots WHERE snapshot_id = ?",
    )
    .bind(&snapshot_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to look up snapshot: {}", e)))?
    .ok_or_else(|| BffError::NotFound(format!("snapshot {} not found", snapshot_id)))?;

    // Verify the VM this snapshot belongs to still exists (ownership check)
    let vm_exists = sqlx::query_scalar::<_, String>("SELECT vm_id FROM vms WHERE vm_id = ?")
        .bind(&snapshot.vm_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to verify vm access: {}", e)))?;

    if vm_exists.is_none() {
        return Err(BffError::NotFound(format!(
            "vm {} for snapshot {} not found",
            snapshot.vm_id, snapshot_id
        )));
    }

    sqlx::query("DELETE FROM vm_snapshots WHERE snapshot_id = ?")
        .bind(&snapshot_id)
        .execute(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to delete snapshot: {}", e)))?;
    state.cache.invalidate("vms:").await;
    state.cache.invalidate("overview").await;


    Ok(Json(json!({
        "snapshot_id": snapshot_id,
        "status": "deleted",
    })))
}

pub async fn restore_snapshot(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;
    let snapshot_id = payload
        .get("snapshot_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing snapshot_id".into()))?
        .to_string();

    let row = sqlx::query_as::<_, SnapshotRow>(
        r#"
        SELECT snapshot_id, vm_id, name, description, size_bytes,
               includes_memory, snapshot_path, status, created_at
        FROM vm_snapshots
        WHERE snapshot_id = ?
        "#,
    )
    .bind(&snapshot_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to look up snapshot: {}", e)))?;

    let snapshot =
        row.ok_or_else(|| BffError::NotFound(format!("snapshot {} not found", snapshot_id)))?;

    // Check VM power state — must be stopped before restoring a snapshot
    let runtime_status: Option<String> =
        sqlx::query_scalar("SELECT runtime_status FROM vm_observed_state WHERE vm_id = ?")
            .bind(&snapshot.vm_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| BffError::Internal(format!("failed to get vm power state: {}", e)))?
            .flatten();

    let is_running = runtime_status
        .as_deref()
        .map(|s| matches!(s, "Running" | "Starting" | "Resuming"))
        .unwrap_or(false);

    if is_running {
        return Err(BffError::BadRequest(
            "VM must be stopped before restoring a snapshot".into(),
        ));
    }

    // Dispatch restore operation through control plane
    let response = state
        .mutations
        .restore_snapshot(
            snapshot.vm_id.clone(),
            snapshot.snapshot_path.clone(),
            claims.username.clone(),
        )
        .await
        .map_err(|e| BffError::Internal(format!("failed to dispatch restore: {:?}", e)))?;

    tracing::info!(
        snapshot_id = %snapshot_id,
        vm_id = %snapshot.vm_id,
        task_id = %response.task_id,
        "restore dispatched"
    );
    state.cache.invalidate("vms:").await;
    state.cache.invalidate("overview").await;


    Ok(Json(json!({
        "snapshot_id": snapshot_id,
        "vm_id": snapshot.vm_id,
        "snapshot_path": snapshot.snapshot_path,
        "status": "restoring",
        "task_id": response.task_id,
    })))
}
