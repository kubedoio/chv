//! Drift report repository — CRUD for architecture_drift_reports.

use crate::architectures::parse_ts;
use crate::{StoreError, StorePool};
use chv_controlplane_types::architecture::{
    ArchitectureDriftReport, ArchitectureDriftReportId, ArchitectureId, ArchitectureVersionId,
    DriftStatus, InventorySnapshotId,
};
use chv_controlplane_types::domain::IdentifierError;
use sqlx::Row;

const ENTITY: &str = "architecture_drift_report";

#[derive(Clone, Debug)]
pub struct DriftReportCreateInput {
    pub id: ArchitectureDriftReportId,
    pub architecture_id: ArchitectureId,
    pub baseline_version_id: ArchitectureVersionId,
    pub inventory_snapshot_id: Option<InventorySnapshotId>,
    pub status: DriftStatus,
    pub summary_json: Option<String>,
    pub findings_json: Option<String>,
}

#[derive(Clone)]
pub struct DriftReportRepository {
    pool: StorePool,
}

impl DriftReportRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &StorePool {
        &self.pool
    }

    pub async fn create(
        &self,
        input: DriftReportCreateInput,
    ) -> Result<ArchitectureDriftReport, StoreError> {
        let row = sqlx::query(
            r#"
            INSERT INTO architecture_drift_reports (
                id,
                architecture_id,
                baseline_version_id,
                inventory_snapshot_id,
                status,
                summary_json,
                findings_json
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            "#,
        )
        .bind(input.id.as_str())
        .bind(input.architecture_id.as_str())
        .bind(input.baseline_version_id.as_str())
        .bind(input.inventory_snapshot_id.as_ref().map(|s| s.as_str()))
        .bind(input.status.as_str())
        .bind(&input.summary_json)
        .bind(&input.findings_json)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| match &err {
            sqlx::Error::Database(db_err) if db_err.is_foreign_key_violation() => {
                StoreError::NotFound {
                    entity: "architecture_topology_or_version",
                    id: input.architecture_id.to_string(),
                }
            }
            _ => StoreError::from(err),
        })?;

        row_to_drift(&row)
    }

    pub async fn get(
        &self,
        id: &ArchitectureDriftReportId,
    ) -> Result<ArchitectureDriftReport, StoreError> {
        let row = sqlx::query(r#"SELECT * FROM architecture_drift_reports WHERE id = $1"#)
            .bind(id.as_str())
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| StoreError::NotFound {
                entity: ENTITY,
                id: id.to_string(),
            })?;
        row_to_drift(&row)
    }

    pub async fn list_for_architecture(
        &self,
        architecture_id: &ArchitectureId,
    ) -> Result<Vec<ArchitectureDriftReport>, StoreError> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM architecture_drift_reports
            WHERE architecture_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(architecture_id.as_str())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_drift).collect()
    }

    /// Drift detection workflow is a Phase 1+ concern.
    pub async fn detect(&self) -> Result<ArchitectureDriftReport, StoreError> {
        Err(StoreError::NotImplemented {
            reason: "DriftReportRepository::detect is a Phase 1+ concern",
        })
    }
}

fn parse_drift_status(s: &str) -> Result<DriftStatus, StoreError> {
    match s {
        "unknown" => Ok(DriftStatus::Unknown),
        "no_drift" => Ok(DriftStatus::NoDrift),
        "drifted" => Ok(DriftStatus::Drifted),
        "check_failed" => Ok(DriftStatus::CheckFailed),
        other => Err(StoreError::InvalidConfiguration {
            reason: format!("unrecognized drift status: {other}"),
        }),
    }
}

fn opt_id<F, T>(value: Option<String>, ctor: F) -> Result<Option<T>, StoreError>
where
    F: FnOnce(String) -> Result<T, IdentifierError>,
{
    match value {
        None => Ok(None),
        Some(v) => ctor(v)
            .map(Some)
            .map_err(|err| StoreError::InvalidConfiguration {
                reason: format!("invalid id stored in drift_report row: {err}"),
            }),
    }
}

fn row_to_drift(row: &sqlx::sqlite::SqliteRow) -> Result<ArchitectureDriftReport, StoreError> {
    let id_str: String = row.try_get("id")?;
    let id =
        ArchitectureDriftReportId::new(id_str).map_err(|err| StoreError::InvalidConfiguration {
            reason: format!("invalid id in drift_report row: {err}"),
        })?;
    let arch_id_str: String = row.try_get("architecture_id")?;
    let architecture_id =
        ArchitectureId::new(arch_id_str).map_err(|err| StoreError::InvalidConfiguration {
            reason: format!("invalid architecture_id in drift_report row: {err}"),
        })?;
    let baseline_version_id_str: String = row.try_get("baseline_version_id")?;
    let baseline_version_id =
        ArchitectureVersionId::new(baseline_version_id_str).map_err(|err| {
            StoreError::InvalidConfiguration {
                reason: format!("invalid baseline_version_id in drift_report row: {err}"),
            }
        })?;
    let status_str: String = row.try_get("status")?;
    let created_at: String = row.try_get("created_at")?;

    Ok(ArchitectureDriftReport {
        id,
        architecture_id,
        baseline_version_id,
        inventory_snapshot_id: opt_id(
            row.try_get("inventory_snapshot_id")?,
            InventorySnapshotId::new,
        )?,
        status: parse_drift_status(&status_str)?,
        summary_json: row.try_get("summary_json")?,
        findings_json: row.try_get("findings_json")?,
        created_at: parse_ts(&created_at, "created_at")?,
    })
}
