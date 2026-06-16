use crate::migrations;
use crate::state::TopologyState;
use chv_errors::ChvError;
use rusqlite::{params, Connection};
use std::path::Path;

pub struct TopologyStore {
    conn: Connection,
}

impl TopologyStore {
    pub fn new(db_path: &Path) -> Result<Self, ChvError> {
        let mut conn = Connection::open(db_path).map_err(|e| ChvError::Io {
            path: db_path.to_string_lossy().to_string(),
            source: std::io::Error::other(e),
        })?;
        // Versioned migrations (see crates/chv-nwd-core/src/migrations/mod.rs).
        // Replaces the previous `let _ = conn.execute("ALTER TABLE …")`
        // pattern that swallowed every error — including racing concurrent
        // boots — and made ADR-007's rollback story structurally
        // unimplementable for nwd.
        migrations::run(&mut conn)?;
        Ok(Self { conn })
    }

    pub fn upsert(&self, state: &TopologyState) -> Result<(), ChvError> {
        let peer_vteps_json =
            serde_json::to_string(&state.peer_vteps).unwrap_or_else(|_| "[]".to_string());
        self.conn.execute(
            "INSERT INTO topologies (network_id, tenant_id, bridge_name, namespace_name, subnet_cidr, gateway_ip, runtime_status, vni, peer_vteps)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(network_id) DO UPDATE SET
               tenant_id = excluded.tenant_id,
               bridge_name = excluded.bridge_name,
               namespace_name = excluded.namespace_name,
               subnet_cidr = excluded.subnet_cidr,
               gateway_ip = excluded.gateway_ip,
               runtime_status = excluded.runtime_status,
               vni = excluded.vni,
               peer_vteps = excluded.peer_vteps",
            params![
                state.network_id, state.tenant_id, state.bridge_name,
                state.namespace_name, state.subnet_cidr, state.gateway_ip, state.runtime_status,
                state.vni.map(|v| v as i64),
                peer_vteps_json,
            ],
        ).map_err(|e| ChvError::Internal { reason: format!("sqlite upsert failed: {}", e) })?;
        Ok(())
    }

    pub fn remove(&self, network_id: &str) -> Result<(), ChvError> {
        self.conn
            .execute(
                "DELETE FROM topologies WHERE network_id = ?1",
                params![network_id],
            )
            .map_err(|e| ChvError::Internal {
                reason: format!("sqlite remove failed: {}", e),
            })?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<TopologyState>, ChvError> {
        let mut stmt = self.conn.prepare(
            "SELECT network_id, tenant_id, bridge_name, namespace_name, subnet_cidr, gateway_ip, runtime_status, vni, peer_vteps FROM topologies"
        ).map_err(|e| ChvError::Internal { reason: format!("sqlite prepare failed: {}", e) })?;
        let rows = stmt
            .query_map([], |row| {
                let vni_raw: Option<i64> = row.get(7)?;
                let peer_vteps_raw: Option<String> = row.get(8)?;
                let peer_vteps: Vec<String> = peer_vteps_raw
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default();
                Ok(TopologyState {
                    network_id: row.get(0)?,
                    tenant_id: row.get(1)?,
                    bridge_name: row.get(2)?,
                    namespace_name: row.get(3)?,
                    subnet_cidr: row.get(4)?,
                    gateway_ip: row.get(5)?,
                    runtime_status: row.get(6)?,
                    vni: vni_raw.map(|v| v as u32),
                    peer_vteps,
                })
            })
            .map_err(|e| ChvError::Internal {
                reason: format!("sqlite query failed: {}", e),
            })?;
        let mut states = Vec::new();
        for row in rows {
            states.push(row.map_err(|e| ChvError::Internal {
                reason: format!("sqlite row failed: {}", e),
            })?);
        }
        Ok(states)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_state(network_id: &str) -> TopologyState {
        TopologyState {
            network_id: network_id.to_string(),
            tenant_id: "t1".to_string(),
            bridge_name: format!("br-{}", network_id),
            namespace_name: format!("ns-{}", network_id),
            subnet_cidr: "10.0.0.0/24".to_string(),
            gateway_ip: "10.0.0.1".to_string(),
            runtime_status: "ensured".to_string(),
            vni: None,
            peer_vteps: Vec::new(),
        }
    }

    #[test]
    fn store_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = TopologyStore::new(&dir.path().join("nwd.db")).unwrap();
        let s = dummy_state("net-1");
        store.upsert(&s).unwrap();
        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].network_id, "net-1");
        store.remove("net-1").unwrap();
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn store_roundtrip_with_peer_vteps() {
        let dir = tempfile::tempdir().unwrap();
        let store = TopologyStore::new(&dir.path().join("nwd.db")).unwrap();
        let s = TopologyState {
            peer_vteps: vec!["10.0.1.1".to_string(), "10.0.1.2".to_string()],
            vni: Some(100),
            ..dummy_state("net-vtep")
        };
        store.upsert(&s).unwrap();
        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].peer_vteps, vec!["10.0.1.1", "10.0.1.2"]);
        assert_eq!(list[0].vni, Some(100));
    }

    #[test]
    fn migrations_run_to_completion_on_fresh_db() {
        // Required test #1: applies all migrations to an empty SQLite,
        // asserts schema_version table tracks them.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nwd.db");
        let _store = TopologyStore::new(&path).unwrap();

        let conn = rusqlite::Connection::open(&path).unwrap();
        let versions = crate::migrations::applied_versions(&conn).unwrap();
        assert_eq!(
            versions,
            vec![1, 2, 3],
            "every embedded migration must be recorded after first boot"
        );

        // Sanity-check that the version-tracking table itself is present.
        let table_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='_chv_nwd_schema_version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(table_exists, 1, "version table must exist after migrations");
    }

    #[test]
    fn migrations_idempotent_on_re_run() {
        // Required test #2: runs migrations twice in a row; second run is a no-op.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nwd.db");

        let _store_first = TopologyStore::new(&path).unwrap();
        // Re-opening the store re-runs migrations; this must not error and
        // must not add duplicate version rows.
        let _store_second = TopologyStore::new(&path).unwrap();

        let conn = rusqlite::Connection::open(&path).unwrap();
        let versions = crate::migrations::applied_versions(&conn).unwrap();
        assert_eq!(
            versions,
            vec![1, 2, 3],
            "rerunning migrations must not duplicate version rows"
        );
    }

    #[test]
    fn migrations_propagate_real_errors() {
        // Required test #3: simulate a hard error (bad SQL in a test-only
        // migration vector) and assert it propagates as a typed error,
        // NOT swallowed.
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        let bad = [crate::migrations::test_support::migration(
            42,
            "intentionally_broken",
            "THIS IS NOT VALID SQL;",
        )];

        let err = crate::migrations::run_with(&mut conn, &bad)
            .expect_err("bad SQL must surface as ChvError, not be swallowed");

        match err {
            ChvError::Internal { reason } => {
                assert!(
                    reason.contains("intentionally_broken"),
                    "error must name the failing migration; got: {reason}"
                );
            }
            other => panic!("expected ChvError::Internal, got {other:?}"),
        }

        // Confirm the failing migration was NOT recorded; the transaction
        // rolled back, so applied_versions stays empty.
        let versions = crate::migrations::applied_versions(&conn).unwrap();
        assert!(
            versions.is_empty(),
            "failed migration must not appear in version history"
        );
    }

    #[test]
    fn store_upgrades_legacy_unversioned_db_in_place() {
        // Verifies the in-place upgrade contract: a database created by the
        // prior unversioned `let _ = ALTER TABLE` code path must converge to
        // schema_version=3 without erroring on the duplicate columns.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nwd.db");
        {
            let legacy = rusqlite::Connection::open(&path).unwrap();
            legacy
                .execute(
                    "CREATE TABLE topologies (
                        network_id TEXT PRIMARY KEY,
                        tenant_id TEXT NOT NULL,
                        bridge_name TEXT NOT NULL,
                        namespace_name TEXT NOT NULL,
                        subnet_cidr TEXT NOT NULL,
                        gateway_ip TEXT NOT NULL,
                        runtime_status TEXT NOT NULL,
                        vni INTEGER,
                        peer_vteps TEXT DEFAULT '[]'
                    )",
                    [],
                )
                .unwrap();
        }

        let store = TopologyStore::new(&path).expect("legacy in-place upgrade must succeed");
        // Round-trip works after the upgrade.
        store
            .upsert(&TopologyState {
                vni: Some(7),
                peer_vteps: vec!["10.0.0.9".to_string()],
                ..dummy_state("net-legacy")
            })
            .unwrap();

        let conn = rusqlite::Connection::open(&path).unwrap();
        let versions = crate::migrations::applied_versions(&conn).unwrap();
        assert_eq!(versions, vec![1, 2, 3]);
    }
}
