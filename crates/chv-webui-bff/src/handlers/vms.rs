use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

pub async fn list_vms(
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
    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vms")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to count vms: {}", e)))?;
    let total_pages = (total_count as u64).div_ceil(page_size);

    let rows = sqlx::query_as::<_, VmRow>(
        r#"
        SELECT
            v.vm_id,
            v.display_name AS name,
            COALESCE(vos.node_id, vds.target_node_id, v.node_id) AS node_id,
            COALESCE(vds.desired_power_state, vos.runtime_status, 'Unknown') AS power_state,
            COALESCE(vos.health_status, 'unknown') AS health,
            COALESCE(CAST(vds.cpu_count AS TEXT), '') AS cpu,
            CASE WHEN vds.memory_bytes IS NULL THEN ''
                 WHEN vds.memory_bytes >= 1073741824 THEN printf('%.1f GiB', CAST(vds.memory_bytes AS REAL)/1073741824.0)
                 WHEN vds.memory_bytes >= 1048576 THEN printf('%.1f MiB', CAST(vds.memory_bytes AS REAL)/1048576.0)
                 WHEN vds.memory_bytes >= 1024 THEN printf('%.1f KiB', CAST(vds.memory_bytes AS REAL)/1024.0)
                 ELSE printf('%d B', vds.memory_bytes) END AS memory,
            COALESCE(volume_counts.volume_count, 0) AS volume_count,
            COALESCE(nic_counts.nic_count, 0) AS nic_count,
            COALESCE(
                (SELECT operation_id FROM operations
                 WHERE resource_kind = 'vm' AND resource_id = v.vm_id
                 ORDER BY requested_at DESC LIMIT 1),
                ''
            ) AS last_task
        FROM vms v
        LEFT JOIN vm_desired_state vds ON v.vm_id = vds.vm_id
        LEFT JOIN vm_observed_state vos ON v.vm_id = vos.vm_id
        LEFT JOIN (
            SELECT attached_vm_id, COUNT(*) AS volume_count
            FROM volume_desired_state
            WHERE attached_vm_id IS NOT NULL
            GROUP BY attached_vm_id
        ) volume_counts ON v.vm_id = volume_counts.attached_vm_id
        LEFT JOIN (
            SELECT vm_id, COUNT(*) AS nic_count
            FROM vm_nic_desired_state
            GROUP BY vm_id
        ) nic_counts ON v.vm_id = nic_counts.vm_id
        ORDER BY v.vm_id
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(page_size as i64)
    .bind(offset as i64)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to list vms: {}", e)))?;

    let items: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "vm_id": r.vm_id,
                "name": r.name,
                "node_id": r.node_id,
                "power_state": r.power_state,
                "health": r.health,
                "cpu": r.cpu,
                "memory": r.memory,
                "volume_count": r.volume_count,
                "nic_count": r.nic_count,
                "last_task": r.last_task,
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

pub async fn get_vm(
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let vm_id = payload
        .get("vm_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing vm_id".into()))?;

    let row = sqlx::query_as::<_, VmSummaryRow>(
        r#"
        SELECT
            v.vm_id,
            v.display_name AS name,
            COALESCE(vos.node_id, vds.target_node_id, v.node_id) AS node_id,
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
        WHERE v.vm_id = $1
        "#,
    )
    .bind(vm_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| BffError::Internal(format!("failed to get vm: {}", e)))?;

    match row {
        Some(r) => {
            let recent_tasks = sqlx::query_as::<_, RecentTaskRow>(
                r#"
                SELECT
                    operation_id AS task_id,
                    status,
                    operation_type AS summary,
                    CAST(strftime('%s', requested_at) AS INTEGER) * 1000 AS started_unix_ms
                FROM operations
                WHERE resource_kind = 'vm' AND resource_id = $1
                ORDER BY requested_at DESC
                LIMIT 5
                "#,
            )
            .bind(vm_id)
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
                        "started_unix_ms": t.started_unix_ms,
                    })
                })
                .collect();

            let attached_volumes = sqlx::query_as::<_, VmVolumeRow>(
                r#"
                SELECT
                    v.volume_id,
                    v.display_name AS name,
                    CASE WHEN v.capacity_bytes IS NULL THEN ''
                         WHEN v.capacity_bytes >= 1073741824 THEN printf('%.1f GiB', CAST(v.capacity_bytes AS REAL)/1073741824.0)
                         WHEN v.capacity_bytes >= 1048576 THEN printf('%.1f MiB', CAST(v.capacity_bytes AS REAL)/1048576.0)
                         WHEN v.capacity_bytes >= 1024 THEN printf('%.1f KiB', CAST(v.capacity_bytes AS REAL)/1024.0)
                         ELSE printf('%d B', v.capacity_bytes) END AS size,
                    COALESCE(vds.device_name, '') AS device_name,
                    COALESCE(vds.read_only, false) AS read_only,
                    COALESCE(vos.health_status, 'unknown') AS health
                FROM volume_desired_state vds
                JOIN volumes v ON vds.volume_id = v.volume_id
                LEFT JOIN volume_observed_state vos ON v.volume_id = vos.volume_id
                WHERE vds.attached_vm_id = $1
                "#,
            )
            .bind(vm_id)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| BffError::Internal(format!("failed to get vm volumes: {}", e)))?;

            let volumes_json: Vec<Value> = attached_volumes
                .into_iter()
                .map(|v| {
                    json!({
                        "volume_id": v.volume_id,
                        "name": v.name,
                        "size": v.size,
                        "device_name": v.device_name,
                        "read_only": v.read_only,
                        "health": v.health,
                    })
                })
                .collect();

            let attached_nics = sqlx::query_as::<_, VmNicRow>(
                r#"
                SELECT
                    nv.nic_id,
                    nv.network_id,
                    COALESCE(n.display_name, nv.network_id) AS network_name,
                    COALESCE(nv.mac_address, '') AS mac_address,
                    COALESCE(nv.ip_address, '') AS ip_address,
                    COALESCE(nv.nic_model, 'virtio') AS nic_model
                FROM vm_nic_desired_state nv
                LEFT JOIN networks n ON nv.network_id = n.network_id
                WHERE nv.vm_id = $1
                "#,
            )
            .bind(vm_id)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| BffError::Internal(format!("failed to get vm nics: {}", e)))?;

            let nics_json: Vec<Value> = attached_nics
                .into_iter()
                .map(|n| {
                    json!({
                        "nic_id": n.nic_id,
                        "network_id": n.network_id,
                        "network_name": n.network_name,
                        "mac_address": n.mac_address,
                        "ip_address": n.ip_address,
                        "nic_model": n.nic_model,
                    })
                })
                .collect();

            Ok(Json(json!({
                "summary": {
                    "vm_id": r.vm_id,
                    "name": r.name,
                    "node_id": r.node_id,
                    "power_state": r.power_state,
                    "health": r.health,
                    "cpu": r.cpu,
                    "memory": r.memory,
                    "recent_tasks": tasks_json,
                    "attached_volumes": volumes_json,
                    "attached_nics": nics_json,
                }
            })))
        }
        None => Err(BffError::NotFound(format!("vm {} not found", vm_id))),
    }
}

pub async fn create_vm(
    crate::auth::BearerToken(_claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    tracing::info!("create_vm handler started");

    // Support both legacy BFF payload and CreateVMModal payload
    let display_name = payload
        .get("display_name")
        .and_then(|v| v.as_str())
        .or_else(|| payload.get("name").and_then(|v| v.as_str()))
        .ok_or_else(|| BffError::BadRequest("missing name/display_name".into()))?
        .to_string();

    tracing::info!(%display_name, "create_vm: parsed display_name");

    let node_id = if let Some(nid) = payload.get("node_id").and_then(|v| v.as_str()) {
        nid.to_string()
    } else {
        // Pick the first enrolled node as default
        tracing::info!("create_vm: looking up default node");
        sqlx::query_scalar::<_, String>(
            "SELECT node_id FROM nodes ORDER BY enrolled_at DESC LIMIT 1",
        )
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to query nodes: {}", e)))?
        .ok_or_else(|| BffError::BadRequest("no nodes enrolled and node_id not provided".into()))?
    };
    tracing::info!(%node_id, "create_vm: selected node");

    let cpu_count = payload
        .get("cpu_count")
        .and_then(|v| v.as_i64())
        .or_else(|| payload.get("vcpu").and_then(|v| v.as_i64()))
        .unwrap_or(2);

    let memory_bytes = payload
        .get("memory_bytes")
        .and_then(|v| v.as_i64())
        .or_else(|| payload.get("memory_mb").and_then(|v| v.as_i64()))
        .map(|mb| mb * 1024 * 1024)
        .unwrap_or(2 * 1024 * 1024 * 1024);

    let mut image_ref = payload
        .get("image_ref")
        .and_then(|v| v.as_str())
        .or_else(|| payload.get("image_id").and_then(|v| v.as_str()))
        .unwrap_or("default")
        .to_string();
    tracing::info!(%image_ref, "create_vm: initial image_ref");

    // If image_id is a real image UUID (not "default"), look up its source_url/path.
    if image_ref != "default" && !image_ref.is_empty() {
        tracing::info!(%image_ref, "create_vm: looking up image in DB");
        if let Some(source_url) = sqlx::query_scalar::<_, Option<String>>(
            "SELECT source_url FROM images WHERE image_id = ?"
        )
        .bind(&image_ref)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to look up image: {}", e)))?
        .flatten()
        {
            tracing::info!(%source_url, "create_vm: resolved image_ref to source_url");
            image_ref = source_url;
        } else {
            tracing::warn!(%image_ref, "create_vm: image not found in DB, keeping original image_ref");
        }
    }

    let requested_by = payload
        .get("requested_by")
        .and_then(|v| v.as_str())
        .unwrap_or("webui")
        .to_string();

    let network_id = payload
        .get("network_id")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();

    let volume_size_bytes = payload
        .get("volume_size_gb")
        .and_then(|v| v.as_i64())
        .unwrap_or(10)
        * 1024
        * 1024
        * 1024;

    let vm_id = uuid::Uuid::new_v4().to_string();
    let volume_id = uuid::Uuid::new_v4().to_string();
    let operation_id = uuid::Uuid::new_v4().to_string();
    tracing::info!(%vm_id, %volume_id, %operation_id, "create_vm: generated IDs, beginning transaction");
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
    .bind(&display_name)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert vm: {}", e)))?;

    // Insert VM desired state
    sqlx::query(
        r#"
        INSERT INTO vm_desired_state (vm_id, desired_generation, desired_status, requested_by, target_node_id, cpu_count, memory_bytes, image_ref, requested_at, updated_at)
        VALUES (?, 1, 'Pending', ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&vm_id)
    .bind(&requested_by)
    .bind(&node_id)
    .bind(cpu_count)
    .bind(memory_bytes)
    .bind(&image_ref)
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
    .bind(format!("{}-disk", display_name))
    .bind(volume_size_bytes)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert volume: {}", e)))?;

    // Insert volume desired state (attached to VM)
    sqlx::query(
        r#"
        INSERT INTO volume_desired_state (volume_id, desired_generation, desired_status, requested_by, attached_vm_id, device_name, read_only, requested_at, updated_at)
        VALUES (?, 1, 'Pending', ?, ?, 'vda', 0, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&volume_id)
    .bind(&requested_by)
    .bind(&vm_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert volume_desired_state: {}", e)))?;

    // Insert network if not exists
    let network_exists: Option<String> =
        sqlx::query_scalar("SELECT network_id FROM networks WHERE network_id = ?")
            .bind(&network_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| BffError::Internal(format!("failed to check network: {}", e)))?;

    if network_exists.is_none() {
        sqlx::query(
            r#"
            INSERT INTO networks (network_id, node_id, display_name, updated_at)
            VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ','now'))
            "#,
        )
        .bind(&network_id)
        .bind(&node_id)
        .bind(format!("network-{}", network_id))
        .execute(&mut *tx)
        .await
        .map_err(|e| BffError::Internal(format!("failed to insert network: {}", e)))?;

        sqlx::query(
            r#"
            INSERT INTO network_desired_state (network_id, desired_generation, desired_status, requested_by, requested_at, updated_at)
            VALUES (?, 1, 'Pending', ?, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
            "#,
        )
        .bind(&network_id)
        .bind(&requested_by)
        .execute(&mut *tx)
        .await
        .map_err(|e| BffError::Internal(format!("failed to insert network_desired_state: {}", e)))?;
    }

    // Insert NIC
    let nic_id = format!("{}-{}", vm_id, network_id);
    let mac_address = generate_mac(&vm_id, &network_id);
    let ip_address = generate_ip(&vm_id, &network_id);

    sqlx::query(
        r#"
        INSERT INTO vm_nic_desired_state (nic_id, vm_id, network_id, mac_address, ip_address, nic_model, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, 'virtio', strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&nic_id)
    .bind(&vm_id)
    .bind(&network_id)
    .bind(&mac_address)
    .bind(&ip_address)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert vm_nic_desired_state: {}", e)))?;

    // Insert operation
    let idempotency_key = format!("create-vm-{}", vm_id);
    sqlx::query(
        r#"
        INSERT INTO operations (operation_id, idempotency_key, resource_kind, resource_id, operation_type, status, requested_by, desired_generation, requested_at, created_at, updated_at)
        VALUES (?, ?, 'vm', ?, 'CreateVm', 'Accepted', ?, 1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&operation_id)
    .bind(&idempotency_key)
    .bind(&vm_id)
    .bind(&requested_by)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert operation: {}", e)))?;

    tracing::info!(%vm_id, "create_vm: committing transaction");
    tx.commit()
        .await
        .map_err(|e| BffError::Internal(format!("failed to commit transaction: {}", e)))?;

    tracing::info!(%vm_id, "create_vm: transaction committed successfully");
    Ok(Json(json!({
        "vm_id": vm_id,
        "operation_id": operation_id,
        "status": "Accepted",
    })))
}

pub async fn delete_vm(
    crate::auth::BearerToken(_claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let vm_id = payload
        .get("vm_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing vm_id".into()))?
        .to_string();

    let requested_by = payload
        .get("requested_by")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing requested_by".into()))?
        .to_string();

    // Check vm exists
    let exists = sqlx::query_scalar::<_, String>("SELECT vm_id FROM vms WHERE vm_id = ?")
        .bind(&vm_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to check vm existence: {}", e)))?;

    if exists.is_none() {
        return Err(BffError::NotFound(format!("vm {} not found", vm_id)));
    }

    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| BffError::Internal(format!("failed to begin transaction: {}", e)))?;

    sqlx::query(
        r#"
        UPDATE vm_desired_state
        SET desired_status = 'Deleting', desired_generation = desired_generation + 1, updated_by = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
        WHERE vm_id = ?
        "#,
    )
    .bind(&requested_by)
    .bind(&vm_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to update vm_desired_state: {}", e)))?;

    let new_generation: i64 =
        sqlx::query_scalar("SELECT desired_generation FROM vm_desired_state WHERE vm_id = ?")
            .bind(&vm_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| BffError::Internal(format!("failed to read generation: {}", e)))?;

    let operation_id = uuid::Uuid::new_v4().to_string();
    let idempotency_key = format!("delete-vm-{}", vm_id);
    sqlx::query(
        r#"
        INSERT INTO operations (operation_id, idempotency_key, resource_kind, resource_id, operation_type, status, requested_by, desired_generation, requested_at, created_at, updated_at)
        VALUES (?, ?, 'vm', ?, 'DeleteVm', 'Accepted', ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&operation_id)
    .bind(&idempotency_key)
    .bind(&vm_id)
    .bind(&requested_by)
    .bind(new_generation)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert operation: {}", e)))?;

    tx.commit()
        .await
        .map_err(|e| BffError::Internal(format!("failed to commit transaction: {}", e)))?;

    Ok(Json(json!({
        "vm_id": vm_id,
        "operation_id": operation_id,
        "status": "Accepted",
    })))
}

pub async fn mutate_vm(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    let vm_id = payload
        .get("vm_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing vm_id".into()))?
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
        .mutate_vm(vm_id, action, force, claims.username)
        .await?;

    Ok(Json(json!({
        "accepted": response.accepted,
        "task_id": response.task_id,
        "vm_id": response.vm_id,
        "summary": response.summary,
    })))
}

#[derive(sqlx::FromRow)]
struct VmRow {
    vm_id: String,
    name: String,
    node_id: Option<String>,
    power_state: String,
    health: String,
    cpu: String,
    memory: String,
    volume_count: i32,
    nic_count: i32,
    last_task: String,
}

#[derive(sqlx::FromRow)]
struct VmSummaryRow {
    vm_id: String,
    name: String,
    node_id: Option<String>,
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
    started_unix_ms: i64,
}

#[derive(sqlx::FromRow)]
struct VmVolumeRow {
    volume_id: String,
    name: String,
    size: String,
    device_name: String,
    read_only: bool,
    health: String,
}

#[derive(sqlx::FromRow)]
struct VmNicRow {
    nic_id: String,
    network_id: String,
    network_name: String,
    mac_address: String,
    ip_address: String,
    nic_model: String,
}

// ── Helpers ───────────────────────────────────────────────────────────────

/// Deterministically generate a MAC address from vm_id + network_id.
/// Sets the locally-administered bit to avoid conflicts with OUI space.
fn generate_mac(vm_id: &str, network_id: &str) -> String {
    let input = format!("{}:{}", vm_id, network_id);
    let mut hash: u64 = 0xcbf29ce484222325; // FNV-1a offset basis
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let octets = [
        0x02, // locally administered
        ((hash >> 8) & 0xFF) as u8,
        ((hash >> 16) & 0xFF) as u8,
        ((hash >> 24) & 0xFF) as u8,
        ((hash >> 32) & 0xFF) as u8,
        ((hash >> 40) & 0xFF) as u8,
    ];
    format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        octets[0], octets[1], octets[2], octets[3], octets[4], octets[5]
    )
}

/// Deterministically generate a 10.x.x.x IP from vm_id + network_id.
fn generate_ip(vm_id: &str, _network_id: &str) -> String {
    let input = vm_id;
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let o2 = ((hash >> 8) & 0xFF) as u8;
    let o3 = ((hash >> 16) & 0xFF) as u8;
    let o4 = ((hash >> 24) & 0xFF) as u8;
    format!("10.{}.{}.{}", o2, o3, o4)
}
