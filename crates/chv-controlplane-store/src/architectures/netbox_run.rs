//! NetBox projection run repository — CRUD + claim/terminal transitions
//! for `netbox_projection_runs`.
//!
//! Status machine (one active — queued or running — run per architecture,
//! enforced by the partial unique index `netbox_projection_runs_one_active`):
//!
//! ```text
//! queued ──▶ running ──▶ succeeded
//!    │           │
//!    │           └────────▶ failed ──▶ (requeue) ──▶ queued
//! ```

use crate::architectures::{parse_ts, parse_ts_opt};
use crate::{StoreError, StorePool};
use chv_controlplane_types::architecture::{
    ArchitectureId, ArchitectureVersionId, NetboxProjectionMode, NetboxProjectionRun,
    NetboxProjectionRunId, NetboxProjectionRunStatus, NetboxProjectionTrigger,
};
use sqlx::Row;

const ENTITY: &str = "netbox_projection_run";

/// Maximum number of attempts (initial run + retries) before a failed run
/// stays `failed` for operator inspection. The component spec requires a
/// bounded attempt count without fixing the number; 5 mirrors the
/// dispatch-retry bounds used elsewhere in the control plane.
pub const MAX_ATTEMPTS: i64 = 5;

#[derive(Clone, Debug)]
pub struct NetboxProjectionRunCreateInput {
    pub id: NetboxProjectionRunId,
    pub architecture_id: ArchitectureId,
    pub architecture_version_id: ArchitectureVersionId,
    pub trigger_kind: NetboxProjectionTrigger,
    pub mode: NetboxProjectionMode,
    /// Deterministic plan from `chv-netbox-adapter`, persisted at run
    /// creation: dry-run runs carry the plan they will report; export
    /// runs carry it once computed (the PR-4 worker may also leave it
    /// `None` and persist results via `mark_succeeded` instead).
    pub plan_json: Option<String>,
    pub requested_by: Option<String>,
}

#[derive(Clone)]
pub struct NetboxProjectionRunRepository {
    pool: StorePool,
}

impl NetboxProjectionRunRepository {
    pub fn new(pool: StorePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &StorePool {
        &self.pool
    }

    /// Enqueue a run: status starts `queued`, `attempt_count = 0`.
    ///
    /// - FK violations (unknown topology or version) map to
    ///   [`StoreError::NotFound`].
    /// - A UNIQUE violation on `netbox_projection_runs_one_active` (an
    ///   active run already exists for the architecture) maps to
    ///   [`StoreError::Conflict`] so the BFF can answer 409
    ///   `NETBOX_RUN_ACTIVE`.
    pub async fn create(
        &self,
        input: NetboxProjectionRunCreateInput,
    ) -> Result<NetboxProjectionRun, StoreError> {
        let row = sqlx::query(
            r#"
            INSERT INTO netbox_projection_runs (
                id,
                architecture_id,
                architecture_version_id,
                trigger_kind,
                mode,
                status,
                plan_json,
                requested_by
            )
            VALUES ($1, $2, $3, $4, $5, 'queued', $6, $7)
            RETURNING *
            "#,
        )
        .bind(input.id.as_str())
        .bind(input.architecture_id.as_str())
        .bind(input.architecture_version_id.as_str())
        .bind(input.trigger_kind.as_str())
        .bind(input.mode.as_str())
        .bind(&input.plan_json)
        .bind(&input.requested_by)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| map_create_error(err, &input))?;

        row_to_run(&row)
    }

    pub async fn get(
        &self,
        run_id: &NetboxProjectionRunId,
    ) -> Result<Option<NetboxProjectionRun>, StoreError> {
        let row = sqlx::query(r#"SELECT * FROM netbox_projection_runs WHERE id = $1"#)
            .bind(run_id.as_str())
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(row_to_run).transpose()
    }

    /// List runs for an architecture, newest first.
    pub async fn list_by_architecture(
        &self,
        architecture_id: &ArchitectureId,
        limit: i64,
    ) -> Result<Vec<NetboxProjectionRun>, StoreError> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM netbox_projection_runs
            WHERE architecture_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(architecture_id.as_str())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_run).collect()
    }

    /// Atomically claim the oldest queued run for an architecture
    /// (`queued → running` in a single statement, mirroring the backup
    /// worker's claim) so concurrent workers cannot double-claim.
    /// Returns `None` when nothing is queued.
    pub async fn claim_next_queued(
        &self,
        architecture_id: &ArchitectureId,
    ) -> Result<Option<NetboxProjectionRun>, StoreError> {
        let row = sqlx::query(
            r#"
            UPDATE netbox_projection_runs SET
                status = 'running',
                started_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
            WHERE id = (
                SELECT id FROM netbox_projection_runs
                WHERE status = 'queued' AND architecture_id = $1
                ORDER BY created_at
                LIMIT 1
            )
            RETURNING *
            "#,
        )
        .bind(architecture_id.as_str())
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(row_to_run).transpose()
    }

    /// Terminal transition `running → succeeded` with the plan results.
    ///
    /// Guarded by `status = 'running'` (compare-and-set) so a stale worker
    /// cannot overwrite a terminal state; guard failures map to
    /// [`StoreError::Conflict`] (missing runs to [`StoreError::NotFound`]).
    pub async fn mark_succeeded(
        &self,
        run_id: &NetboxProjectionRunId,
        result_json: Option<String>,
        summary_json: Option<String>,
    ) -> Result<NetboxProjectionRun, StoreError> {
        let row = sqlx::query(
            r#"
            UPDATE netbox_projection_runs SET
                status = 'succeeded',
                result_json = $2,
                summary_json = $3,
                finished_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
            WHERE id = $1 AND status = 'running'
            RETURNING *
            "#,
        )
        .bind(run_id.as_str())
        .bind(&result_json)
        .bind(&summary_json)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => row_to_run(&row),
            None => Err(self
                .guard_violation(run_id, "run is not in the running state")
                .await),
        }
    }

    /// Terminal transition `running → failed` with `attempt_count + 1`.
    ///
    /// Error messages are redacted by CALLERS (the projection worker) —
    /// the store does not scrub. Same `running` CAS guard as
    /// [`Self::mark_succeeded`].
    pub async fn mark_failed(
        &self,
        run_id: &NetboxProjectionRunId,
        error_message: Option<String>,
    ) -> Result<NetboxProjectionRun, StoreError> {
        let row = sqlx::query(
            r#"
            UPDATE netbox_projection_runs SET
                status = 'failed',
                error_message = $2,
                attempt_count = attempt_count + 1,
                finished_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
            WHERE id = $1 AND status = 'running'
            RETURNING *
            "#,
        )
        .bind(run_id.as_str())
        .bind(&error_message)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => row_to_run(&row),
            None => Err(self
                .guard_violation(run_id, "run is not in the running state")
                .await),
        }
    }

    /// Retry enqueue: `failed → queued`, keeping `attempt_count` and
    /// clearing `finished_at`. Only allowed while the run is `failed` and
    /// `attempt_count < MAX_ATTEMPTS`; guard violations map to
    /// [`StoreError::Conflict`].
    ///
    /// The flip back to `queued` re-enters the `one_active` partial
    /// unique index, so requeueing while another run is queued/running
    /// fails the UPDATE with a UNIQUE violation — mapped to
    /// [`StoreError::Conflict`] (409 at the BFF), not a raw database
    /// error.
    pub async fn requeue(
        &self,
        run_id: &NetboxProjectionRunId,
    ) -> Result<NetboxProjectionRun, StoreError> {
        let row = sqlx::query(
            r#"
            UPDATE netbox_projection_runs SET
                status = 'queued',
                finished_at = NULL
            WHERE id = $1 AND status = 'failed' AND attempt_count < $2
            RETURNING *
            "#,
        )
        .bind(run_id.as_str())
        .bind(MAX_ATTEMPTS)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| map_requeue_error(err, run_id))?;

        match row {
            Some(row) => row_to_run(&row),
            None => {
                let reason = match self.get(run_id).await? {
                    None => {
                        return Err(StoreError::NotFound {
                            entity: ENTITY,
                            id: run_id.to_string(),
                        })
                    }
                    Some(run) if run.status != NetboxProjectionRunStatus::Failed => {
                        "run is not in the failed state"
                    }
                    Some(_) => "retry attempts exhausted",
                };
                Err(StoreError::Conflict {
                    entity: ENTITY,
                    id: run_id.to_string(),
                    reason,
                })
            }
        }
    }

    /// Disambiguate a failed CAS guard: a missing run is
    /// [`StoreError::NotFound`], an existing run in another state is
    /// [`StoreError::Conflict`].
    async fn guard_violation(
        &self,
        run_id: &NetboxProjectionRunId,
        reason: &'static str,
    ) -> StoreError {
        match self.get(run_id).await {
            Ok(None) => StoreError::NotFound {
                entity: ENTITY,
                id: run_id.to_string(),
            },
            _ => StoreError::Conflict {
                entity: ENTITY,
                id: run_id.to_string(),
                reason,
            },
        }
    }
}

/// Conflict used when the `netbox_projection_runs_one_active` partial
/// unique index rejects a write: another queued/running run exists for
/// the architecture.
fn active_run_conflict(id: String) -> StoreError {
    StoreError::Conflict {
        entity: ENTITY,
        id,
        reason: "an active run already exists for this architecture",
    }
}

/// Map a sqlx error from the create path into [`StoreError`].
///
/// SQLite reports the partial-index violation as
/// `UNIQUE constraint failed: netbox_projection_runs.architecture_id`
/// (the indexed column) and a duplicate PK as `...runs.id`, so the two
/// are distinguished by message. FK violations surface as `NotFound` so
/// the BFF answers 404 instead of 500.
fn map_create_error(err: sqlx::Error, input: &NetboxProjectionRunCreateInput) -> StoreError {
    if let sqlx::Error::Database(ref db_err) = err {
        if db_err.is_unique_violation() {
            let message = db_err.message();
            if message.contains("architecture_id") {
                return active_run_conflict(input.architecture_id.to_string());
            }
            return StoreError::Conflict {
                entity: ENTITY,
                id: input.id.to_string(),
                reason: "run id already exists",
            };
        }
        if db_err.is_foreign_key_violation() {
            return StoreError::NotFound {
                entity: "architecture_topology_or_version",
                id: input.architecture_id.to_string(),
            };
        }
    }
    StoreError::Database(err)
}

/// Map a sqlx error from the requeue path into [`StoreError`].
///
/// The only expected failure is the `one_active` partial unique index:
/// the `failed → queued` flip collides with another active run for the
/// same architecture. Without this mapping the caller would see a raw
/// [`StoreError::Database`] (HTTP 500) instead of the contract's 409.
fn map_requeue_error(err: sqlx::Error, run_id: &NetboxProjectionRunId) -> StoreError {
    if let sqlx::Error::Database(ref db_err) = err {
        if db_err.is_unique_violation() {
            return active_run_conflict(run_id.to_string());
        }
    }
    StoreError::Database(err)
}

fn parse_trigger_kind(s: &str) -> Result<NetboxProjectionTrigger, StoreError> {
    match s {
        "manual" => Ok(NetboxProjectionTrigger::Manual),
        "post_apply" => Ok(NetboxProjectionTrigger::PostApply),
        other => Err(StoreError::InvalidConfiguration {
            reason: format!("unrecognized netbox projection trigger kind: {other}"),
        }),
    }
}

fn parse_mode(s: &str) -> Result<NetboxProjectionMode, StoreError> {
    match s {
        "dry_run" => Ok(NetboxProjectionMode::DryRun),
        "export" => Ok(NetboxProjectionMode::Export),
        other => Err(StoreError::InvalidConfiguration {
            reason: format!("unrecognized netbox projection mode: {other}"),
        }),
    }
}

fn parse_run_status(s: &str) -> Result<NetboxProjectionRunStatus, StoreError> {
    match s {
        "queued" => Ok(NetboxProjectionRunStatus::Queued),
        "running" => Ok(NetboxProjectionRunStatus::Running),
        "succeeded" => Ok(NetboxProjectionRunStatus::Succeeded),
        "failed" => Ok(NetboxProjectionRunStatus::Failed),
        other => Err(StoreError::InvalidConfiguration {
            reason: format!("unrecognized netbox projection run status: {other}"),
        }),
    }
}

fn row_to_run(row: &sqlx::sqlite::SqliteRow) -> Result<NetboxProjectionRun, StoreError> {
    let id_str: String = row.try_get("id")?;
    let id =
        NetboxProjectionRunId::new(id_str).map_err(|err| StoreError::InvalidConfiguration {
            reason: format!("invalid id in netbox_projection_run row: {err}"),
        })?;
    let arch_id_str: String = row.try_get("architecture_id")?;
    let architecture_id =
        ArchitectureId::new(arch_id_str).map_err(|err| StoreError::InvalidConfiguration {
            reason: format!("invalid architecture_id in netbox_projection_run row: {err}"),
        })?;
    let version_id_str: String = row.try_get("architecture_version_id")?;
    let architecture_version_id = ArchitectureVersionId::new(version_id_str).map_err(|err| {
        StoreError::InvalidConfiguration {
            reason: format!("invalid architecture_version_id in netbox_projection_run row: {err}"),
        }
    })?;

    let trigger_str: String = row.try_get("trigger_kind")?;
    let mode_str: String = row.try_get("mode")?;
    let status_str: String = row.try_get("status")?;
    let started_at: Option<String> = row.try_get("started_at")?;
    let finished_at: Option<String> = row.try_get("finished_at")?;
    let created_at: String = row.try_get("created_at")?;

    Ok(NetboxProjectionRun {
        id,
        architecture_id,
        architecture_version_id,
        trigger_kind: parse_trigger_kind(&trigger_str)?,
        mode: parse_mode(&mode_str)?,
        status: parse_run_status(&status_str)?,
        plan_json: row.try_get("plan_json")?,
        result_json: row.try_get("result_json")?,
        summary_json: row.try_get("summary_json")?,
        error_message: row.try_get("error_message")?,
        attempt_count: row.try_get("attempt_count")?,
        requested_by: row.try_get("requested_by")?,
        started_at: parse_ts_opt(started_at.as_deref(), "started_at")?,
        finished_at: parse_ts_opt(finished_at.as_deref(), "finished_at")?,
        created_at: parse_ts(&created_at, "created_at")?,
    })
}
