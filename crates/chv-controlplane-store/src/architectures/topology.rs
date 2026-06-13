//! Topology repository — CRUD with optimistic concurrency and soft delete.

use crate::architectures::{format_ts, parse_ts, parse_ts_opt};
use crate::{StoreError, StorePool};
use chv_controlplane_types::architecture::{
    ArchitectureId, ArchitectureStatus, ArchitectureTopology, ArchitectureVersionId,
    FleetCheckStatus, ValidationStatus,
};
use chv_controlplane_types::domain::IdentifierError;
use sqlx::Row;

const ENTITY: &str = "architecture_topology";

#[derive(Clone, Debug)]
pub struct TopologyCreateInput {
    pub id: ArchitectureId,
    pub name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub environment: Option<String>,
    pub status: ArchitectureStatus,
    pub owner_user_id: Option<String>,
    pub design_graph_json: Option<String>,
    pub latest_yaml: Option<String>,
}

#[derive(Clone, Debug)]
pub struct TopologyUpdateInput {
    pub id: ArchitectureId,
    /// Version the caller read; update fails with [`StoreError::StaleVersion`]
    /// when the stored version no longer matches.
    pub expected_version: i64,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub environment: Option<String>,
    pub status: Option<ArchitectureStatus>,
    pub design_graph_json: Option<String>,
    pub latest_yaml: Option<String>,
    pub latest_version_id: Option<ArchitectureVersionId>,
    pub last_validation_status: Option<ValidationStatus>,
    pub last_fleet_check_status: Option<FleetCheckStatus>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TopologyListFilter {
    pub include_archived: bool,
}

#[derive(Clone)]
pub struct TopologyRepository {
    pool: StorePool,
}

impl TopologyRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &StorePool {
        &self.pool
    }

    pub async fn create(
        &self,
        input: TopologyCreateInput,
    ) -> Result<ArchitectureTopology, StoreError> {
        let result = sqlx::query(
            r#"
            INSERT INTO architecture_topologies (
                id,
                name,
                display_name,
                description,
                environment,
                status,
                owner_user_id,
                design_graph_json,
                latest_yaml,
                version_number
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 1)
            RETURNING *
            "#,
        )
        .bind(input.id.as_str())
        .bind(&input.name)
        .bind(&input.display_name)
        .bind(&input.description)
        .bind(&input.environment)
        .bind(input.status.as_str())
        .bind(&input.owner_user_id)
        .bind(&input.design_graph_json)
        .bind(&input.latest_yaml)
        .fetch_one(&self.pool)
        .await;

        match result {
            Ok(row) => row_to_topology(&row),
            Err(err) => Err(map_create_error(err, &input.name)),
        }
    }

    pub async fn get(&self, id: &ArchitectureId) -> Result<ArchitectureTopology, StoreError> {
        let row = sqlx::query(r#"SELECT * FROM architecture_topologies WHERE id = $1"#)
            .bind(id.as_str())
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| StoreError::NotFound {
                entity: ENTITY,
                id: id.to_string(),
            })?;

        row_to_topology(&row)
    }

    pub async fn list(
        &self,
        filter: TopologyListFilter,
    ) -> Result<Vec<ArchitectureTopology>, StoreError> {
        let rows = if filter.include_archived {
            sqlx::query(r#"SELECT * FROM architecture_topologies ORDER BY created_at DESC, id ASC"#)
                .fetch_all(&self.pool)
                .await?
        } else {
            sqlx::query(
                r#"
                SELECT * FROM architecture_topologies
                WHERE archived_at IS NULL
                ORDER BY created_at DESC, id ASC
                "#,
            )
            .fetch_all(&self.pool)
            .await?
        };

        rows.iter().map(row_to_topology).collect()
    }

    /// Update with optimistic concurrency. The query carries
    /// `WHERE version_number = expected_version`; on zero rows affected we
    /// distinguish "version mismatch" from "missing/archived row" with a
    /// follow-up SELECT executed *in the same transaction* and return either
    /// [`StoreError::StaleVersion`] or [`StoreError::NotFound`]. Running both
    /// statements in one transaction closes the TOCTOU race against a
    /// concurrent archive that could otherwise turn a stale-version error
    /// into a misleading not-found.
    pub async fn update(
        &self,
        input: TopologyUpdateInput,
    ) -> Result<ArchitectureTopology, StoreError> {
        let mut tx = self.pool.begin().await?;

        let result = sqlx::query(
            r#"
            UPDATE architecture_topologies SET
                display_name = COALESCE($2, display_name),
                description = COALESCE($3, description),
                environment = COALESCE($4, environment),
                status = COALESCE($5, status),
                design_graph_json = COALESCE($6, design_graph_json),
                latest_yaml = COALESCE($7, latest_yaml),
                latest_version_id = COALESCE($8, latest_version_id),
                last_validation_status = COALESCE($9, last_validation_status),
                last_fleet_check_status = COALESCE($10, last_fleet_check_status),
                version_number = version_number + 1,
                updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
            WHERE id = $1 AND version_number = $11 AND archived_at IS NULL
            "#,
        )
        .bind(input.id.as_str())
        .bind(&input.display_name)
        .bind(&input.description)
        .bind(&input.environment)
        .bind(input.status.map(|s| s.as_str()))
        .bind(&input.design_graph_json)
        .bind(&input.latest_yaml)
        .bind(input.latest_version_id.as_ref().map(|v| v.as_str()))
        .bind(input.last_validation_status.map(|s| s.as_str()))
        .bind(input.last_fleet_check_status.map(|s| s.as_str()))
        .bind(input.expected_version)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            let probe = fetch_version_and_archived_in_tx(&mut tx, &input.id).await?;
            // We ran no UPDATE; commit-or-rollback are equivalent. Rolling
            // back keeps our intent explicit ("this transaction made no
            // change").
            tx.rollback().await?;
            return Err(stale_or_not_found(probe, &input.id, input.expected_version));
        }

        // Re-SELECT the freshly-updated row inside the same transaction so
        // we return a consistent view that cannot interleave with another
        // writer.
        let row = sqlx::query(r#"SELECT * FROM architecture_topologies WHERE id = $1"#)
            .bind(input.id.as_str())
            .fetch_one(&mut *tx)
            .await?;
        let topology = row_to_topology(&row)?;
        tx.commit().await?;
        Ok(topology)
    }

    /// Soft-delete via `archived_at = now()` with optimistic concurrency. The
    /// caller passes the version they read; concurrent updates that bumped
    /// the row are reported as [`StoreError::StaleVersion`]. An already-archived
    /// row (regardless of version) is reported as [`StoreError::NotFound`] so
    /// archive remains idempotent at the routing layer.
    pub async fn archive(
        &self,
        id: &ArchitectureId,
        expected_version: i64,
    ) -> Result<ArchitectureTopology, StoreError> {
        let mut tx = self.pool.begin().await?;

        let result = sqlx::query(
            r#"
            UPDATE architecture_topologies SET
                archived_at = strftime('%Y-%m-%dT%H:%M:%SZ','now'),
                status = 'archived',
                updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now'),
                version_number = version_number + 1
            WHERE id = $1 AND archived_at IS NULL AND version_number = $2
            "#,
        )
        .bind(id.as_str())
        .bind(expected_version)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            let probe = fetch_version_and_archived_in_tx(&mut tx, id).await?;
            tx.rollback().await?;
            return Err(stale_or_not_found(probe, id, expected_version));
        }

        let row = sqlx::query(r#"SELECT * FROM architecture_topologies WHERE id = $1"#)
            .bind(id.as_str())
            .fetch_one(&mut *tx)
            .await?;
        let topology = row_to_topology(&row)?;
        tx.commit().await?;
        Ok(topology)
    }
}

/// Probe the current `(version_number, archived)` state of a topology row
/// inside an open transaction so callers can disambiguate "missing", "already
/// archived", and "version mismatch" without racing a concurrent writer.
async fn fetch_version_and_archived_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: &ArchitectureId,
) -> Result<Option<(i64, bool)>, StoreError> {
    let row = sqlx::query(
        r#"SELECT version_number, archived_at FROM architecture_topologies WHERE id = $1"#,
    )
    .bind(id.as_str())
    .fetch_optional(&mut **tx)
    .await?;

    match row {
        None => Ok(None),
        Some(row) => {
            let version: i64 = row.try_get("version_number")?;
            let archived_at: Option<String> = row.try_get("archived_at")?;
            Ok(Some((version, archived_at.is_some())))
        }
    }
}

/// Translate the result of [`fetch_version_and_archived_in_tx`] into the
/// canonical store error for the failed-update path:
///
/// - row missing → `NotFound`
/// - row archived → `NotFound` (already-archived is treated as gone)
/// - row not archived, version mismatch → `StaleVersion`
fn stale_or_not_found(
    probe: Option<(i64, bool)>,
    id: &ArchitectureId,
    expected: i64,
) -> StoreError {
    match probe {
        None => StoreError::NotFound {
            entity: ENTITY,
            id: id.to_string(),
        },
        Some((_, true)) => StoreError::NotFound {
            entity: ENTITY,
            id: id.to_string(),
        },
        Some((current, false)) => StoreError::StaleVersion {
            entity: ENTITY,
            id: id.to_string(),
            current,
            expected,
        },
    }
}

/// Map a sqlx error from the create path into [`StoreError`]. Surfaces UNIQUE
/// violations (duplicate `name`) as [`StoreError::Conflict`] so the BFF can
/// answer 409 instead of 500.
fn map_create_error(err: sqlx::Error, name: &str) -> StoreError {
    if let sqlx::Error::Database(ref db_err) = err {
        if db_err.is_unique_violation() {
            return StoreError::Conflict {
                entity: ENTITY,
                id: name.to_string(),
                reason: "name already exists",
            };
        }
    }
    StoreError::Database(err)
}

fn opt_status_to_validation(s: Option<String>) -> Result<Option<ValidationStatus>, StoreError> {
    match s.as_deref() {
        None => Ok(None),
        Some("unknown") => Ok(Some(ValidationStatus::Unknown)),
        Some("passed") => Ok(Some(ValidationStatus::Passed)),
        Some("failed") => Ok(Some(ValidationStatus::Failed)),
        Some(other) => Err(StoreError::InvalidConfiguration {
            reason: format!("unrecognized validation status: {other}"),
        }),
    }
}

fn opt_status_to_fleet(s: Option<String>) -> Result<Option<FleetCheckStatus>, StoreError> {
    match s.as_deref() {
        None => Ok(None),
        Some("unknown") => Ok(Some(FleetCheckStatus::Unknown)),
        Some("passed") => Ok(Some(FleetCheckStatus::Passed)),
        Some("failed") => Ok(Some(FleetCheckStatus::Failed)),
        Some(other) => Err(StoreError::InvalidConfiguration {
            reason: format!("unrecognized fleet-check status: {other}"),
        }),
    }
}

fn parse_status(s: &str) -> Result<ArchitectureStatus, StoreError> {
    match s {
        "draft" => Ok(ArchitectureStatus::Draft),
        "valid" => Ok(ArchitectureStatus::Valid),
        "invalid" => Ok(ArchitectureStatus::Invalid),
        "planned" => Ok(ArchitectureStatus::Planned),
        "applying" => Ok(ArchitectureStatus::Applying),
        "applied" => Ok(ArchitectureStatus::Applied),
        "drifted" => Ok(ArchitectureStatus::Drifted),
        "failed" => Ok(ArchitectureStatus::Failed),
        "archived" => Ok(ArchitectureStatus::Archived),
        other => Err(StoreError::InvalidConfiguration {
            reason: format!("unrecognized architecture status: {other}"),
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
                reason: format!("invalid id stored in topology row: {err}"),
            }),
    }
}

fn row_to_topology(row: &sqlx::sqlite::SqliteRow) -> Result<ArchitectureTopology, StoreError> {
    let id_str: String = row.try_get("id")?;
    let id = ArchitectureId::new(id_str).map_err(|err| StoreError::InvalidConfiguration {
        reason: format!("invalid id in topology row: {err}"),
    })?;
    let status_str: String = row.try_get("status")?;
    let created_at: String = row.try_get("created_at")?;
    let updated_at: String = row.try_get("updated_at")?;
    let archived_at: Option<String> = row.try_get("archived_at")?;

    Ok(ArchitectureTopology {
        id,
        name: row.try_get("name")?,
        display_name: row.try_get("display_name")?,
        description: row.try_get("description")?,
        environment: row.try_get("environment")?,
        status: parse_status(&status_str)?,
        owner_user_id: row.try_get("owner_user_id")?,
        design_graph_json: row.try_get("design_graph_json")?,
        latest_yaml: row.try_get("latest_yaml")?,
        latest_version_id: opt_id(
            row.try_get("latest_version_id")?,
            ArchitectureVersionId::new,
        )?,
        last_validation_status: opt_status_to_validation(row.try_get("last_validation_status")?)?,
        last_fleet_check_status: opt_status_to_fleet(row.try_get("last_fleet_check_status")?)?,
        last_plan_id: row.try_get("last_plan_id")?,
        last_apply_run_id: row.try_get("last_apply_run_id")?,
        last_apply_task_id: row.try_get("last_apply_task_id")?,
        last_drift_status: row.try_get("last_drift_status")?,
        version_number: row.try_get("version_number")?,
        archived_at: parse_ts_opt(archived_at.as_deref(), "archived_at")?,
        created_at: parse_ts(&created_at, "created_at")?,
        updated_at: parse_ts(&updated_at, "updated_at")?,
    })
}

// Re-exported for tests in sibling modules.
#[allow(dead_code)]
pub(crate) fn now_text() -> String {
    format_ts(chrono::Utc::now())
}
