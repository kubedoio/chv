//! VTEP registry and VNI allocation for overlay networking.

use crate::{StoreError, StorePool};

/// Repository for VTEP (Virtual Tunnel Endpoint) registry operations.
#[derive(Clone)]
pub struct VtepRepository {
    pool: StorePool,
}

/// A VTEP registry entry.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct VtepEntry {
    pub node_id: String,
    pub vtep_ip: String,
    pub vtep_port: i32,
    pub updated_at: String,
}

/// A VNI allocation entry.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct VniAllocation {
    pub vni: i32,
    pub network_id: String,
    pub allocated_at: String,
    pub released_at: Option<String>,
}

impl VtepRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &StorePool {
        &self.pool
    }

    /// Register or update a VTEP entry for a node.
    pub async fn register_vtep(
        &self,
        node_id: &str,
        vtep_ip: &str,
        vtep_port: i32,
    ) -> Result<(), StoreError> {
        sqlx::query(
            r#"INSERT INTO vtep_registry (node_id, vtep_ip, vtep_port, updated_at)
               VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
               ON CONFLICT(node_id) DO UPDATE SET
                   vtep_ip = excluded.vtep_ip,
                   vtep_port = excluded.vtep_port,
                   updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')"#,
        )
        .bind(node_id)
        .bind(vtep_ip)
        .bind(vtep_port)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get a single VTEP entry for a node.
    pub async fn get_vtep(&self, node_id: &str) -> Result<VtepEntry, StoreError> {
        sqlx::query_as::<_, VtepEntry>(
            "SELECT node_id, vtep_ip, vtep_port, updated_at FROM vtep_registry WHERE node_id = ?",
        )
        .bind(node_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StoreError::NotFound {
            entity: "vtep_registry",
            id: node_id.to_string(),
        })
    }

    /// Get VTEPs for all nodes that have VMs on a given network.
    /// This joins vtep_registry with vm placements that are attached to the network.
    pub async fn get_vteps_for_network(
        &self,
        network_id: &str,
    ) -> Result<Vec<VtepEntry>, StoreError> {
        let entries = sqlx::query_as::<_, VtepEntry>(
            r#"SELECT DISTINCT vr.node_id, vr.vtep_ip, vr.vtep_port, vr.updated_at
               FROM vtep_registry vr
               INNER JOIN vm_desired_state vds ON vds.target_node_id = vr.node_id
               INNER JOIN vm_nic_desired_state vnds ON vnds.vm_id = vds.vm_id
               WHERE vnds.network_id = ?"#,
        )
        .bind(network_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(entries)
    }

    /// Allocate the next free VNI for a network.
    /// VNI range: 1 to 16777214. Skips VNIs released less than 24 hours ago.
    pub async fn allocate_vni(&self, network_id: &str) -> Result<i32, StoreError> {
        // Find the next available VNI that is not currently allocated
        // and was not released within the last 24 hours
        let next_vni: Option<i32> = sqlx::query_scalar(
            r#"SELECT MIN(candidate.vni) FROM (
                   SELECT value AS vni FROM generate_series(1, 16777214)
                   WHERE value NOT IN (
                       SELECT vni FROM vni_allocations
                       WHERE released_at IS NULL
                          OR released_at > strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-24 hours')
                   )
                   LIMIT 1
               ) candidate"#,
        )
        .fetch_optional(&self.pool)
        .await?
        .flatten();

        // SQLite may not have generate_series. Use a simpler approach:
        // Find the max allocated VNI and use max+1, or scan for gaps.
        let vni = match next_vni {
            Some(v) => v,
            None => {
                // Fallback: find max current VNI and increment
                let max_vni: Option<i32> = sqlx::query_scalar(
                    r#"SELECT MAX(vni) FROM vni_allocations
                       WHERE released_at IS NULL
                          OR released_at > strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-24 hours')"#,
                )
                .fetch_optional(&self.pool)
                .await?
                .flatten();

                max_vni.unwrap_or(0) + 1
            }
        };

        if !(1..=16777214).contains(&vni) {
            return Err(StoreError::InvalidConfiguration {
                reason: "VNI address space exhausted".to_string(),
            });
        }

        sqlx::query(
            r#"INSERT INTO vni_allocations (vni, network_id, allocated_at)
               VALUES (?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))"#,
        )
        .bind(vni)
        .bind(network_id)
        .execute(&self.pool)
        .await?;

        // Also update the networks table
        sqlx::query("UPDATE networks SET vni = ? WHERE network_id = ?")
            .bind(vni)
            .bind(network_id)
            .execute(&self.pool)
            .await?;

        Ok(vni)
    }

    /// Release a VNI allocation for a network (sets released_at timestamp).
    pub async fn release_vni(&self, network_id: &str) -> Result<(), StoreError> {
        sqlx::query(
            r#"UPDATE vni_allocations
               SET released_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
               WHERE network_id = ? AND released_at IS NULL"#,
        )
        .bind(network_id)
        .execute(&self.pool)
        .await?;

        // Clear the VNI on the network
        sqlx::query("UPDATE networks SET vni = 0 WHERE network_id = ?")
            .bind(network_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Get the current VNI for a network.
    pub async fn get_vni_for_network(&self, network_id: &str) -> Result<Option<i32>, StoreError> {
        let vni: Option<i32> = sqlx::query_scalar(
            "SELECT vni FROM vni_allocations WHERE network_id = ? AND released_at IS NULL",
        )
        .bind(network_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(vni)
    }
}
