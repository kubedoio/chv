//! Read-only `NetworkRepository` used by the Architecture Designer fleet
//! checks. The `networks` table only carries identity/display columns;
//! `cidr` (and `gateway`, etc.) live on `network_desired_state`. We join
//! the two so a single `list()` call yields what the validator needs.
//!
//! Bridge name and VLAN id are not modeled in the live schema yet (the
//! agent inventory does not surface them), so the corresponding fields
//! stay `None`. Architecture-Designer fleet checks for `BRIDGE_UNAVAILABLE`
//! / `VLAN_UNAVAILABLE` therefore never fire against a real fleet today —
//! a follow-up migration will add the columns once the agent reports them.
//!
//! Read-only by design — networks are created/updated through the
//! existing CRUD path (`crate::desired_state::NetworkDesiredStateInput`).

use crate::{StoreError, StorePool};
use sqlx::Row;

/// Flat view of a network row joined with its desired-state CIDR.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkRow {
    pub id: String,
    pub name: String,
    /// Always `None` until the schema gains a bridge column. Kept on the
    /// row so callers can map straight to `NetworkInfo` without an extra
    /// branch.
    pub bridge: Option<String>,
    /// Always `None` until the schema gains a vlan_id column.
    pub vlan_id: Option<i64>,
    pub cidr: Option<String>,
}

#[derive(Clone)]
pub struct NetworkRepository {
    pool: StorePool,
}

impl NetworkRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &StorePool {
        &self.pool
    }

    /// List every network with its desired-state CIDR (when known).
    pub async fn list(&self) -> Result<Vec<NetworkRow>, StoreError> {
        let rows = sqlx::query(
            r#"
            SELECT
                n.network_id   AS id,
                n.display_name AS name,
                d.cidr         AS cidr
            FROM networks n
            LEFT JOIN network_desired_state d ON d.network_id = n.network_id
            ORDER BY n.network_id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.iter()
            .map(|row| {
                Ok(NetworkRow {
                    id: row.try_get("id")?,
                    name: row.try_get("name")?,
                    bridge: None,
                    vlan_id: None,
                    cidr: row.try_get("cidr")?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::create_test_pool;

    #[tokio::test]
    async fn list_returns_networks_with_cidr_from_desired_state() {
        let pool = create_test_pool().await;

        // Seed one network with a desired-state CIDR.
        sqlx::query(
            r#"INSERT INTO networks (network_id, display_name, network_class)
               VALUES ('net-1', 'edge', 'bridge')"#,
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO network_desired_state
               (network_id, desired_generation, desired_status, cidr)
               VALUES ('net-1', 1, 'Active', '10.0.0.0/24')"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let repo = NetworkRepository::new(pool);
        let rows = repo.list().await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "net-1");
        assert_eq!(rows[0].name, "edge");
        assert_eq!(rows[0].cidr.as_deref(), Some("10.0.0.0/24"));
        assert!(rows[0].bridge.is_none());
        assert!(rows[0].vlan_id.is_none());
    }
}
