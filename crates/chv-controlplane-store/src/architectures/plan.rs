//! Plan repository — CRUD for architecture_plans.
//!
//! The "generate a plan from inventory + version" workflow is a Phase 1+
//! concern; [`PlanRepository::generate`] is a stub returning
//! [`StoreError::NotImplemented`].

use crate::architectures::{format_ts, parse_ts, parse_ts_opt};
use crate::{StoreError, StorePool};
use chv_controlplane_types::architecture::{
    ArchitectureId, ArchitecturePlan, ArchitecturePlanId, ArchitectureVersionId,
    InventorySnapshotId, PlanMode, PlanStatus,
};
use chv_controlplane_types::domain::IdentifierError;
use sqlx::Row;

const ENTITY: &str = "architecture_plan";

#[derive(Clone, Debug)]
pub struct PlanCreateInput {
    pub id: ArchitecturePlanId,
    pub architecture_id: ArchitectureId,
    pub architecture_version_id: ArchitectureVersionId,
    pub inventory_snapshot_id: Option<InventorySnapshotId>,
    pub mode: PlanMode,
    pub status: PlanStatus,
    pub plan_json: Option<String>,
    pub summary_json: Option<String>,
    pub created_by: Option<String>,
    /// Application code computes this as `created_at + 15min` per
    /// ADR-004-Designer.
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug)]
pub struct PlanStatusUpdateInput {
    pub id: ArchitecturePlanId,
    pub status: PlanStatus,
    pub confirmed_by: Option<String>,
    pub mark_confirmed: bool,
    pub mark_discarded: bool,
}

#[derive(Clone)]
pub struct PlanRepository {
    pool: StorePool,
}

impl PlanRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &StorePool {
        &self.pool
    }

    pub async fn create(&self, input: PlanCreateInput) -> Result<ArchitecturePlan, StoreError> {
        let row = sqlx::query(
            r#"
            INSERT INTO architecture_plans (
                id,
                architecture_id,
                architecture_version_id,
                inventory_snapshot_id,
                mode,
                status,
                plan_json,
                summary_json,
                created_by,
                expires_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING *
            "#,
        )
        .bind(input.id.as_str())
        .bind(input.architecture_id.as_str())
        .bind(input.architecture_version_id.as_str())
        .bind(input.inventory_snapshot_id.as_ref().map(|s| s.as_str()))
        .bind(input.mode.as_str())
        .bind(input.status.as_str())
        .bind(&input.plan_json)
        .bind(&input.summary_json)
        .bind(&input.created_by)
        .bind(format_ts(input.expires_at))
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

        row_to_plan(&row)
    }

    pub async fn get(&self, id: &ArchitecturePlanId) -> Result<ArchitecturePlan, StoreError> {
        let row = sqlx::query(r#"SELECT * FROM architecture_plans WHERE id = $1"#)
            .bind(id.as_str())
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| StoreError::NotFound {
                entity: ENTITY,
                id: id.to_string(),
            })?;
        row_to_plan(&row)
    }

    pub async fn list_for_architecture(
        &self,
        architecture_id: &ArchitectureId,
    ) -> Result<Vec<ArchitecturePlan>, StoreError> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM architecture_plans
            WHERE architecture_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(architecture_id.as_str())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_plan).collect()
    }

    pub async fn update_status(
        &self,
        input: PlanStatusUpdateInput,
    ) -> Result<ArchitecturePlan, StoreError> {
        let result = sqlx::query(
            r#"
            UPDATE architecture_plans SET
                status = $2,
                confirmed_by = COALESCE($3, confirmed_by),
                confirmed_at = CASE WHEN $4 = 1 THEN strftime('%Y-%m-%dT%H:%M:%SZ','now') ELSE confirmed_at END,
                discarded_at = CASE WHEN $5 = 1 THEN strftime('%Y-%m-%dT%H:%M:%SZ','now') ELSE discarded_at END
            WHERE id = $1
            "#,
        )
        .bind(input.id.as_str())
        .bind(input.status.as_str())
        .bind(&input.confirmed_by)
        .bind(if input.mark_confirmed { 1_i64 } else { 0_i64 })
        .bind(if input.mark_discarded { 1_i64 } else { 0_i64 })
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound {
                entity: ENTITY,
                id: input.id.to_string(),
            });
        }

        self.get(&input.id).await
    }

    /// Stub: generating a plan from a (version, inventory) pair is a
    /// Phase 1+ concern. Persistence callers should use [`Self::create`]
    /// directly with the precomputed plan JSON.
    pub async fn generate(&self) -> Result<ArchitecturePlan, StoreError> {
        Err(StoreError::NotImplemented {
            reason: "PlanRepository::generate is a Phase 1+ concern",
        })
    }
}

fn parse_mode(s: &str) -> Result<PlanMode, StoreError> {
    match s {
        "dry_run" => Ok(PlanMode::DryRun),
        "confirm" => Ok(PlanMode::Confirm),
        other => Err(StoreError::InvalidConfiguration {
            reason: format!("unrecognized plan mode: {other}"),
        }),
    }
}

fn parse_plan_status(s: &str) -> Result<PlanStatus, StoreError> {
    match s {
        "draft" => Ok(PlanStatus::Draft),
        "failed_validation" => Ok(PlanStatus::FailedValidation),
        "requires_confirmation" => Ok(PlanStatus::RequiresConfirmation),
        "ready_to_apply" => Ok(PlanStatus::ReadyToApply),
        "applying" => Ok(PlanStatus::Applying),
        "applied" => Ok(PlanStatus::Applied),
        "failed" => Ok(PlanStatus::Failed),
        "expired" => Ok(PlanStatus::Expired),
        "discarded" => Ok(PlanStatus::Discarded),
        other => Err(StoreError::InvalidConfiguration {
            reason: format!("unrecognized plan status: {other}"),
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
                reason: format!("invalid id stored in plan row: {err}"),
            }),
    }
}

fn row_to_plan(row: &sqlx::sqlite::SqliteRow) -> Result<ArchitecturePlan, StoreError> {
    let id_str: String = row.try_get("id")?;
    let id = ArchitecturePlanId::new(id_str).map_err(|err| {
        StoreError::InvalidConfiguration {
            reason: format!("invalid id in plan row: {err}"),
        }
    })?;
    let arch_id_str: String = row.try_get("architecture_id")?;
    let architecture_id = ArchitectureId::new(arch_id_str).map_err(|err| {
        StoreError::InvalidConfiguration {
            reason: format!("invalid architecture_id in plan row: {err}"),
        }
    })?;
    let version_id_str: String = row.try_get("architecture_version_id")?;
    let architecture_version_id = ArchitectureVersionId::new(version_id_str).map_err(|err| {
        StoreError::InvalidConfiguration {
            reason: format!("invalid architecture_version_id in plan row: {err}"),
        }
    })?;

    let mode_str: String = row.try_get("mode")?;
    let status_str: String = row.try_get("status")?;
    let created_at: String = row.try_get("created_at")?;
    let expires_at: String = row.try_get("expires_at")?;
    let confirmed_at: Option<String> = row.try_get("confirmed_at")?;
    let discarded_at: Option<String> = row.try_get("discarded_at")?;

    Ok(ArchitecturePlan {
        id,
        architecture_id,
        architecture_version_id,
        inventory_snapshot_id: opt_id(
            row.try_get("inventory_snapshot_id")?,
            InventorySnapshotId::new,
        )?,
        mode: parse_mode(&mode_str)?,
        status: parse_plan_status(&status_str)?,
        plan_json: row.try_get("plan_json")?,
        summary_json: row.try_get("summary_json")?,
        created_by: row.try_get("created_by")?,
        created_at: parse_ts(&created_at, "created_at")?,
        expires_at: parse_ts(&expires_at, "expires_at")?,
        confirmed_at: parse_ts_opt(confirmed_at.as_deref(), "confirmed_at")?,
        confirmed_by: row.try_get("confirmed_by")?,
        discarded_at: parse_ts_opt(discarded_at.as_deref(), "discarded_at")?,
    })
}
