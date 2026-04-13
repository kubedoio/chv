use crate::state::TopologyState;
use chv_errors::ChvError;
use rusqlite::{params, Connection};
use std::path::Path;

pub struct TopologyStore {
    conn: Connection,
}

impl TopologyStore {
    pub fn new(db_path: &Path) -> Result<Self, ChvError> {
        let conn = Connection::open(db_path).map_err(|e| ChvError::Io {
            path: db_path.to_string_lossy().to_string(),
            source: std::io::Error::new(std::io::ErrorKind::Other, e),
        })?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS topologies (
                network_id TEXT PRIMARY KEY,
                tenant_id TEXT NOT NULL,
                bridge_name TEXT NOT NULL,
                namespace_name TEXT NOT NULL,
                subnet_cidr TEXT NOT NULL,
                gateway_ip TEXT NOT NULL,
                runtime_status TEXT NOT NULL
            )",
            [],
        ).map_err(|e| ChvError::Internal { reason: format!("sqlite init failed: {}", e) })?;
        Ok(Self { conn })
    }

    pub fn upsert(&self, state: &TopologyState) -> Result<(), ChvError> {
        self.conn.execute(
            "INSERT INTO topologies (network_id, tenant_id, bridge_name, namespace_name, subnet_cidr, gateway_ip, runtime_status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(network_id) DO UPDATE SET
               tenant_id = excluded.tenant_id,
               bridge_name = excluded.bridge_name,
               namespace_name = excluded.namespace_name,
               subnet_cidr = excluded.subnet_cidr,
               gateway_ip = excluded.gateway_ip,
               runtime_status = excluded.runtime_status",
            params![
                state.network_id, state.tenant_id, state.bridge_name,
                state.namespace_name, state.subnet_cidr, state.gateway_ip, state.runtime_status,
            ],
        ).map_err(|e| ChvError::Internal { reason: format!("sqlite upsert failed: {}", e) })?;
        Ok(())
    }

    pub fn remove(&self, network_id: &str) -> Result<(), ChvError> {
        self.conn.execute(
            "DELETE FROM topologies WHERE network_id = ?1",
            params![network_id],
        ).map_err(|e| ChvError::Internal { reason: format!("sqlite remove failed: {}", e) })?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<TopologyState>, ChvError> {
        let mut stmt = self.conn.prepare(
            "SELECT network_id, tenant_id, bridge_name, namespace_name, subnet_cidr, gateway_ip, runtime_status FROM topologies"
        ).map_err(|e| ChvError::Internal { reason: format!("sqlite prepare failed: {}", e) })?;
        let rows = stmt.query_map([], |row| {
            Ok(TopologyState {
                network_id: row.get(0)?,
                tenant_id: row.get(1)?,
                bridge_name: row.get(2)?,
                namespace_name: row.get(3)?,
                subnet_cidr: row.get(4)?,
                gateway_ip: row.get(5)?,
                runtime_status: row.get(6)?,
            })
        }).map_err(|e| ChvError::Internal { reason: format!("sqlite query failed: {}", e) })?;
        let mut states = Vec::new();
        for row in rows {
            states.push(row.map_err(|e| ChvError::Internal { reason: format!("sqlite row failed: {}", e) })?);
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
}
