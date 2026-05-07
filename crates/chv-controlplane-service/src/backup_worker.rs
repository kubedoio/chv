use crate::node_client_pool::NodeClientPool;
use chv_controlplane_store::{
    BackupJobCreateInput, BackupJobStatusUpdateInput, BackupRepository, StorePool,
};
use chv_errors::ChvError;
use cron::Schedule;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use tracing::{error, info, warn};

const MAX_RETRIES: i64 = 3;

/// Background worker that schedules backup jobs from cron expressions
/// and executes pending backup jobs against node agents.
pub struct BackupWorker {
    pool: StorePool,
    backup_repo: BackupRepository,
    agent_socket_pattern: String,
    tick_interval: Duration,
    node_client_pool: NodeClientPool,
}

impl BackupWorker {
    pub fn new(
        pool: StorePool,
        backup_repo: BackupRepository,
        agent_socket_pattern: String,
        node_client_pool: NodeClientPool,
    ) -> Self {
        Self {
            pool,
            backup_repo,
            agent_socket_pattern,
            tick_interval: Duration::from_secs(30),
            node_client_pool,
        }
    }

    pub async fn run(self, mut shutdown_rx: tokio::sync::watch::Receiver<()>) {
        info!("backup worker starting");
        let mut interval = tokio::time::interval(self.tick_interval);
        let mut tick_count: u64 = 0;
        loop {
            tokio::select! {
                _ = interval.tick() => {}
                _ = shutdown_rx.changed() => {
                    info!("backup worker shutting down");
                    break;
                }
            }
            tick_count += 1;
            if let Err(e) = self.tick(tick_count).await {
                warn!(error = %e, "backup worker tick failed");
            }
        }
    }

    async fn tick(&self, tick_count: u64) -> Result<(), ChvError> {
        // Executor runs every tick (30s)
        if let Err(e) = self.run_executor().await {
            warn!(error = %e, "backup executor failed");
        }

        // Scheduler runs every 2nd tick (60s)
        if tick_count.is_multiple_of(2) {
            if let Err(e) = self.run_scheduler().await {
                warn!(error = %e, "backup scheduler failed");
            }
        }

        Ok(())
    }

    async fn run_scheduler(&self) -> Result<(), ChvError> {
        let schedules = self
            .backup_repo
            .list_enabled_schedules()
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("failed to list enabled backup schedules: {e}"),
            })?;

        let now = chrono::Utc::now();

        for schedule in schedules {
            if let Err(e) = self.process_schedule(&schedule, now).await {
                warn!(
                    schedule_id = %schedule.schedule_id,
                    error = %e,
                    "failed to process backup schedule"
                );
            }
        }

        Ok(())
    }

    async fn process_schedule(
        &self,
        schedule: &chv_controlplane_store::BackupScheduleRow,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), ChvError> {
        let cron =
            Schedule::from_str(&schedule.cron_expression).map_err(|e| ChvError::Internal {
                reason: format!(
                    "invalid cron expression '{}': {e}",
                    schedule.cron_expression
                ),
            })?;

        let last_run = schedule
            .last_run_at
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        let reference_time = last_run.unwrap_or_else(|| {
            schedule
                .created_at
                .parse::<chrono::DateTime<chrono::Utc>>()
                .unwrap_or(now)
        });

        // Find the next occurrence after the reference time
        let next_due = cron.after(&reference_time).next();

        if let Some(due_time) = next_due {
            if due_time <= now {
                // Create a pending backup job
                let input = BackupJobCreateInput {
                    vm_id: schedule.vm_id.clone(),
                    volume_id: schedule.volume_id.clone(),
                    schedule_id: Some(schedule.schedule_id.clone()),
                    status: "Pending".into(),
                    backup_type: "full".into(),
                    target_path: schedule.destination.clone(),
                    storage_backend: None,
                    started_at: None,
                    completed_at: None,
                    error_message: None,
                    size_bytes: None,
                };

                let job_id =
                    self.backup_repo
                        .create_job(&input)
                        .await
                        .map_err(|e| ChvError::Internal {
                            reason: format!("failed to create backup job from schedule: {e}"),
                        })?;

                info!(
                    schedule_id = %schedule.schedule_id,
                    job_id = %job_id,
                    "created scheduled backup job"
                );

                // Update last_run_at
                let now_str = now.to_rfc3339();
                self.backup_repo
                    .update_schedule_last_run(&schedule.schedule_id, &now_str)
                    .await
                    .map_err(|e| ChvError::Internal {
                        reason: format!("failed to update schedule last_run_at: {e}"),
                    })?;

                // Enforce retention: prune old completed jobs beyond retention_count
                if schedule.retention_count > 0 {
                    match self
                        .backup_repo
                        .prune_old_jobs_for_schedule(&schedule.schedule_id, schedule.retention_count)
                        .await
                    {
                        Ok(pruned) if pruned > 0 => {
                            info!(
                                schedule_id = %schedule.schedule_id,
                                vm_id = %schedule.vm_id,
                                pruned_count = pruned,
                                retention_count = schedule.retention_count,
                                "pruned old backup jobs"
                            );
                        }
                        Err(e) => {
                            warn!(
                                schedule_id = %schedule.schedule_id,
                                error = %e,
                                "failed to prune old backup jobs"
                            );
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(())
    }

    async fn run_executor(&self) -> Result<(), ChvError> {
        // Atomically claim pending jobs by setting status to Running in a single query.
        // This prevents double-execution if multiple backup workers run concurrently.
        let jobs: Vec<chv_controlplane_store::BackupJobRow> = sqlx::query_as(
            "UPDATE backup_jobs SET status = 'Running', started_at = strftime('%Y-%m-%dT%H:%M:%SZ','now') \
             WHERE job_id IN ( \
                 SELECT job_id FROM backup_jobs WHERE status = 'Pending' \
                 ORDER BY created_at ASC LIMIT 10 \
             ) \
             RETURNING job_id, vm_id, volume_id, status, backup_type, target_path, \
                       storage_backend, created_at, started_at, completed_at, \
                       error_message, size_bytes, retry_count, next_retry_at",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to claim pending backup jobs: {e}"),
        })?;

        let now_str = chrono::Utc::now().to_rfc3339();
        let retryable: Vec<chv_controlplane_store::BackupJobRow> = sqlx::query_as(
            "UPDATE backup_jobs SET status = 'Running', started_at = strftime('%Y-%m-%dT%H:%M:%SZ','now') \
             WHERE job_id IN ( \
                 SELECT job_id FROM backup_jobs \
                 WHERE status = 'RetryPending' AND next_retry_at <= ? \
                 ORDER BY next_retry_at ASC LIMIT 10 \
             ) \
             RETURNING job_id, vm_id, volume_id, status, backup_type, target_path, \
                       storage_backend, created_at, started_at, completed_at, \
                       error_message, size_bytes, retry_count, next_retry_at",
        )
        .bind(&now_str)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to claim retryable backup jobs: {e}"),
        })?;

        for job in jobs.iter().chain(retryable.iter()) {
            if let Err(e) = self.execute_job(job).await {
                warn!(
                    job_id = %job.job_id,
                    error = %e,
                    "failed to execute backup job"
                );

                // Check if we can retry
                let new_retry_count = job.retry_count + 1;
                if new_retry_count <= MAX_RETRIES {
                    let backoff_secs = 60i64 * (1 << (new_retry_count - 1)); // 60s, 120s, 240s
                    let next_retry = chrono::Utc::now() + chrono::Duration::seconds(backoff_secs);
                    if let Err(retry_err) = self
                        .backup_repo
                        .mark_for_retry(
                            &job.job_id,
                            new_retry_count,
                            &next_retry.to_rfc3339(),
                            &e.to_string(),
                        )
                        .await
                    {
                        error!(
                            job_id = %job.job_id,
                            error = %retry_err,
                            "failed to mark job for retry"
                        );
                    } else {
                        info!(
                            job_id = %job.job_id,
                            retry = new_retry_count,
                            next_retry_at = %next_retry.to_rfc3339(),
                            "backup job scheduled for retry"
                        );
                    }
                } else {
                    // Permanently failed
                    let now = chrono::Utc::now().to_rfc3339();
                    if let Err(update_err) = self
                        .backup_repo
                        .update_job_status(&BackupJobStatusUpdateInput {
                            job_id: job.job_id.clone(),
                            status: "Failed".into(),
                            started_at: Some(now.clone()),
                            completed_at: Some(now),
                            error_message: Some(format!(
                                "permanently failed after {} retries: {}",
                                MAX_RETRIES, e
                            )),
                            size_bytes: None,
                        })
                        .await
                    {
                        error!(
                            job_id = %job.job_id,
                            error = %update_err,
                            "failed to update job status after exhausting retries"
                        );
                    }
                }
            }
        }

        Ok(())
    }

    async fn execute_job(
        &self,
        job: &chv_controlplane_store::BackupJobRow,
    ) -> Result<(), ChvError> {
        // Job already claimed as Running by the atomic UPDATE...RETURNING in run_executor.

        // Find the VM's node_id and generation in a single query
        let row: Option<(String, i64)> = sqlx::query_as(
            "SELECT v.node_id, COALESCE(vos.observed_generation, 1) as generation \
             FROM vms v \
             LEFT JOIN vm_observed_state vos ON v.vm_id = vos.vm_id \
             WHERE v.vm_id = ?",
        )
        .bind(&job.vm_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to query vm node_id and generation: {e}"),
        })?;

        let (node_id, generation) = row.ok_or_else(|| ChvError::InvalidArgument {
            field: "node_id".to_string(),
            reason: format!("vm {} has no target node", job.vm_id),
        })?;

        let socket_path = self.resolve_agent_socket(&node_id);
        let mut client = self
            .node_client_pool
            .get_or_connect(&node_id, &socket_path)
            .await?;
        let generation_str = generation.to_string();
        let snapshot_name = format!("backup-{}", job.job_id);

        let ack = if let Some(volume_id) = &job.volume_id {
            client
                .snapshot_volume(
                    &node_id,
                    volume_id,
                    &generation_str,
                    &snapshot_name,
                    &job.job_id,
                    None,
                )
                .await
        } else {
            let destination = job.target_path.as_deref().unwrap_or("");
            client
                .snapshot_vm(
                    &node_id,
                    &job.vm_id,
                    &generation_str,
                    destination,
                    &job.job_id,
                    None,
                )
                .await
        };

        match ack {
            Ok(result) => {
                let status = result
                    .result
                    .as_ref()
                    .map(|r| r.status.as_str())
                    .unwrap_or("OK");
                let accepted = status.eq_ignore_ascii_case("ok");
                let final_status = if accepted { "Succeeded" } else { "Failed" };
                let error_message = if accepted {
                    None
                } else {
                    result.result.map(|r| r.human_summary)
                };

                let now = chrono::Utc::now().to_rfc3339();
                self.backup_repo
                    .update_job_status(&BackupJobStatusUpdateInput {
                        job_id: job.job_id.clone(),
                        status: final_status.into(),
                        started_at: Some(now.clone()),
                        completed_at: Some(now),
                        error_message,
                        size_bytes: None,
                    })
                    .await
                    .map_err(|e| ChvError::Internal {
                        reason: format!("failed to update job status after execution: {e}"),
                    })?;

                info!(
                    job_id = %job.job_id,
                    vm_id = %job.vm_id,
                    node_id = %node_id,
                    "backup job executed"
                );
                Ok(())
            }
            Err(e) => {
                if matches!(e, ChvError::BackendUnavailable { .. }) {
                    self.node_client_pool.evict(&node_id);
                }
                Err(e)
            }
        }
    }

    fn resolve_agent_socket(&self, node_id: &str) -> PathBuf {
        if self.agent_socket_pattern.contains("{node_id}") {
            PathBuf::from(self.agent_socket_pattern.replace("{node_id}", node_id))
        } else {
            PathBuf::from(&self.agent_socket_pattern)
        }
    }
}
