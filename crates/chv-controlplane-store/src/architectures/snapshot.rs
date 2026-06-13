//! Inventory snapshot repository.
//!
//! Snapshots are the basis for plans and drift reports. They are
//! append-only at this layer; pruning is a future task.

use crate::architectures::parse_ts;
use crate::{StoreError, StorePool};
use chv_controlplane_types::architecture::{InventorySnapshot, InventorySnapshotId};
use sqlx::Row;

const ENTITY: &str = "inventory_snapshot";

#[derive(Clone, Debug)]
pub struct InventorySnapshotCreateInput {
    pub id: InventorySnapshotId,
    pub source: String,
    pub snapshot_json: String,
    pub summary_json: Option<String>,
    pub captured_by: Option<String>,
}

#[derive(Clone)]
pub struct InventorySnapshotRepository {
    pool: StorePool,
}

impl InventorySnapshotRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &StorePool {
        &self.pool
    }

    pub async fn create(
        &self,
        input: InventorySnapshotCreateInput,
    ) -> Result<InventorySnapshot, StoreError> {
        let row = sqlx::query(
            r#"
            INSERT INTO inventory_snapshots (
                id, source, snapshot_json, summary_json, captured_by
            )
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(input.id.as_str())
        .bind(&input.source)
        .bind(&input.snapshot_json)
        .bind(&input.summary_json)
        .bind(&input.captured_by)
        .fetch_one(&self.pool)
        .await?;

        row_to_snapshot(&row)
    }

    pub async fn get(&self, id: &InventorySnapshotId) -> Result<InventorySnapshot, StoreError> {
        let row = sqlx::query(r#"SELECT * FROM inventory_snapshots WHERE id = $1"#)
            .bind(id.as_str())
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| StoreError::NotFound {
                entity: ENTITY,
                id: id.to_string(),
            })?;
        row_to_snapshot(&row)
    }

    pub async fn list_recent(&self, limit: i64) -> Result<Vec<InventorySnapshot>, StoreError> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM inventory_snapshots
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_snapshot).collect()
    }

    /// Capture-from-fleet is a Phase 1+ concern; left as a stub so callers
    /// can route to it without conditional compilation.
    pub async fn capture_from_fleet(&self) -> Result<InventorySnapshot, StoreError> {
        Err(StoreError::NotImplemented {
            reason: "InventorySnapshotRepository::capture_from_fleet is a Phase 1+ concern",
        })
    }
}

fn row_to_snapshot(row: &sqlx::sqlite::SqliteRow) -> Result<InventorySnapshot, StoreError> {
    let id_str: String = row.try_get("id")?;
    let id =
        InventorySnapshotId::new(id_str).map_err(|err| StoreError::InvalidConfiguration {
            reason: format!("invalid id in inventory snapshot row: {err}"),
        })?;
    let created_at: String = row.try_get("created_at")?;

    Ok(InventorySnapshot {
        id,
        source: row.try_get("source")?,
        snapshot_json: row.try_get("snapshot_json")?,
        summary_json: row.try_get("summary_json")?,
        captured_by: row.try_get("captured_by")?,
        created_at: parse_ts(&created_at, "created_at")?,
    })
}
