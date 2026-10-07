//! Live `InventoryProvider` implementation backed by the control-plane
//! SQLite repositories.
//!
//! **Datastores (#514, follow-up to #542's STOP branch)**:
//! `list_datastores` now parses the string array enrollment actually
//! persists. Enrollment writes `node_inventory.storage_classes` as a
//! JSON array of class STRINGS (`chv-controlplane-service/src/
//! enrollment.rs`) — e.g. `["local","lvm"]`. Each class string
//! becomes a datastore entry named by the class, with `kind` = the
//! raw class string (the two vocabularies — class strings vs the
//! architecture's `DatastoreType` wire kinds — are disjoint; inventing
//! a class→wire-kind mapping would be a lie, so the derivation stays
//! truthful and the drift check's kind comparison may mint a
//! kind-changed finding where a baseline wire kind meets a live class
//! kind — disclosed, see `drift/compute.rs`) and capacity/free =
//! `None` — NEVER a fabricated number (the 2026-10-07 ruling's
//! never-fabricate prohibition, made representable by #514's
//! `Option<u64>` reshaping of `DatastoreInfo`). An object shape
//! `{"name","kind","capacity_gb","free_gb"}` (no writer in the tree
//! persists it; only tests construct it) still parses, with numbers
//! mapping to `Some` and absent fields mapping to `None` — the old
//! `.unwrap_or(0)` defaults were fabrications and are gone. The fleet
//! check downgrades the capacity verdict to a warning when free is
//! unknown (the `*_complete`-flag precedent), and class-named entries
//! now exist in the live set, so architectures naming a class the
//! node offers no longer hit blocking `DATASTORE_NOT_FOUND` — the
//! truthful direction (the node DOES offer the class). Backup targets
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
        // #514 (follow-up ruling, 2026-10-07 — supersedes #542's STOP
        // branch): enrollment persists `node_inventory.storage_classes`
        // as a JSON array of class STRINGS (e.g. `["local","lvm"]` —
        // see `chv-controlplane-service/src/enrollment.rs`), and this
        // parse now handles exactly that: each class string becomes a
        // datastore entry named by the class, `kind` = the raw class
        // string (NEVER an invented class→wire-kind mapping), and
        // capacity/free = `None` — unknown, never a fabricated number
        // (the ruling's never-fabricate prohibition stands: a zero or
        // invented number in place of `None` must fail loudly, which
        // `list_datastores_string_array_blob_yields_class_entries`
        // pins). The object shape `{ name, kind, capacity_gb,
        // free_gb }` (no production writer persists it; only tests
        // construct it) keeps parsing — numbers map to `Some`, absent
        // fields map to `None` (the old `.unwrap_or(0)` defaults were
        // fabrications). An absent/empty blob still yields no entries.
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
        // Aggregating an unknown with a known is unknown: if any host
        // reporting a name has no capacity number, the aggregate is
        // `None` — summing a known with an unknown would fabricate a
        // total the fleet never reported. BOTH branches below route
        // through `sum_known`, so the poisoning is order-independent
        // (a same-name string class arriving after an object with
        // numbers poisons the aggregate just as the reverse does).
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
                // Object shape: `{ name, kind, capacity_gb, free_gb }`
                // — numbers become `Some`, absent fields `None`.
                if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                    let kind = item
                        .get("kind")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let capacity_gb = item.get("capacity_gb").and_then(|v| v.as_u64());
                    let free_gb = item.get("free_gb").and_then(|v| v.as_u64());
                    acc.entry(name.to_string())
                        .and_modify(|existing| {
                            existing.capacity_gb = sum_known(existing.capacity_gb, capacity_gb);
                            existing.free_gb = sum_known(existing.free_gb, free_gb);
                        })
                        .or_insert(DatastoreInfo {
                            name: name.to_string(),
                            kind,
                            capacity_gb,
                            free_gb,
                            host: Some(node_id.clone()),
                        });
                } else if let Some(class) = item.as_str() {
                    // String shape — what enrollment actually persists.
                    // Each class string is a datastore entry named by
                    // the class; kind is the raw class string; capacity
                    // is unknown (`None`), never fabricated. The
                    // `and_modify` routes the unknown through the same
                    // `sum_known` poisoning path as the object branch:
                    // an unknown term poisons an EXISTING aggregate
                    // too (a bare `or_insert` would be a no-op on an
                    // existing key and silently drop the unknown,
                    // making the invariant order-dependent). First
                    // entry still wins for kind/host.
                    acc.entry(class.to_string())
                        .and_modify(|existing| {
                            existing.capacity_gb = sum_known(existing.capacity_gb, None);
                            existing.free_gb = sum_known(existing.free_gb, None);
                        })
                        .or_insert(DatastoreInfo {
                            name: class.to_string(),
                            kind: class.to_string(),
                            capacity_gb: None,
                            free_gb: None,
                            host: Some(node_id.clone()),
                        });
                }
                // Anything else (a number, a bool, an object without a
                // `name`) is not a shape any writer persists; skip it.
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

/// Sum two capacity numbers for cross-host aggregation. `Some` +
/// `Some` = `Some(sum)`; anything else is `None` — if any host
/// reporting a datastore name has no capacity number, the aggregate
/// is unknown. Fabricating a total over an unknown term is the exact
/// lie #514's never-fabricate prohibition forbids.
fn sum_known(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.saturating_add(b)),
        _ => None,
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
        // The OBJECT shape — no production writer persists it (only
        // tests construct it); the shape enrollment actually persists
        // is the string array covered by
        // `list_datastores_string_array_blob_yields_class_entries`
        // (#514).
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
        assert_eq!(stores[0].capacity_gb, Some(2000), "summed across nodes");
        assert_eq!(stores[0].free_gb, Some(1000));
    }

    #[tokio::test]
    async fn list_datastores_object_shape_without_capacity_fields_is_none() {
        // The object shape `{ name, kind, capacity_gb, free_gb }` (no
        // production writer persists it — tests only) maps numbers to
        // `Some` and ABSENT fields to `None`. The old parse defaulted
        // missing fields to 0 — a fabrication; this pins the honest
        // handling (#514's never-fabricate rule).
        let p = build_provider().await;
        let pool = p.nodes.pool().clone();

        sqlx::query(
            r#"INSERT INTO nodes (node_id, hostname, display_name)
               VALUES ('n1', 'h1', 'h1')"#,
        )
        .execute(&pool)
        .await
        .unwrap();
        let blob = r#"[{"name":"fast","kind":"nvme"}]"#;
        sqlx::query(
            r#"INSERT INTO node_inventory (node_id, architecture, cpu_count, memory_bytes, storage_classes)
               VALUES ('n1', 'x86_64', 1, 1, ?1)"#,
        )
        .bind(blob)
        .execute(&pool)
        .await
        .unwrap();

        let stores = p.list_datastores().await.unwrap();
        assert_eq!(stores.len(), 1);
        assert_eq!(stores[0].name, "fast");
        assert_eq!(
            stores[0].capacity_gb, None,
            "absent capacity is unknown, not 0"
        );
        assert_eq!(stores[0].free_gb, None, "absent free is unknown, not 0");
    }

    #[tokio::test]
    async fn list_datastores_unknown_capacity_poisons_the_aggregate() {
        // Cross-host aggregation: `Some + Some = Some(sum)`, but any
        // `None` term makes the aggregate `None` — summing a known
        // with an unknown would fabricate a total the fleet never
        // reported (#514's never-fabricate rule).
        let p = build_provider().await;
        let pool = p.nodes.pool().clone();

        sqlx::query(
            r#"INSERT INTO nodes (node_id, hostname, display_name)
               VALUES ('n1', 'h1', 'h1'), ('n2', 'h2', 'h2')"#,
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO node_inventory (node_id, architecture, cpu_count, memory_bytes, storage_classes)
               VALUES ('n1', 'x86_64', 1, 1, ?1)"#,
        )
        .bind(r#"[{"name":"fast","kind":"nvme","capacity_gb":1000,"free_gb":500}]"#)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO node_inventory (node_id, architecture, cpu_count, memory_bytes, storage_classes)
               VALUES ('n2', 'x86_64', 1, 1, ?1)"#,
        )
        .bind(r#"[{"name":"fast","kind":"nvme"}]"#)
        .execute(&pool)
        .await
        .unwrap();

        let stores = p.list_datastores().await.unwrap();
        assert_eq!(stores.len(), 1);
        assert_eq!(stores[0].capacity_gb, None, "known + unknown = unknown");
        assert_eq!(stores[0].free_gb, None);
    }

    #[tokio::test]
    async fn list_datastores_string_and_object_same_name_poison_both_orderings() {
        // Review-fold pin (PR #546 SHOULD-FIX): a same-name class
        // string arriving AFTER an object with numbers must poison
        // the aggregate just as the reverse ordering does — the
        // string branch routes through the same `sum_known` poisoning
        // path instead of a bare `or_insert` (which was a no-op on an
        // existing key and silently dropped the second host's
        // unknown, making the documented invariant order-dependent).
        //
        // Both permutations are covered in one scan-order-agnostic
        // test: "alpha" carries numbers on n1 and the string on n2,
        // "beta" the mirror — whichever order SQLite visits the rows,
        // each name sees one known and one unknown term, and both
        // aggregates must be `None`.
        let p = build_provider().await;
        let pool = p.nodes.pool().clone();

        sqlx::query(
            r#"INSERT INTO nodes (node_id, hostname, display_name)
               VALUES ('n1', 'h1', 'h1'), ('n2', 'h2', 'h2')"#,
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO node_inventory (node_id, architecture, cpu_count, memory_bytes, storage_classes)
               VALUES ('n1', 'x86_64', 1, 1, ?1)"#,
        )
        // alpha: object with numbers; beta: class string.
        .bind(r#"[{"name":"alpha","kind":"nvme","capacity_gb":1000,"free_gb":500},"beta"]"#)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO node_inventory (node_id, architecture, cpu_count, memory_bytes, storage_classes)
               VALUES ('n2', 'x86_64', 1, 1, ?1)"#,
        )
        // alpha: class string; beta: object with numbers.
        .bind(r#"["alpha",{"name":"beta","kind":"nvme","capacity_gb":1000,"free_gb":500}]"#)
        .execute(&pool)
        .await
        .unwrap();

        let stores = p.list_datastores().await.unwrap();
        assert_eq!(stores.len(), 2);
        let by_name: std::collections::BTreeMap<&str, &DatastoreInfo> =
            stores.iter().map(|s| (s.name.as_str(), s)).collect();
        for name in ["alpha", "beta"] {
            let ds = by_name.get(name).expect("same-name entries aggregate");
            assert_eq!(
                ds.capacity_gb, None,
                "{name}: known + unknown = unknown regardless of visit order"
            );
            assert_eq!(ds.free_gb, None, "{name}: free poisoned the same way");
        }
    }

    #[tokio::test]
    async fn list_datastores_string_array_blob_yields_class_entries() {
        // FLIPS #542's pin (`list_datastores_string_array_blob_stays_empty`)
        // — that flip is the point of the follow-up ruling (2026-10-07),
        // which Option-ified `DatastoreInfo.capacity_gb`/`free_gb` so the
        // ruled repair could land truthfully: each class string in the
        // enrollment-persisted blob (exactly what `serde_json::to_value`
        // on the agent's `Vec<String>` produces) becomes a datastore
        // entry named by the class, with `kind` = the raw class string
        // and capacity/free = `None` — the truthful representation of
        // unknown. The never-fabricate prohibition STANDS: a zero or
        // invented number in place of `None` must still fail loudly,
        // here and in the fleet check's unknown-capacity warning path.
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
        assert_eq!(
            stores.len(),
            2,
            "each class string yields one class-named entry (#514 follow-up ruling)"
        );
        let by_name: std::collections::BTreeMap<&str, &DatastoreInfo> =
            stores.iter().map(|s| (s.name.as_str(), s)).collect();
        let local = by_name
            .get("local")
            .expect("class 'local' becomes an entry");
        assert_eq!(
            local.kind, "local",
            "kind is the raw class string, never an invented wire kind"
        );
        assert_eq!(
            local.capacity_gb, None,
            "unknown capacity is None, never a fabricated number (2026-10-07 ruling)"
        );
        assert_eq!(local.free_gb, None);
        let lvm = by_name.get("lvm").expect("class 'lvm' becomes an entry");
        assert_eq!(lvm.kind, "lvm");
        assert_eq!(lvm.capacity_gb, None);
        assert_eq!(lvm.free_gb, None);
    }
}
