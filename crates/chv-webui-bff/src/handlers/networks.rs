use axum::{extract::State, response::Json};
use serde_json::{json, Value};

use crate::auth::BearerToken;
use crate::router::AppState;
use crate::BffError;

pub async fn list_networks(
    BearerToken(_claims): BearerToken,
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
    let cache_key = format!("networks:list:{}:{}", page, page_size);
    if let Some(cached) = state.cache.get(&cache_key).await {
        return Ok(Json(
            serde_json::from_str(&cached).map_err(|e| BffError::Internal(e.to_string()))?,
        ));
    }

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
            -- attached_vms mirrors the delete gate's liveness predicate
            -- (#356): only VMs that are not being deleted count.
            (SELECT COUNT(*) FROM vm_nic_desired_state nv
             JOIN vms v ON v.vm_id = nv.vm_id
             LEFT JOIN vm_desired_state vds ON vds.vm_id = v.vm_id
             WHERE nv.network_id = n.network_id
               AND (vds.desired_status IS NULL OR vds.desired_status != 'Deleting')) AS attached_vms,
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

pub async fn get_network(
    BearerToken(_claims): BearerToken,
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
                   LEFT JOIN vm_desired_state vds ON vds.vm_id = v.vm_id
                   WHERE nv.network_id = ?
                     AND (vds.desired_status IS NULL OR vds.desired_status != 'Deleting')"#,
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

    // N6 (M4.4 re-qualification): an absent gateway must DEFAULT to the
    // cidr's first usable host, exactly like the vm-create implicit-network
    // fallback always did (its `10.200.0.1` default). Without this, the
    // network's desired state carries a NULL gateway, the VM spec dispatches
    // `gateway: ""`, and nwd's ensure skips BOTH the bridge's L3 address and
    // dnsmasq — VMs attach to an L2-only bridge with no host↔guest
    // connectivity and no DHCP. Pre-#354 this was masked because
    // `vm create --network <name>` missed the name lookup and fell back to
    // the implicit network (which carried the default); #354's correct name
    // resolution exposed the operator-created network's missing gateway.
    // An EXPLICIT empty string stays empty (an operator's deliberate
    // L2-only choice); only an ABSENT field defaults.
    let gateway = match payload.get("gateway").and_then(|v| v.as_str()) {
        Some(gateway) => gateway.to_string(),
        None => default_gateway_for_cidr(&cidr),
    };

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

    let firewall_rules_json = firewall_rules_payload(&payload)?;

    let nat_rules_json = payload.get("nat_rules").map(|v| v.to_string());

    let dhcp_scope_json = payload.get("dhcp_scope").map(|v| v.to_string());

    let dns_enabled = payload
        .get("dns_enabled")
        .and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|i| i != 0)))
        .unwrap_or(false);

    let dns_scope_json = payload.get("dns_scope").map(|v| v.to_string());

    let network_id = chv_common::gen_short_id();

    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| BffError::Internal(format!("failed to begin transaction: {}", e)))?;

    sqlx::query(
        r#"
        INSERT INTO networks (network_id, display_name, network_class, owner_id, created_at, updated_at)
        VALUES (?, ?, 'bridge', ?, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&network_id)
    .bind(&name)
    .bind(&claims.sub)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert network: {}", e)))?;

    sqlx::query(
        r#"
        INSERT INTO network_desired_state (
            network_id, desired_generation, desired_status, cidr, gateway,
            dhcp_enabled, ipam_mode, is_default, firewall_rules_json, nat_rules_json,
            dhcp_scope_json, dns_enabled, dns_scope_json, requested_at, updated_at
        )
        VALUES (?, 1, 'Pending', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
    )
    .bind(&network_id)
    .bind(&cidr)
    .bind(&gateway)
    .bind(if dhcp_enabled { 1 } else { 0 })
    .bind(&ipam_mode)
    .bind(if is_default { 1 } else { 0 })
    .bind(&firewall_rules_json)
    .bind(&nat_rules_json)
    .bind(&dhcp_scope_json)
    .bind(if dns_enabled { 1 } else { 0 })
    .bind(&dns_scope_json)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to insert network_desired_state: {}", e)))?;

    tx.commit()
        .await
        .map_err(|e| BffError::Internal(format!("failed to commit transaction: {}", e)))?;

    state.cache.invalidate("networks:").await;
    state.cache.invalidate("overview").await;
    Ok(Json(json!({
        "accepted": true,
        "task_id": null,
        "network_id": network_id,
        "summary": format!("Creating network '{}'", name),
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

    let mut conn = state
        .pool
        .acquire()
        .await
        .map_err(|e| BffError::Internal(format!("failed to acquire connection: {}", e)))?;
    require_network_owner(&mut conn, &network_id, &claims.sub, claims.role == "admin").await?;
    drop(conn);

    // Attached-VM gate (#356): count only LIVE VMs — nic rows linger
    // after `vm delete` in pre-#356 data (the vms/vm_desired_state rows
    // persist as 'Deleting' tombstones), and a raw nic-row count made
    // network delete permanently refuse with "N VM(s) still attached"
    // (verified on real KVM by the M4.4 qualification, issue #356). A
    // VM whose deletion was accepted (desired_status 'Deleting') no
    // longer blocks the network it is leaving.
    // BEGIN IMMEDIATE: serialize concurrent writers — the count→GC→delete
    // sequence must not race a concurrent delete_vm (DEFERRED
    // read-then-write can surface SQLITE_BUSY_SNAPSHOT as a 500 instead
    // of the clean 409/200 contract).
    let mut tx = state
        .pool
        .begin_with("BEGIN IMMEDIATE;")
        .await
        .map_err(|e| BffError::Internal(format!("failed to begin transaction: {}", e)))?;

    let attached_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM vm_nic_desired_state nv
        JOIN vms v ON v.vm_id = nv.vm_id
        LEFT JOIN vm_desired_state vds ON vds.vm_id = v.vm_id
        WHERE nv.network_id = ?
          AND (vds.desired_status IS NULL OR vds.desired_status != 'Deleting')
        "#,
    )
    .bind(&network_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to count attached vms: {}", e)))?;

    if attached_count > 0 {
        return Err(BffError::Conflict(format!(
            "cannot delete network {}: {} VM(s) still attached",
            network_id, attached_count
        )));
    }

    let exists =
        sqlx::query_scalar::<_, String>("SELECT network_id FROM networks WHERE network_id = ?")
            .bind(&network_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| BffError::Internal(format!("failed to check network existence: {}", e)))?;

    if exists.is_none() {
        return Err(BffError::NotFound(format!(
            "network {} not found",
            network_id
        )));
    }

    // Stale-row GC (#356): drop this network's nic rows whose VM is
    // gone or deleting — required for the NIC table, not cosmetic:
    // vm_nic_desired_state has network_id REFERENCES networks ON DELETE
    // RESTRICT, so leaving them would turn the delete below into an FK
    // violation (500) instead of the clean 409/200 contract.
    sqlx::query(
        r#"
        DELETE FROM vm_nic_desired_state
        WHERE network_id = ?
          AND vm_id NOT IN (
              SELECT v.vm_id FROM vms v
              JOIN vm_desired_state vds ON vds.vm_id = v.vm_id
              WHERE vds.desired_status != 'Deleting'
          )
        "#,
    )
    .bind(&network_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to gc stale nic rows: {}", e)))?;

    // VNI allocations (#356, the last FK hazard): vni_allocations
    // references networks(network_id) with no ON DELETE action, so ANY
    // row — soft-released or not — FK-failed the delete below; overlay
    // networks that ever allocated a VNI could never be deleted. Two
    // statements, in order:
    //   1. soft-release active rows (same bookkeeping as
    //      Store::release_vni: allocation history keeps its release
    //      timestamp);
    //   2. delete the rows — the FK leaves no other choice under this
    //      schema (network_id is NOT NULL, so ON DELETE SET NULL is not
    //      expressible either).
    // Tradeoff, accepted and documented: the 24h VNI-reuse quarantine
    // (the allocation query's released_at window) cannot survive the
    // network's deletion under this schema — the rows ARE the
    // quarantine. Network delete is an explicit operator action gated
    // on zero live attachments and preceded by the last-detach host
    // teardown (#362), so immediate reuse is the bounded residual; a
    // schema change (nullable reference or a side quarantine table)
    // would be needed to do better.
    sqlx::query(
        r#"
        UPDATE vni_allocations
        SET released_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
        WHERE network_id = ? AND released_at IS NULL
        "#,
    )
    .bind(&network_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| BffError::Internal(format!("failed to release vni allocations: {}", e)))?;

    sqlx::query("DELETE FROM vni_allocations WHERE network_id = ?")
        .bind(&network_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| BffError::Internal(format!("failed to delete vni allocations: {}", e)))?;

    sqlx::query("DELETE FROM networks WHERE network_id = ?")
        .bind(&network_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| BffError::Internal(format!("failed to delete network: {}", e)))?;

    tx.commit()
        .await
        .map_err(|e| BffError::Internal(format!("failed to commit transaction: {}", e)))?;

    state.cache.invalidate("networks:").await;
    state.cache.invalidate("overview").await;
    Ok(Json(json!({
        "accepted": true,
        "task_id": null,
        "network_id": network_id,
        "summary": format!("Deleted network '{}'", network_id),
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

    let mut conn = state
        .pool
        .acquire()
        .await
        .map_err(|e| BffError::Internal(format!("failed to acquire connection: {}", e)))?;
    require_network_owner(&mut conn, &network_id, &claims.sub, claims.role == "admin").await?;

    let exists =
        sqlx::query_scalar::<_, String>("SELECT network_id FROM networks WHERE network_id = ?")
            .bind(&network_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| BffError::Internal(format!("failed to check network existence: {}", e)))?;
    // Release the pooled connection before opening the write transaction —
    // single-connection pools (tests) would otherwise deadlock the begin().
    drop(conn);

    if exists.is_none() {
        return Err(BffError::NotFound(format!(
            "network {} not found",
            network_id
        )));
    }

    let name = payload.get("name").and_then(|v| v.as_str());
    let cidr = payload.get("cidr").and_then(|v| v.as_str());
    let gateway = payload.get("gateway").and_then(|v| v.as_str());
    let dhcp_enabled = payload
        .get("dhcp_enabled")
        .and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|i| i != 0)))
        .map(|d| if d { 1 } else { 0 });
    let ipam_mode = payload.get("ipam_mode").and_then(|v| v.as_str());
    let is_default = payload
        .get("is_default")
        .and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|i| i != 0)))
        .map(|d| if d { 1 } else { 0 });
    let firewall_rules_json = firewall_rules_payload(&payload)?;
    let nat_rules_json = payload.get("nat_rules").map(|v| v.to_string());
    let dhcp_scope_json = payload.get("dhcp_scope").map(|v| v.to_string());
    let dns_enabled = payload
        .get("dns_enabled")
        .and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|i| i != 0)))
        .map(|d| if d { 1 } else { 0 });
    let dns_scope_json = payload.get("dns_scope").map(|v| v.to_string());

    let has_network_update = cidr.is_some()
        || gateway.is_some()
        || dhcp_enabled.is_some()
        || ipam_mode.is_some()
        || is_default.is_some()
        || firewall_rules_json.is_some()
        || nat_rules_json.is_some()
        || dhcp_scope_json.is_some()
        || dns_enabled.is_some()
        || dns_scope_json.is_some();

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

    if has_network_update {
        sqlx::query(
            r#"
            UPDATE network_desired_state
            SET
                cidr = COALESCE(?, cidr),
                gateway = COALESCE(?, gateway),
                dhcp_enabled = COALESCE(?, dhcp_enabled),
                ipam_mode = COALESCE(?, ipam_mode),
                is_default = COALESCE(?, is_default),
                firewall_rules_json = COALESCE(?, firewall_rules_json),
                nat_rules_json = COALESCE(?, nat_rules_json),
                dhcp_scope_json = COALESCE(?, dhcp_scope_json),
                dns_enabled = COALESCE(?, dns_enabled),
                dns_scope_json = COALESCE(?, dns_scope_json),
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
        .bind(&firewall_rules_json)
        .bind(&nat_rules_json)
        .bind(&dhcp_scope_json)
        .bind(dns_enabled)
        .bind(&dns_scope_json)
        .bind(&network_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            BffError::Internal(format!("failed to update network desired state: {}", e))
        })?;
    }

    tx.commit()
        .await
        .map_err(|e| BffError::Internal(format!("failed to commit transaction: {}", e)))?;

    state.cache.invalidate("networks:").await;
    state.cache.invalidate("overview").await;
    let Json(mut detail) = get_network(
        BearerToken(claims),
        State(state),
        axum::Json(json!({ "network_id": network_id })),
    )
    .await?;
    // Honest reporting (#355): firewall policy travels with the VM spec
    // and the Core executor applies it at ATTACH time (default-deny plus
    // these rules). The DB update alone does not reach an
    // already-materialized network — say so instead of implying the
    // rules are live on the node. Clearing the rules (`[]`) is reported
    // differently and truthfully: empty rulesets are never dispatched
    // (nwd would engage default-deny with no allow rules and cut the
    // network's guests off entirely), so a policy previously applied on
    // a live network STAYS in force until that network's host topology
    // is torn down — nwd re-asserts the last recorded non-empty policy
    // on every new attach.
    if let Some(rules) = firewall_rules_json.as_deref() {
        let note = if chv_common::firewall_ruleset_is_empty(rules) {
            "cleared: empty rulesets are never dispatched; a policy previously \
             applied on a live network (if any) stays in force until that \
             network's host topology is torn down (tracked in #355)"
        } else {
            "pending: applied when a VM spec is next dispatched to a node \
             (VM create or spec update); already-attached VMs are not \
             re-policyed until then"
        };
        match detail.as_object_mut() {
            Some(obj) => {
                obj.insert("policy_application".to_string(), json!(note));
            }
            None => {
                tracing::warn!(
                    network_id = %network_id,
                    "network detail response was not a JSON object; policy_application note dropped"
                );
            }
        }
    }
    Ok(Json(detail))
}

/// Extract a `firewall_rules` payload that must be a JSON array of rules
/// in the ENGINE vocabulary (#355, then #368's N7 trigger).
///
/// The snapshot is dispatched VERBATIM to nwd at VM attach time, where a
/// malformed ruleset fails rule parsing (or the attach-time policy-guard
/// refresh) and bricks every subsequent VM create on the network with an
/// opaque RUNTIME_UNAVAILABLE — observed live in the M4.4 re-qualification:
/// a rule authored in the UI-era dialect (`direction: "ingress"`,
/// `action: "allow"`) rode the spec, `set_firewall_policy` stored it, and
/// `attach_vm_nic`'s policy-guard refresh rejected it. Reject at the API
/// boundary where the operator sees it immediately; clients that mean
/// "clear the rules" send `[]`.
///
/// The vocabulary (directions, protocols, actions, rule keys, CIDR/port
/// field formats) is defined ONCE in [`chv_common::firewall`] — the same
/// definition nwd's engine enforces.
fn firewall_rules_payload(payload: &serde_json::Value) -> Result<Option<String>, BffError> {
    match payload.get("firewall_rules") {
        None => Ok(None),
        Some(v) if v.is_array() => {
            validate_firewall_ruleset(v.as_array().expect("checked is_array"))?;
            Ok(Some(v.to_string()))
        }
        Some(_) => Err(BffError::BadRequest(
            "firewall_rules must be an array of rules".into(),
        )),
    }
}

/// Validate every rule of a `firewall_rules` array against the engine
/// vocabulary. Error messages name the offending rule index and teach the
/// common UI-era alias mistakes — the operator coming from the old dialect
/// needs the mapping, not a bare enum.
fn validate_firewall_ruleset(rules: &[serde_json::Value]) -> Result<(), BffError> {
    use chv_common::firewall as vocab;
    for (index, rule) in rules.iter().enumerate() {
        let obj = rule.as_object().ok_or_else(|| {
            BffError::BadRequest(format!("firewall_rules[{index}] must be an object"))
        })?;
        for key in obj.keys() {
            if !vocab::RULE_KEYS.contains(&key.as_str()) {
                return Err(BffError::BadRequest(format!(
                    "firewall_rules[{index}]: unknown field '{key}' — allowed fields are \
                     [{}]. Common mistakes: 'source' → 'source_cidr', 'port_range' → \
                     'dest_port', 'priority'/'description' are not part of the engine ruleset",
                    vocab::RULE_KEYS.join(", ")
                )));
            }
        }
        let field = |name: &str| obj.get(name).and_then(|v| v.as_str());
        for (name, allowed, value) in [
            ("direction", vocab::DIRECTIONS, field("direction")),
            ("protocol", vocab::PROTOCOLS, field("protocol")),
            ("action", vocab::ACTIONS, field("action")),
        ] {
            match value {
                None => {
                    return Err(BffError::BadRequest(format!(
                        "firewall_rules[{index}]: missing required field '{name}'"
                    )))
                }
                Some(value) if !allowed.contains(&value) => {
                    return Err(BffError::BadRequest(format!(
                        "firewall_rules[{index}]: invalid {name} '{value}' — must be one of \
                         [{}]. Common mistakes: 'ingress'/'egress' → 'inbound'/'outbound', \
                         'allow'/'deny' → 'accept'/'drop'",
                        allowed.join(", ")
                    )))
                }
                Some(_) => {}
            }
        }
        if let Some(source_cidr) = field("source_cidr") {
            if !vocab::is_valid_cidr(source_cidr) {
                return Err(BffError::BadRequest(format!(
                    "firewall_rules[{index}]: invalid source_cidr '{source_cidr}' \
                     (expected a.b.c.d/p IPv4 or IPv6 CIDR)"
                )));
            }
        }
        if let Some(dest_port) = field("dest_port") {
            if !vocab::is_valid_port_spec(dest_port) {
                return Err(BffError::BadRequest(format!(
                    "firewall_rules[{index}]: invalid dest_port '{dest_port}' \
                     (expected a port like 443 or a range like 8080-8090)"
                )));
            }
        }
    }
    Ok(())
}

/// Check if the user is the owner of a network or an admin.
/// Returns Ok(()) if allowed, Err(BffError::Forbidden) if not.
pub(crate) async fn require_network_owner(
    conn: &mut sqlx::SqliteConnection,
    network_id: &str,
    user_id: &str,
    is_admin: bool,
) -> Result<(), BffError> {
    if is_admin {
        return Ok(());
    }
    let owner: Option<String> =
        sqlx::query_scalar("SELECT owner_id FROM networks WHERE network_id = ?")
            .bind(network_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| BffError::Internal(format!("failed to check network owner: {}", e)))?;
    match owner {
        Some(o) if o == user_id => Ok(()),
        None => {
            tracing::warn!(resource_id = %network_id, "ownership check failed: resource has no owner_id set");
            Err(BffError::Forbidden(
                "resource has no owner; admin access required".into(),
            ))
        }
        Some(_) => Err(BffError::Forbidden("you do not own this network".into())),
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

    let mut conn = state
        .pool
        .acquire()
        .await
        .map_err(|e| BffError::Internal(format!("failed to acquire connection: {}", e)))?;
    require_network_owner(&mut conn, &network_id, &claims.sub, claims.role == "admin").await?;

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
        .mutate_network(network_id, action, force, claims.sub)
        .await?;

    state.cache.invalidate("networks:").await;
    state.cache.invalidate("overview").await;
    Ok(Json(json!({
        "accepted": response.accepted,
        "task_id": response.task_id,
        "network_id": response.network_id,
        "summary": response.summary,
    })))
}

/// Default gateway for a network created without an explicit one (N6):
/// the cidr's first usable host — `10.200.0.1` for `10.200.0.0/24`, the
/// same value the vm-create implicit-network fallback always defaulted
/// to. `None` for a malformed or non-IPv4 cidr, or a prefix so large
/// (/31, /32) that network|1 is not a valid host — the gateway then
/// stays unset and the create proceeds unchanged (the existing L2-only
/// behavior, now an explicit operator choice rather than a silent one).
fn default_gateway_for_cidr(cidr: &str) -> String {
    fn default_gateway_for_cidr_inner(cidr: &str) -> Option<String> {
        let (addr, prefix) = cidr.split_once('/')?;
        let addr: std::net::Ipv4Addr = addr.parse().ok()?;
        let prefix: u32 = prefix.parse().ok()?;
        if prefix > 30 {
            // /31 and /32 have no separate host addresses; the point-to-
            // point and host routes leave no room for a gateway.
            return None;
        }
        let host = u32::from(addr) | 1;
        Some(std::net::Ipv4Addr::from(host).to_string())
    }
    default_gateway_for_cidr_inner(cidr).unwrap_or_default()
}
