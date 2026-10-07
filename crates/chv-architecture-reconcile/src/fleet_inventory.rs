//! Live `InventoryProvider` implementation backed by the control-plane
//! SQLite repositories.
//!
//! **Datastores are dead as operated (#514)**: `list_datastores`
//! derives from `node_inventory.storage_classes`, but the two ends of
//! that seam disagree — enrollment persists the agent's report as a
//! JSON array of class STRINGS (`chv-controlplane-service/src/
//! enrollment.rs`), while the parse below expects an array of
//! `{ name, kind, capacity_gb, free_gb }` objects. String items have
//! no `name`, so the parse yields nothing and the surface always
//! returns empty. The 2026-10-07 maintainer ruling STOPped the ruled
//! repair (map each class string to a datastore entry with unknown
//! capacity) because `DatastoreInfo::capacity_gb`/`free_gb` are
//! required `u64` — unknown cannot be represented without fabricating
//! a number, and a fabricated `free_gb: 0` would make the fleet check
//! emit blocking `DATASTORE_INSUFFICIENT_CAPACITY` findings ("0 GB
//! free") where the empty list yields the truthful
//! `DATASTORE_NOT_FOUND`. The truthful repair needs the capacity
//! fields made Optional (plus a ruling on what the capacity check
//! does with unknown) — see #514. Backup targets
//! likewise return an empty list with `complete = false` until a real
//! `BackupTargetRepository` lands; the validator downgrades the
//! corresponding `BACKUP_TARGET_UNREACHABLE` finding to a warning
//! while incomplete.
//!
//! `caller_can_deploy` is plumbed through as a constructor field. The
//! BFF flips it based on the caller's role; until the
//! `architecture:apply` permission lands the BFF passes `true` so we
//! never spuriously emit `PERMISSION_DENIED_DEPLOY`. Phase 4 wires the
//! real role check.

use async_trait::async_trait;
use chv_architecture_validate::fleet::{
    BackupTargetInfo, DatastoreInfo, FleetError, ImageInfo, InventoryProvider, NetworkInfo,
    NodeInfo,
};
use chv_controlplane_store::{ImageRepository, NetworkRepository, NodeRepository};
use sqlx::Row;
use std::collections::BTreeMap;

/// Constructed by the BFF; fields are public so wiring code reads as
/// data, matching the existing `AppState`-style construction in this
/// codebase rather than a builder.
#[derive(Clone)]
pub struct FleetInventoryProvider {
    pub nodes: NodeRepository,
    pub networks: NetworkRepository,
    pub images: ImageRepository,
    /// `true` when the caller currently holds the (future)
    /// `architecture:apply` permission. The BFF resolves this from the
    /// caller's role and passes it in.
    pub deploy_allowed_for_caller: bool,
}

#[async_trait]
impl InventoryProvider for FleetInventoryProvider {
    async fn list_nodes(&self) -> Result<Vec<NodeInfo>, FleetError> {
        let rows = sqlx::query(
            r#"
            SELECT
                n.node_id          AS node_id,
                n.hostname         AS hostname,
                n.display_name     AS display_name,
                inv.cpu_count      AS cpu_count,
                inv.memory_bytes   AS memory_bytes,
                COALESCE(s.scheduling_paused, 0) AS scheduling_paused
            FROM nodes n
            LEFT JOIN node_inventory inv ON inv.node_id = n.node_id
            LEFT JOIN node_desired_state s ON s.node_id = n.node_id
            ORDER BY n.node_id
            "#,
        )
        .fetch_all(self.nodes.pool())
        .await
        .map_err(|e| FleetError::Provider(format!("list_nodes: {e}")))?;

        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            let cpu: Option<i32> = row.try_get("cpu_count").ok();
            let mem_bytes: Option<i64> = row.try_get("memory_bytes").ok();
            let display_name: String = row.try_get("display_name").unwrap_or_default();
            let hostname: String = row.try_get("hostname").unwrap_or_default();
            let scheduling_paused: i64 = row.try_get("scheduling_paused").unwrap_or(0);

            // Use display_name when present, else fall back to hostname so
            // checks that match against `placement.server` strings have a
            // sensible identifier.
            let name = if !display_name.is_empty() {
                display_name
            } else {
                hostname
            };

            let memory_gb = mem_bytes
                .filter(|b| *b > 0)
                .map(|b| (b as u64) / (1024 * 1024 * 1024))
                .unwrap_or(0) as u32;
            let cpu_cores = cpu.filter(|c| *c > 0).unwrap_or(0) as u32;

            out.push(NodeInfo {
                name,
                schedulable: scheduling_paused == 0,
                cpu_cores,
                memory_gb,
                bridges: Vec::new(),
                vlans: Vec::new(),
                used_ips: Vec::new(),
            });
        }
        Ok(out)
    }

    async fn list_networks(&self) -> Result<Vec<NetworkInfo>, FleetError> {
        let rows = self
            .networks
            .list()
            .await
            .map_err(|e| FleetError::Provider(format!("list_networks: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|r| NetworkInfo {
                name: r.name,
                bridge: r.bridge,
                vlan_id: r.vlan_id.and_then(|v| u32::try_from(v).ok()),
                cidr: r.cidr,
            })
            .collect())
    }

    async fn list_datastores(&self) -> Result<Vec<DatastoreInfo>, FleetError> {
        // DEAD AS OPERATED (#514) — pinned by
        // `list_datastores_string_array_blob_stays_empty`. Enrollment
        // persists `node_inventory.storage_classes` as a JSON array of
        // class STRINGS (e.g. `["local","lvm"]` — see
        // `chv-controlplane-service/src/enrollment.rs`), but this parse
        // expects an array of `{ name, kind, capacity_gb, free_gb }`
        // objects — a shape no writer persists. String items never
        // yield a `name`, so the loop below matches nothing and this
        // always returns empty; fleet checks then emit
        // DATASTORE_NOT_FOUND for any architecture that names a
        // datastore, which is the truthful behavior today.
        //
        // The ruled repair (2026-10-07: map each class string to an
        // entry with name/kind from the class and capacity unknown)
        // is STOPped on a shape fact: `DatastoreInfo.capacity_gb` and
        // `free_gb` are required `u64`, so "unknown" cannot be
        // represented without fabricating a number — and a fabricated
        // `free_gb: 0` would flip the fleet check's
        // DATASTORE_NOT_FOUND into lying blocking
        // DATASTORE_INSUFFICIENT_CAPACITY ("0 GB free") findings. Do
        // NOT "fix" this parse by mapping strings onto zero-capacity
        // entries; the truthful repair needs the capacity fields made
        // Optional first (tracked in #514).
        let rows = sqlx::query(
            r#"
            SELECT n.node_id AS node_id, inv.storage_classes AS storage_classes
            FROM nodes n
            LEFT JOIN node_inventory inv ON inv.node_id = n.node_id
            "#,
        )
        .fetch_all(self.nodes.pool())
        .await
        .map_err(|e| FleetError::Provider(format!("list_datastores: {e}")))?;

        // Aggregate by datastore name; first entry wins for kind/host,
        // capacity/free sum across hosts that report the same name.
        let mut acc: BTreeMap<String, DatastoreInfo> = BTreeMap::new();
        for row in &rows {
            let node_id: String = row.try_get("node_id").unwrap_or_default();
            let blob: Option<String> = row.try_get("storage_classes").ok().flatten();
            let Some(blob) = blob else { continue };
            let val: serde_json::Value = match serde_json::from_str(&blob) {
                Ok(v) => v,
                Err(err) => {
                    tracing::warn!(
                        node_id = %node_id,
                        error = %err,
                        "skipping unparsable storage_classes blob"
                    );
                    continue;
                }
            };
            let arr = match val.as_array() {
                Some(a) => a,
                None => continue,
            };
            for item in arr {
                let Some(name) = item.get("name").and_then(|v| v.as_str()) else {
                    continue;
                };
                let kind = item
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let capacity_gb = item
                    .get("capacity_gb")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let free_gb = item.get("free_gb").and_then(|v| v.as_u64()).unwrap_or(0);
                acc.entry(name.to_string())
                    .and_modify(|existing| {
                        existing.capacity_gb = existing.capacity_gb.saturating_add(capacity_gb);
                        existing.free_gb = existing.free_gb.saturating_add(free_gb);
                    })
                    .or_insert(DatastoreInfo {
                        name: name.to_string(),
                        kind,
                        capacity_gb,
                        free_gb,
                        host: Some(node_id.clone()),
                    });
            }
        }
        Ok(acc.into_values().collect())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>, FleetError> {
        let rows = self
            .images
            .list()
            .await
            .map_err(|e| FleetError::Provider(format!("list_images: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|r| ImageInfo {
                name: r.display_name,
                format: r.format,
            })
            .collect())
    }

    async fn list_backup_targets(&self) -> Result<(Vec<BackupTargetInfo>, bool), FleetError> {
        // No `BackupTargetRepository` exists yet (Phase 3 stop-gap per the
        // task plan). Return an empty inventory and `complete = false` so
        // `BACKUP_TARGET_UNREACHABLE` findings degrade to warnings.
        Ok((Vec::new(), false))
    }

    async fn caller_can_deploy(&self) -> Result<bool, FleetError> {
        Ok(self.deploy_allowed_for_caller)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chv_controlplane_store::test_util::create_test_pool;

    async fn build_provider() -> FleetInventoryProvider {
        let pool = create_test_pool().await;
        FleetInventoryProvider {
            nodes: NodeRepository::new(pool.clone()),
            networks: NetworkRepository::new(pool.clone()),
            images: ImageRepository::new(pool),
            deploy_allowed_for_caller: true,
        }
    }

    #[tokio::test]
    async fn empty_provider_returns_empty_collections() {
        let p = build_provider().await;
        assert!(p.list_nodes().await.unwrap().is_empty());
        assert!(p.list_networks().await.unwrap().is_empty());
        assert!(p.list_datastores().await.unwrap().is_empty());
        assert!(p.list_images().await.unwrap().is_empty());
        let (targets, complete) = p.list_backup_targets().await.unwrap();
        assert!(targets.is_empty());
        assert!(!complete, "backup_targets must report incomplete");
        assert!(p.caller_can_deploy().await.unwrap());
    }

    #[tokio::test]
    async fn list_nodes_reflects_inventory_and_scheduling_paused() {
        let p = build_provider().await;
        let pool = p.nodes.pool().clone();

        sqlx::query(
            r#"INSERT INTO nodes (node_id, hostname, display_name)
               VALUES ('n1', 'host-1', 'node-one')"#,
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO node_inventory (node_id, architecture, cpu_count, memory_bytes)
               VALUES ('n1', 'x86_64', 8, 17179869184)"#, // 16 GiB
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO node_desired_state
               (node_id, desired_generation, desired_state, scheduling_paused)
               VALUES ('n1', 1, 'Running', 1)"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let nodes = p.list_nodes().await.unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "node-one");
        assert_eq!(nodes[0].cpu_cores, 8);
        assert_eq!(nodes[0].memory_gb, 16);
        assert!(
            !nodes[0].schedulable,
            "scheduling_paused=1 -> not schedulable"
        );
    }

    #[tokio::test]
    async fn list_datastores_aggregates_storage_classes_across_nodes() {
        let p = build_provider().await;
        let pool = p.nodes.pool().clone();

        sqlx::query(
            r#"INSERT INTO nodes (node_id, hostname, display_name)
               VALUES ('n1', 'h1', 'h1'), ('n2', 'h2', 'h2')"#,
        )
        .execute(&pool)
        .await
        .unwrap();
        let blob = r#"[{"name":"fast","kind":"nvme","capacity_gb":1000,"free_gb":500}]"#;
        sqlx::query(
            r#"INSERT INTO node_inventory (node_id, architecture, cpu_count, memory_bytes, storage_classes)
               VALUES ('n1', 'x86_64', 1, 1, ?1), ('n2', 'x86_64', 1, 1, ?1)"#,
        )
        .bind(blob)
        .execute(&pool)
        .await
        .unwrap();

        let stores = p.list_datastores().await.unwrap();
        assert_eq!(stores.len(), 1);
        assert_eq!(stores[0].name, "fast");
        assert_eq!(stores[0].capacity_gb, 2000, "summed across nodes");
        assert_eq!(stores[0].free_gb, 1000);
    }

    #[tokio::test]
    async fn list_datastores_string_array_blob_stays_empty() {
        // Pins the KNOWN-DEAD status quo of #514, not a desired
        // behavior: enrollment persists `storage_classes` as a JSON
        // array of class strings — exactly this blob, the output of
        // `serde_json::to_value` on the agent's `Vec<String>` — while
        // the parse expects `{ name, kind, capacity_gb, free_gb }`
        // objects, so the persisted reality yields no datastores.
        //
        // The maintainer ruling (2026-10-07) STOPped the ruled repair
        // (each class string -> an entry with name/kind from the class
        // and capacity unknown) because `DatastoreInfo`'s
        // `capacity_gb`/`free_gb` are required `u64`: unknown capacity
        // cannot be represented without fabricating a number, and a
        // fabricated `free_gb: 0` would make the fleet check emit
        // blocking `DATASTORE_INSUFFICIENT_CAPACITY` ("0 GB free")
        // findings where the empty list yields the truthful
        // `DATASTORE_NOT_FOUND`. Flipping this test green by mapping
        // strings onto zero-capacity entries is the exact lie the
        // ruling forbids; the truthful repair needs the capacity
        // fields made Optional first — tracked in #514.
        let p = build_provider().await;
        let pool = p.nodes.pool().clone();

        sqlx::query(
            r#"INSERT INTO nodes (node_id, hostname, display_name)
               VALUES ('n1', 'h1', 'h1')"#,
        )
        .execute(&pool)
        .await
        .unwrap();
        let blob = r#"["local","lvm"]"#;
        sqlx::query(
            r#"INSERT INTO node_inventory (node_id, architecture, cpu_count, memory_bytes, storage_classes)
               VALUES ('n1', 'x86_64', 1, 1, ?1)"#,
        )
        .bind(blob)
        .execute(&pool)
        .await
        .unwrap();

        let stores = p.list_datastores().await.unwrap();
        assert!(
            stores.is_empty(),
            "string-array blob yields no datastores (dead parse, #514); \
             repairing it with fabricated zero capacities is forbidden by \
             the 2026-10-07 ruling"
        );
    }
}
