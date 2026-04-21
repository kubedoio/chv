use axum::{
    extract::{Multipart, State},
    response::Json,
};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;

use crate::router::AppState;
use crate::BffError;

const QCOW2_MAGIC: &[u8] = b"QFI\xfb";
const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024 * 1024; // 100 GiB

pub async fn import_vm(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;

    let mut name: Option<String> = None;
    let mut vm_id: Option<String> = None;
    let mut file_handle: Option<tokio::fs::File> = None;
    let mut header_buf = Vec::new();
    let mut total_size: u64 = 0;

    while let Some(mut field) = multipart.next_field().await.map_err(|e| {
        BffError::BadRequest(format!("multipart error: {}", e))
    })? {
        let field_name = field.name().unwrap_or("").to_string();

        if field_name == "name" {
            name = Some(
                field
                    .text()
                    .await
                    .map_err(|e| BffError::BadRequest(format!("invalid name: {}", e)))?,
            );
            continue;
        }

        if field_name != "file" {
            continue;
        }

        while let Some(chunk) = field.chunk().await.map_err(|e| {
            BffError::BadRequest(format!("chunk error: {}", e))
        })? {
            total_size += chunk.len() as u64;
            if total_size > MAX_FILE_SIZE {
                return Err(BffError::BadRequest(
                    "file exceeds maximum size of 100 GiB".into(),
                ));
            }

            if file_handle.is_none() {
                header_buf.extend_from_slice(&chunk);
                if header_buf.len() >= 4 {
                    if &header_buf[0..4] != QCOW2_MAGIC {
                        return Err(BffError::BadRequest(
                            "file is not a valid qcow2 image".into(),
                        ));
                    }
                    let id = chv_common::gen_short_id();
                    let vm_dir = state.agent_runtime_dir.join("vms").join(&id);
                    tokio::fs::create_dir_all(&vm_dir)
                        .await
                        .map_err(|e| {
                            BffError::Internal(format!("failed to create vm dir: {}", e))
                        })?;
                    let f = tokio::fs::File::create(vm_dir.join("disk.qcow2"))
                        .await
                        .map_err(|e| {
                            BffError::Internal(format!("failed to create disk file: {}", e))
                        })?;
                    file_handle = Some(f);
                    vm_id = Some(id);

                    if let Some(ref mut f) = file_handle {
                        f.write_all(&header_buf).await.map_err(|e| {
                            BffError::Internal(format!("failed to write disk: {}", e))
                        })?;
                    }
                    header_buf.clear();
                }
            } else if let Some(ref mut f) = file_handle {
                f.write_all(&chunk).await.map_err(|e| {
                    BffError::Internal(format!("failed to write disk: {}", e))
                })?;
            }
        }
    }

    let name = name.ok_or_else(|| BffError::BadRequest("missing name".into()))?;
    let vm_id = vm_id.ok_or_else(|| {
        BffError::BadRequest("missing file or file too small to be qcow2".into())
    })?;

    let disk_path = state
        .agent_runtime_dir
        .join("vms")
        .join(&vm_id)
        .join("disk.qcow2");
    let file_size = tokio::fs::metadata(&disk_path)
        .await
        .map_err(|e| BffError::Internal(format!("failed to get file metadata: {}", e)))?
        .len() as i64;

    // Pick default node
    let node_id = sqlx::query_scalar::<_, String>(
        "SELECT node_id FROM nodes ORDER BY enrolled_at DESC LIMIT 1",
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to query nodes: {}", e)))?
    .ok_or_else(|| BffError::BadRequest("no nodes enrolled".into()))?;

    let volume_id = chv_common::gen_short_id();
    let operation_id = chv_common::gen_short_id();

    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| BffError::Internal(format!("failed to begin transaction: {}", e)))?;

    // Insert VM
    sqlx::query(
        r#"
        INSERT INTO vms (vm_id, node_id, display_name, created_at, updated_at)
        VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&vm_id)
    .bind(&node_id)
    .bind(&name)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert vm: {}", e)))?;

    // Insert VM desired state
    let disk_path_str = disk_path.to_string_lossy().to_string();
    sqlx::query(
        r#"
        INSERT INTO vm_desired_state (
            vm_id, desired_generation, desired_status, desired_power_state,
            requested_by, target_node_id, cpu_count, memory_bytes, image_ref,
            requested_at, updated_at
        )
        VALUES (?, 1, 'Pending', 'Running', ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&vm_id)
    .bind(&claims.sub)
    .bind(&node_id)
    .bind(1i64)
    .bind(512i64 * 1024i64 * 1024i64)
    .bind(&disk_path_str)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert vm_desired_state: {}", e)))?;

    // Insert volume
    sqlx::query(
        r#"
        INSERT INTO volumes (volume_id, node_id, display_name, capacity_bytes, updated_at)
        VALUES (?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&volume_id)
    .bind(&node_id)
    .bind(format!("{}-disk", name))
    .bind(file_size)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert volume: {}", e)))?;

    // Insert volume desired state
    sqlx::query(
        r#"
        INSERT INTO volume_desired_state (
            volume_id, desired_generation, desired_status, requested_by,
            attached_vm_id, device_name, read_only, requested_at, updated_at
        )
        VALUES (?, 1, 'Pending', ?, ?, 'vda', 0, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&volume_id)
    .bind(&claims.sub)
    .bind(&vm_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert volume_desired_state: {}", e)))?;

    // Insert operation
    let idempotency_key = format!("import-vm-{}", vm_id);
    sqlx::query(
        r#"
        INSERT INTO operations (
            operation_id, idempotency_key, resource_kind, resource_id,
            operation_type, status, requested_by, desired_generation,
            requested_at, created_at, updated_at
        )
        VALUES (?, ?, 'vm', ?, 'CreateVm', 'Accepted', ?, 1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&operation_id)
    .bind(&idempotency_key)
    .bind(&vm_id)
    .bind(&claims.sub)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert operation: {}", e)))?;

    tx.commit()
        .await
        .map_err(|e| BffError::Internal(format!("failed to commit transaction: {}", e)))?;

    Ok(Json(json!({
        "id": vm_id,
        "name": name,
        "image_id": "imported",
        "storage_pool_id": "",
        "network_id": "",
        "desired_state": "Pending",
        "actual_state": "Unknown",
        "vcpu": 1,
        "memory_mb": 512,
        "disk_path": disk_path_str,
        "seed_iso_path": "",
        "workspace_path": "",
    })))
}
