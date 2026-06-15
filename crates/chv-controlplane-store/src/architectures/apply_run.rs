//! Apply-run repository — CRUD for architecture_apply_runs.

use crate::architectures::{format_ts, parse_ts, parse_ts_opt};
use crate::{StoreError, StorePool};
use chrono::{DateTime, Utc};
use chv_controlplane_types::architecture::{
    ArchitectureApplyRun, ArchitectureApplyRunId, ArchitectureId, ArchitecturePlanId,
    ArchitectureVersionId, RunStatus,
};
use chv_controlplane_types::domain::IdentifierError;
use sqlx::Row;

const ENTITY: &str = "architecture_apply_run";

#[derive(Clone, Debug)]
pub struct ApplyRunCreateInput {
    pub id: ArchitectureApplyRunId,
    pub architecture_id: ArchitectureId,
    pub architecture_version_id: ArchitectureVersionId,
    pub plan_id: Option<ArchitecturePlanId>,
    pub task_id: Option<String>,
    pub status: RunStatus,
    pub requested_by: Option<String>,
    /// Wall-clock instant the run started doing work. Persisted at
    /// create time when the caller already has a clock reading; lets the
    /// failure path always render a non-NULL `started_at` even when the
    /// run never reached `Running`. Pass `None` for runs that genuinely
    /// have not started yet (orchestrator-driven create).
    pub started_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct ApplyRunUpdateInput {
    pub id: ArchitectureApplyRunId,
    pub status: Option<RunStatus>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub task_id: Option<String>,
    pub result_json: Option<String>,
    pub logs_ref: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Clone)]
pub struct ApplyRunRepository {
    pool: StorePool,
}

impl ApplyRunRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &StorePool {
        &self.pool
    }

    pub async fn create(
        &self,
        input: ApplyRunCreateInput,
    ) -> Result<ArchitectureApplyRun, StoreError> {
        let row = sqlx::query(
            r#"
            INSERT INTO architecture_apply_runs (
                id,
                architecture_id,
                architecture_version_id,
                plan_id,
                task_id,
                status,
                requested_by,
                started_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING *
            "#,
        )
        .bind(input.id.as_str())
        .bind(input.architecture_id.as_str())
        .bind(input.architecture_version_id.as_str())
        .bind(input.plan_id.as_ref().map(|p| p.as_str()))
        .bind(&input.task_id)
        .bind(input.status.as_str())
        .bind(&input.requested_by)
        .bind(input.started_at.map(format_ts))
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

        row_to_run(&row)
    }

    pub async fn get(
        &self,
        id: &ArchitectureApplyRunId,
    ) -> Result<ArchitectureApplyRun, StoreError> {
        let row = sqlx::query(r#"SELECT * FROM architecture_apply_runs WHERE id = $1"#)
            .bind(id.as_str())
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| StoreError::NotFound {
                entity: ENTITY,
                id: id.to_string(),
            })?;
        row_to_run(&row)
    }

    pub async fn list_for_architecture(
        &self,
        architecture_id: &ArchitectureId,
    ) -> Result<Vec<ArchitectureApplyRun>, StoreError> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM architecture_apply_runs
            WHERE architecture_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(architecture_id.as_str())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_run).collect()
    }

    pub async fn update(
        &self,
        input: ApplyRunUpdateInput,
    ) -> Result<ArchitectureApplyRun, StoreError> {
        let result = sqlx::query(
            r#"
            UPDATE architecture_apply_runs SET
                status = COALESCE($2, status),
                started_at = COALESCE($3, started_at),
                finished_at = COALESCE($4, finished_at),
                task_id = COALESCE($5, task_id),
                result_json = COALESCE($6, result_json),
                logs_ref = COALESCE($7, logs_ref),
                error_message = COALESCE($8, error_message),
                updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
            WHERE id = $1
            "#,
        )
        .bind(input.id.as_str())
        .bind(input.status.map(|s| s.as_str()))
        .bind(input.started_at.map(format_ts))
        .bind(input.finished_at.map(format_ts))
        .bind(&input.task_id)
        .bind(&input.result_json)
        .bind(&input.logs_ref)
        .bind(&input.error_message)
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

    /// Apply orchestration is a Phase 1+ concern.
    pub async fn execute_plan(&self) -> Result<ArchitectureApplyRun, StoreError> {
        Err(StoreError::NotImplemented {
            reason: "ApplyRunRepository::execute_plan is a Phase 1+ concern",
        })
    }
}

fn parse_run_status(s: &str) -> Result<RunStatus, StoreError> {
    match s {
        "queued" => Ok(RunStatus::Queued),
        "running" => Ok(RunStatus::Running),
        "succeeded" => Ok(RunStatus::Succeeded),
        "partially_failed" => Ok(RunStatus::PartiallyFailed),
        "failed" => Ok(RunStatus::Failed),
        "cancelled" => Ok(RunStatus::Cancelled),
        other => Err(StoreError::InvalidConfiguration {
            reason: format!("unrecognized run status: {other}"),
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
                reason: format!("invalid id stored in apply_run row: {err}"),
            }),
    }
}

fn row_to_run(row: &sqlx::sqlite::SqliteRow) -> Result<ArchitectureApplyRun, StoreError> {
    let id_str: String = row.try_get("id")?;
    let id =
        ArchitectureApplyRunId::new(id_str).map_err(|err| StoreError::InvalidConfiguration {
            reason: format!("invalid id in apply_run row: {err}"),
        })?;
    let arch_id_str: String = row.try_get("architecture_id")?;
    let architecture_id =
        ArchitectureId::new(arch_id_str).map_err(|err| StoreError::InvalidConfiguration {
            reason: format!("invalid architecture_id in apply_run row: {err}"),
        })?;
    let version_id_str: String = row.try_get("architecture_version_id")?;
    let architecture_version_id = ArchitectureVersionId::new(version_id_str).map_err(|err| {
        StoreError::InvalidConfiguration {
            reason: format!("invalid architecture_version_id in apply_run row: {err}"),
        }
    })?;

    let status_str: String = row.try_get("status")?;
    let started_at: Option<String> = row.try_get("started_at")?;
    let finished_at: Option<String> = row.try_get("finished_at")?;
    let created_at: String = row.try_get("created_at")?;
    let updated_at: String = row.try_get("updated_at")?;

    Ok(ArchitectureApplyRun {
        id,
        architecture_id,
        architecture_version_id,
        plan_id: opt_id(row.try_get("plan_id")?, ArchitecturePlanId::new)?,
        task_id: row.try_get("task_id")?,
        status: parse_run_status(&status_str)?,
        started_at: parse_ts_opt(started_at.as_deref(), "started_at")?,
        finished_at: parse_ts_opt(finished_at.as_deref(), "finished_at")?,
        requested_by: row.try_get("requested_by")?,
        result_json: row.try_get("result_json")?,
        logs_ref: row.try_get("logs_ref")?,
        error_message: row.try_get("error_message")?,
        created_at: parse_ts(&created_at, "created_at")?,
        updated_at: parse_ts(&updated_at, "updated_at")?,
    })
}
