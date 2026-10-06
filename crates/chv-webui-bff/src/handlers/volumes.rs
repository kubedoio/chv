use axum::{extract::State, response::Json, Extension};
use serde_json::{json, Value};

use crate::router::AppState;
use crate::BffError;

pub async fn list_volumes(
    crate::auth::BearerToken(_claims): crate::auth::BearerToken,
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
    let cache_key = format!("volumes:list:{}:{}", page, page_size);
    if let Some(cached) = state.cache.get(&cache_key).await {
        return Ok(Json(
            serde_json::from_str(&cached).map_err(|e| BffError::Internal(e.to_string()))?,
        ));
    }

    let offset = (page - 1) * page_size;
    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM volumes")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| BffError::Internal(format!("failed to count volumes: {}", e)))?;
    let total_pages = (total_count as u64).div_ceil(page_size);

    let rows = sqlx::query_as::<_, VolumeRow>(
        r#"
        SELECT
            v.volume_id,
            v.display_name AS name,
            v.node_id,
            COALESCE(vos.health_status, 'unknown') AS health,
            COALESCE(vds.desired_status, vos.runtime_status, 'Unknown') AS status,
            CASE WHEN v.capacity_bytes IS NULL THEN ''
                 WHEN v.capacity_bytes >= 1073741824 THEN printf('%.1f GiB', CAST(v.capacity_bytes AS REAL)/1073741824.0)
                 WHEN v.capacity_bytes >= 1048576 THEN printf('%.1f MiB', CAST(v.capacity_bytes AS REAL)/1048576.0)
                 WHEN v.capacity_bytes >= 1024 THEN printf('%.1f KiB', CAST(v.capacity_bytes AS REAL)/1024.0)
                 ELSE printf('%d B', v.capacity_bytes) END AS size,
            COALESCE(vds.attached_vm_id, '') AS attached_vm_id,
            COALESCE(vms.display_name, '') AS attached_vm_name,
            COALESCE(
                (SELECT operation_type FROM operations
                 WHERE resource_kind = 'volume' AND resource_id = v.volume_id
                 ORDER BY requested_at DESC LIMIT 1),
                ''
            ) AS last_task
        FROM volumes v
        LEFT JOIN volume_desired_state vds ON v.volume_id = vds.volume_id
        LEFT JOIN vms ON vds.attached_vm_id = vms.vm_id
        LEFT JOIN volume_observed_state vos ON v.volume_id = vos.volume_id
        ORDER BY v.volume_id
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(page_size as i64)
    .bind(offset as i64)
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

    let response = Json(json!({
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
    }));
    if let Ok(json) = serde_json::to_string(&response.0) {
        state.cache.set(&cache_key, json).await;
    }
    Ok(response)
}

pub async fn get_volume(
    crate::auth::BearerToken(_claims): crate::auth::BearerToken,
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
            CASE WHEN v.capacity_bytes IS NULL THEN ''
                 WHEN v.capacity_bytes >= 1073741824 THEN printf('%.1f GiB', CAST(v.capacity_bytes AS REAL)/1073741824.0)
                 WHEN v.capacity_bytes >= 1048576 THEN printf('%.1f MiB', CAST(v.capacity_bytes AS REAL)/1048576.0)
                 WHEN v.capacity_bytes >= 1024 THEN printf('%.1f KiB', CAST(v.capacity_bytes AS REAL)/1024.0)
                 ELSE printf('%d B', v.capacity_bytes) END AS size,
            v.capacity_bytes,
            COALESCE(vds.desired_status, vos.runtime_status, 'Unknown') AS status,
            COALESCE(vds.attached_vm_id, '') AS attached_vm_id,
            COALESCE(vms.display_name, '') AS attached_vm_name,
            COALESCE(vds.device_name, '') AS device_name,
            COALESCE(vds.read_only, false) AS read_only,
            COALESCE(v.volume_kind, '') AS volume_kind,
            COALESCE(v.storage_class, '') AS storage_class,
            COALESCE(
                (SELECT operation_type FROM operations
                 WHERE resource_kind = 'volume' AND resource_id = v.volume_id
                 ORDER BY requested_at DESC LIMIT 1),
                ''
            ) AS last_task
        FROM volumes v
        LEFT JOIN volume_desired_state vds ON v.volume_id = vds.volume_id
        LEFT JOIN vms ON vds.attached_vm_id = vms.vm_id
        LEFT JOIN volume_observed_state vos ON v.volume_id = vos.volume_id
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
                    status,
                    operation_type AS summary,
                    operation_type AS operation,
                    CAST(strftime('%s', requested_at) AS INTEGER) * 1000 AS started_unix_ms,
                    error_code,
                    error_message
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
                        // #502: NULL until a terminal failure records a
                        // cause — passed through verbatim, never
                        // fabricated.
                        "error_code": t.error_code,
                        "error_message": t.error_message,
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
                    "capacity_bytes": r.capacity_bytes,
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
        None => Err(BffError::NotFound(format!(
            "volume {} not found",
            volume_id
        ))),
    }
}

/// The DP3 capacity ceiling: 64 TiB — now the single shared constant
/// (`chv_hypervisor_api::resources::MAX_VOLUME_BYTES`, beside
/// `BACKEND_CLASSES` in the shared-vocabulary home; the #513 PR 3
/// consolidation — previously a BFF-local literal here). The
/// GiB-denominated create surfaces (`handlers/vms.rs`,
/// `handlers/templates.rs`) derive their `MAX_VOLUME_SIZE_GB` from the
/// same definition.
const MAX_VOLUME_BYTES: i64 = chv_hypervisor_api::resources::MAX_VOLUME_BYTES;

/// #513 PR 2 (the adopted design's DP1/DP3–DP8): the standalone
/// volume-create route — the first production producer of the PR 1
/// `CreateVolume` dispatch carrier (#523). Journaling is BFF-direct,
/// mirroring `POST /v1/vms` (DP1): `volumes` + `volume_desired_state`
/// (`desired_status 'Pending'`, `attached_vm_id` NULL — the volume is
/// born standalone) + `operations` (`'CreateVolume'`/`'Accepted'`, the
/// arm PR 1 landed) in one `BEGIN IMMEDIATE` transaction, after every
/// accept-time check has passed. Every check runs BEFORE the
/// transaction, so a rejection journals zero rows — the #378/#516
/// discipline.
///
/// Accept-time checks, in order: operator-or-admin tier; the DP4/DP3
/// reserved-key rejections (`attached_vm_id` names the mutate-attach
/// path, `seed_image_ref` is not supported); display-name validation;
/// required `node_id` (DP3 — no first-enrolled-node default: silent
/// placement of storage is worse than silent placement of a VM);
/// positive `capacity_bytes` within the 64 TiB ceiling (DP3); the
/// `storage_class` vocabulary against the one shared list (DP5); the
/// node-capability check via the shared
/// `NodeRepository::node_storage_class_rejection` composition (DP5 —
/// fails OPEN on unreported nodes, byte-exactly the lifecycle
/// semantics); the core-managed rejection (DP7, the #378 mirror — the
/// provisioning RPC is a legacy-path stord side effect, and on a
/// core-managed node the single writer is Core); then the quota
/// enforcement with the storage column (DP6) inside the transaction,
/// where it runs before any INSERT so a rejection still journals
/// nothing.
///
/// Ownership (DP6, the #386 lesson): the `volumes` row is stamped
/// `owner_id = claims.sub` — an unstamped volume is admin-only via
/// `require_volume_owner`, locking its own creator out of mutating it.
///
/// Wire translation happens at the existing seam: this route journals
/// `storage_class` verbatim; the orchestrator/node_client resolves it
/// to `backend_class` on dispatch (PR 1's carrier). NULL class is
/// journaled as NULL and dispatches as local — never materialized.
pub async fn create_volume(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    Extension(correlation_id): Extension<Option<String>>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;

    // DP4: `attached_vm_id` is reserved, NOT accepted, in v1 — a
    // payload carrying it gets a loud 400 naming the mutate-attach
    // path (the #372 `--vlan` lesson: a reserved key is never
    // silently dropped). A JSON `null` is semantically absent, same
    // as the blank-storage_class convention.
    if let Some(value) = payload.get("attached_vm_id") {
        if !value.is_null() {
            return Err(BffError::BadRequest(
                "attached_vm_id is not accepted at create time (v1 creates standalone volumes \
                 only): create the volume first, then attach it via POST /v1/volumes/mutate \
                 with action 'attach' and vm_id"
                    .into(),
            ));
        }
    }

    // DP3: `seed_image_ref` is deferred and rejected at accept if ever
    // sent — there is no seed path for standalone volumes (the LVM
    // open path refuses `seed_from` by design).
    if let Some(value) = payload.get("seed_image_ref") {
        if !value.is_null() {
            return Err(BffError::BadRequest(
                "seed_image_ref is not supported for volume create".into(),
            ));
        }
    }

    // `name` + `display_name` alias, validated by the shared VM-create
    // rule (the name is interpolated into volume names/paths — the
    // same traversal guard).
    let display_name = payload
        .get("display_name")
        .and_then(|v| v.as_str())
        .or_else(|| payload.get("name").and_then(|v| v.as_str()))
        .ok_or_else(|| BffError::BadRequest("missing name/display_name".into()))?
        .to_string();
    if !super::vms::is_valid_display_name(&display_name) {
        return Err(BffError::BadRequest(
            "display_name must match ^[A-Za-z0-9 ._-]{1,64}$".into(),
        ));
    }

    // DP3: `node_id` is REQUIRED — standalone volumes have no VM to
    // place them, and no first-enrolled-node default (silent placement
    // of storage is worse than silent placement of a VM).
    let node_id = payload
        .get("node_id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            BffError::BadRequest("missing node_id (standalone volume create requires an explicit placement node; there is no default)".into())
        })?
        .to_string();

    // DP3: positive `capacity_bytes`, bounded by the 64 TiB ceiling
    // (the `MAX_VOLUME_SIZE_GB` discipline). The agent's create
    // handler opens WITH the size (create-on-open), so a non-positive
    // capacity can never provision — and the orchestrator arm refuses
    // it at dispatch; this is the accept-time half.
    let capacity_bytes = payload
        .get("capacity_bytes")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| BffError::BadRequest("missing capacity_bytes".into()))?;
    if capacity_bytes <= 0 || capacity_bytes > MAX_VOLUME_BYTES {
        return Err(BffError::BadRequest(format!(
            "capacity_bytes must be between 1 and {} (64 TiB)",
            MAX_VOLUME_BYTES
        )));
    }

    // DP5: the optional `storage_class`, validated against the one
    // shared list (`chv_hypervisor_api::resources`), never a local
    // copy — the exact `POST /v1/vms` arm. Absent (or blank) means
    // NULL in `volumes.storage_class` = local; the string "local" is
    // only stored when the caller names it explicitly; aliases reject
    // here. Journaled verbatim — the → backend_class translation is
    // the orchestrator/node_client seam, not the BFF's.
    let storage_class = match payload.get("storage_class") {
        None => None,
        Some(value) => {
            let class = value
                .as_str()
                .ok_or_else(|| BffError::BadRequest("storage_class must be a string".into()))?;
            let class = class.trim();
            if class.is_empty() {
                None
            } else if chv_hypervisor_api::resources::is_known_backend_class(class) {
                Some(class.to_string())
            } else {
                return Err(BffError::BadRequest(format!(
                    "unknown storage_class '{}': must be one of local, iscsi, ceph, lvm",
                    class
                )));
            }
        }
    };

    // Both node checks share one NodeId parse. A node_id that does not
    // parse skips the checks (fail-open) and keeps today's behavior —
    // the checks add rejection modes, never new parse errors (the
    // create_vm discipline).
    if let Ok(node) = chv_controlplane_types::domain::NodeId::new(node_id.clone()) {
        // DP5 (#516): the node-capability check — the single shared
        // composition the lifecycle RPC uses, fired BEFORE the
        // transaction so a rejection journals nothing. Fails OPEN on a
        // node that never reported classes (stord's own backend
        // validation at the open is the backstop); a classless request
        // (NULL = local) rejects on an LVM-only node, same as vm
        // create.
        if let Some(reason) = state
            .node_repo
            .node_storage_class_rejection(&node, storage_class.as_deref(), "volume create")
            .await?
        {
            tracing::warn!(%node_id, class = ?storage_class, "create_volume: rejecting storage class the node does not offer");
            return Err(BffError::BadRequest(reason));
        }

        // DP7 (#378 mirror): reject at accept on core-managed nodes —
        // the same authority-mode resolution the CP's volume-op
        // rejection uses (`get_authority_mode`, compared against
        // `AUTHORITY_MODE_CORE_MANAGED`, fail-open on every other
        // value). The provisioning RPC is a legacy-path stord side
        // effect; on a core-managed node the single writer is Core,
        // which cannot express a standalone volume create.
        let mode = state.node_repo.get_authority_mode(&node).await?;
        if mode.as_deref() == Some(chv_controlplane_store::AUTHORITY_MODE_CORE_MANAGED) {
            tracing::warn!(%node_id, "create_volume: rejecting create on core-managed node");
            return Err(BffError::BadRequest(
                "volume create is not supported on core-managed nodes".into(),
            ));
        }
    }

    // BEGIN IMMEDIATE (the create_vm discipline): acquire SQLite's
    // RESERVED lock at tx start, serializing concurrent writers and
    // closing the quota-check TOCTOU window.
    let mut tx = state
        .pool
        .begin_with("BEGIN IMMEDIATE;")
        .await
        .map_err(|e| BffError::Internal(format!("failed to begin transaction: {}", e)))?;

    // DP6: quota enforcement with the storage column — the first
    // direct storage-quota consumer outside VM creates
    // (`vm_count_delta = 0`: no VM is created). Inside the
    // transaction, before any INSERT, so a rejection journals nothing.
    super::vms::enforce_user_quota(&mut tx, &claims.sub, 0, 0, capacity_bytes, 0).await?;

    // DP3: server-minted id (no client-supplied volume_id — the clone
    // path's caller-supplied id exists for replay semantics this
    // surface doesn't need).
    let volume_id = chv_common::gen_short_id();
    let operation_id = correlation_id.unwrap_or_else(chv_common::gen_short_id);
    let requested_by = claims.sub.clone();
    tracing::info!(%volume_id, %operation_id, %node_id, "create_volume: generated IDs, continuing transaction");

    // DP6 (#386): stamp owner_id — an unstamped volume is admin-only
    // via require_volume_owner, locking its own creator out. DP8:
    // volume_kind 'data' — the one query that distinguishes data
    // volumes from boot disks on the retention boundary (there is no
    // delete). storage_class NULL when the caller did not name a
    // class (the historical row shape).
    sqlx::query(
        r#"
        INSERT INTO volumes (volume_id, node_id, display_name, owner_id, capacity_bytes, volume_kind, storage_class, updated_at)
        VALUES (?, ?, ?, ?, ?, 'data', ?, strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&volume_id)
    .bind(&node_id)
    .bind(&display_name)
    .bind(&claims.sub)
    .bind(capacity_bytes)
    .bind(&storage_class)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert volume: {}", e)))?;

    // DP4: the volume is born standalone — `desired_status 'Pending'`,
    // `attached_vm_id` NULL, no device name. The orchestrator's
    // `CreateVolume` arm claims this row's volume by resource_id and
    // dispatches the provisioning open at generation 1.
    sqlx::query(
        r#"
        INSERT INTO volume_desired_state (volume_id, desired_generation, desired_status, requested_by, attached_vm_id, device_name, read_only, requested_at, updated_at)
        VALUES (?, 1, 'Pending', ?, NULL, NULL, 0, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&volume_id)
    .bind(&requested_by)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert volume_desired_state: {}", e)))?;

    // The Accepted operation the PR 1 arm dispatches. resource_kind
    // 'volume' (lowercase) — the BFF's volume display surfaces read
    // that exact spelling, and the orchestrator's claim query resolves
    // the node by resource_id regardless of kind.
    let idempotency_key = format!("create-volume-{}", volume_id);
    sqlx::query(
        r#"
        INSERT INTO operations (operation_id, idempotency_key, resource_kind, resource_id, operation_type, status, requested_by, desired_generation, requested_at, created_at, updated_at)
        VALUES (?, ?, 'volume', ?, 'CreateVolume', 'Accepted', ?, 1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&operation_id)
    .bind(&idempotency_key)
    .bind(&volume_id)
    .bind(&requested_by)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert operation: {}", e)))?;

    tx.commit()
        .await
        .map_err(|e| BffError::Internal(format!("failed to commit transaction: {}", e)))?;

    tracing::info!(%volume_id, "create_volume: transaction committed successfully");
    state.cache.invalidate("volumes:").await;
    state.cache.invalidate("overview").await;
    Ok(Json(json!({
        "accepted": true,
        "task_id": operation_id,
        "volume_id": volume_id,
        "summary": format!("Creating volume '{}'", display_name),
        "next_refresh_path": format!("/api/v1/tasks/{}", operation_id),
    })))
}

pub async fn mutate_volume(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;
    let volume_id = payload
        .get("volume_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing volume_id".into()))?
        .to_string();

    let mut conn = state
        .pool
        .acquire()
        .await
        .map_err(|e| BffError::Internal(format!("failed to acquire connection: {}", e)))?;
    require_volume_owner(&mut conn, &volume_id, &claims.sub, claims.role == "admin").await?;

    let action = payload
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing action".into()))?
        .to_string();

    let force = payload
        .get("force")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let resize_bytes = payload.get("resize_bytes").and_then(|v| v.as_u64());
    let vm_id = payload
        .get("vm_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let response = state
        .mutations
        .mutate_volume(volume_id, action, force, resize_bytes, vm_id, claims.sub)
        .await?;

    state.cache.invalidate("volumes:").await;
    state.cache.invalidate("overview").await;
    Ok(Json(json!({
        "accepted": response.accepted,
        "task_id": response.task_id,
        "volume_id": response.volume_id,
        "summary": response.summary,
    })))
}

pub async fn snapshot_volume(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;
    let volume_id = payload
        .get("volume_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing volume_id".into()))?
        .to_string();

    let mut conn = state
        .pool
        .acquire()
        .await
        .map_err(|e| BffError::Internal(format!("failed to acquire connection: {}", e)))?;
    require_volume_owner(&mut conn, &volume_id, &claims.sub, claims.role == "admin").await?;
    let snapshot_name = payload
        .get("snapshot_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing snapshot_name".into()))?
        .to_string();

    let response = state
        .mutations
        .snapshot_volume(volume_id, snapshot_name, claims.sub)
        .await?;

    state.cache.invalidate("volumes:").await;
    state.cache.invalidate("overview").await;
    Ok(Json(json!({
        "accepted": response.accepted,
        "task_id": response.task_id,
        "volume_id": response.volume_id,
        "summary": response.summary,
    })))
}

pub async fn restore_volume_snapshot(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;
    let volume_id = payload
        .get("volume_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing volume_id".into()))?
        .to_string();

    let mut conn = state
        .pool
        .acquire()
        .await
        .map_err(|e| BffError::Internal(format!("failed to acquire connection: {}", e)))?;
    require_volume_owner(&mut conn, &volume_id, &claims.sub, claims.role == "admin").await?;
    let snapshot_name = payload
        .get("snapshot_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing snapshot_name".into()))?
        .to_string();

    let response = state
        .mutations
        .restore_volume_snapshot(volume_id, snapshot_name, claims.sub)
        .await?;

    state.cache.invalidate("volumes:").await;
    state.cache.invalidate("overview").await;
    Ok(Json(json!({
        "accepted": response.accepted,
        "task_id": response.task_id,
        "volume_id": response.volume_id,
        "summary": response.summary,
    })))
}

pub async fn delete_volume_snapshot(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;
    let volume_id = payload
        .get("volume_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing volume_id".into()))?
        .to_string();

    let mut conn = state
        .pool
        .acquire()
        .await
        .map_err(|e| BffError::Internal(format!("failed to acquire connection: {}", e)))?;
    require_volume_owner(&mut conn, &volume_id, &claims.sub, claims.role == "admin").await?;
    let snapshot_name = payload
        .get("snapshot_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing snapshot_name".into()))?
        .to_string();

    let response = state
        .mutations
        .delete_volume_snapshot(volume_id, snapshot_name, claims.sub)
        .await?;

    state.cache.invalidate("volumes:").await;
    state.cache.invalidate("overview").await;
    Ok(Json(json!({
        "accepted": response.accepted,
        "task_id": response.task_id,
        "volume_id": response.volume_id,
        "summary": response.summary,
    })))
}

pub async fn clone_volume(
    crate::auth::BearerToken(claims): crate::auth::BearerToken,
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<Value>,
) -> Result<Json<Value>, BffError> {
    crate::auth::require_operator_or_admin(&claims)?;
    let source_volume_id = payload
        .get("source_volume_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing source_volume_id".into()))?
        .to_string();

    let mut conn = state
        .pool
        .acquire()
        .await
        .map_err(|e| BffError::Internal(format!("failed to acquire connection: {}", e)))?;
    require_volume_owner(
        &mut conn,
        &source_volume_id,
        &claims.sub,
        claims.role == "admin",
    )
    .await?;
    let target_volume_id = payload
        .get("target_volume_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BffError::BadRequest("missing target_volume_id".into()))?
        .to_string();

    let response = state
        .mutations
        .clone_volume(source_volume_id, target_volume_id, claims.sub)
        .await?;

    state.cache.invalidate("volumes:").await;
    state.cache.invalidate("overview").await;
    Ok(Json(json!({
        "accepted": response.accepted,
        "task_id": response.task_id,
        "volume_id": response.volume_id,
        "summary": response.summary,
    })))
}

/// Check if the user is the owner of a volume or an admin.
/// Returns Ok(()) if allowed, Err(BffError::Forbidden) if not.
pub(crate) async fn require_volume_owner(
    conn: &mut sqlx::SqliteConnection,
    volume_id: &str,
    user_id: &str,
    is_admin: bool,
) -> Result<(), BffError> {
    if is_admin {
        return Ok(());
    }
    let owner: Option<String> =
        sqlx::query_scalar("SELECT owner_id FROM volumes WHERE volume_id = ?")
            .bind(volume_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| BffError::Internal(format!("failed to check volume owner: {}", e)))?;
    match owner {
        Some(o) if o == user_id => Ok(()),
        None => {
            tracing::warn!(resource_id = %volume_id, "ownership check failed: resource has no owner_id set");
            Err(BffError::Forbidden(
                "resource has no owner; admin access required".into(),
            ))
        }
        Some(_) => Err(BffError::Forbidden("you do not own this volume".into())),
    }
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
    capacity_bytes: Option<i64>,
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
    // #502: the terminal-failure cause the fast-fail work journals.
    error_code: Option<String>,
    error_message: Option<String>,
}
