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
    pub fn new(pool: StorePool, backup_repo: BackupRepository, agent_socket_pattern: String, node_client_pool: NodeClientPool) -> Self {
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
        let cron = Schedule::from_str(&schedule.cron_expression).map_err(|e| ChvError::Internal {
            reason: format!("invalid cron expression '{}': {e}", schedule.cron_expression),
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
                    status: "Pending".into(),
                    backup_type: "full".into(),
                    target_path: schedule.destination.clone(),
                    storage_backend: None,
                    started_at: None,
                    completed_at: None,
                    error_message: None,
                    size_bytes: None,
                };

                let job_id = self
                    .backup_repo
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
            }
        }

        Ok(())
    }

    async fn run_executor(&self) -> Result<(), ChvError> {
        let jobs = self
            .backup_repo
            .list_pending_jobs()
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("failed to list pending backup jobs: {e}"),
            })?;

        for job in jobs {
            if let Err(e) = self.execute_job(&job).await {
                warn!(
                    job_id = %job.job_id,
                    error = %e,
                    "failed to execute backup job"
                );

                // Mark as failed
                let now = chrono::Utc::now().to_rfc3339();
                if let Err(update_err) = self
                    .backup_repo
                    .update_job_status(&BackupJobStatusUpdateInput {
                        job_id: job.job_id.clone(),
                        status: "Failed".into(),
                        started_at: Some(now.clone()),
                        completed_at: Some(now),
                        error_message: Some(e.to_string()),
                        size_bytes: None,
                    })
                    .await
                {
                    error!(
                        job_id = %job.job_id,
                        error = %update_err,
                        "failed to update job status after execution failure"
                    );
                }
            }
        }

        Ok(())
    }

    async fn execute_job(
        &self,
        job: &chv_controlplane_store::BackupJobRow,
    ) -> Result<(), ChvError> {
        // Find the VM's node_id
        let node_id: Option<String> = sqlx::query_scalar("SELECT node_id FROM vms WHERE vm_id = ?")
            .bind(&job.vm_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("failed to query vm node_id: {e}"),
            })?;

        let node_id = node_id.ok_or_else(|| ChvError::InvalidArgument {
            field: "node_id".to_string(),
            reason: format!("vm {} has no target node", job.vm_id),
        })?;

        let socket_path = self.resolve_agent_socket(&node_id);
        let mut client = self.node_client_pool.get_or_connect(&node_id, &socket_path).await?;

        let generation = "1";
        let snapshot_name = format!("backup-{}", job.job_id);

        let ack = if let Some(volume_id) = &job.volume_id {
            client
                .snapshot_volume(
                    &node_id,
                    volume_id,
                    generation,
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
                    generation,
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
                let now = chrono::Utc::now().to_rfc3339();
                self.backup_repo
                    .update_job_status(&BackupJobStatusUpdateInput {
                        job_id: job.job_id.clone(),
                        status: "Failed".into(),
                        started_at: Some(now.clone()),
                        completed_at: Some(now),
                        error_message: Some(e.to_string()),
                        size_bytes: None,
                    })
                    .await
                    .map_err(|e2| ChvError::Internal {
                        reason: format!(
                            "agent rejected backup job and status update failed: {e2}"
                        ),
                    })?;
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
