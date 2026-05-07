use crate::node_client_pool::NodeClientPool;
use chv_controlplane_store::{
    HypervisorSettingsRepository, HypervisorSettingsRow, OperationRepository,
    OperationStatusUpdateInput, StorePool,
};
use chv_controlplane_types::domain::{OperationId, OperationStatus};
use chv_errors::ChvError;
use chv_observability::{CHV_NODES_READY, CHV_OPERATION_DURATION_SECONDS, CHV_VMS_TOTAL};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{error, info, warn};

use chv_common::hypervisor::HypervisorOverrides;

const MAX_DISPATCH_RETRIES: i32 = 3;

/// Background task that polls for accepted operations and dispatches them to node agents.
pub struct Orchestrator {
    pool: StorePool,
    operation_repo: OperationRepository,
    agent_socket_pattern: String,
    kernel_path: String,
    firmware_path: String,
    tick_interval: Duration,
    node_client_pool: NodeClientPool,
}

impl Orchestrator {
    pub fn new(
        pool: StorePool,
        operation_repo: OperationRepository,
        agent_socket_pattern: String,
        kernel_path: String,
        firmware_path: String,
        node_client_pool: NodeClientPool,
    ) -> Self {
        Self {
            pool,
            operation_repo,
            agent_socket_pattern,
            kernel_path,
            firmware_path,
            tick_interval: Duration::from_secs(2),
            node_client_pool,
        }
    }

    pub async fn run(self, mut shutdown_rx: tokio::sync::watch::Receiver<()>) {
        info!("orchestrator starting");
        let mut interval = tokio::time::interval(self.tick_interval);
        loop {
            tokio::select! {
                _ = interval.tick() => {}
                _ = shutdown_rx.changed() => {
                    info!("orchestrator shutting down");
                    break;
                }
            }
            if let Err(e) = self.tick().await {
                warn!(error = %e, "orchestrator tick failed");
            }
        }
    }

    async fn tick(&self) -> Result<(), ChvError> {
        // Update ADR-009 gauges
        let vm_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vms")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);
        metrics::gauge!(CHV_VMS_TOTAL).set(vm_count as f64);

        let node_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM nodes WHERE status = 'ready'")
                .fetch_one(&self.pool)
                .await
                .unwrap_or(0);
        metrics::gauge!(CHV_NODES_READY).set(node_count as f64);

        self.reap_stuck_operations().await?;
        self.check_node_liveness().await?;

        // Atomically claim operations by marking them Running in the same query.
        // This prevents double-dispatch if tick overlaps (takes longer than interval).
        let claimed_rows = sqlx::query_as::<_, ClaimedOperationRow>(
            r#"
            UPDATE operations SET status = 'Running', updated_by = 'orchestrator'
            WHERE operation_id IN (
                SELECT o.operation_id
                FROM operations o
                WHERE o.status = 'Accepted'
                ORDER BY o.requested_at ASC
                LIMIT 10
            )
            RETURNING
                operation_id,
                operation_type,
                resource_kind,
                resource_id,
                desired_generation,
                correlation_id
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to claim accepted operations: {e}"),
        })?;

        // Also claim operations that are pending retry and whose next_retry_at has passed
        let retryable_rows = sqlx::query_as::<_, ClaimedOperationRow>(
            r#"
            UPDATE operations SET status = 'Running', updated_by = 'orchestrator'
            WHERE operation_id IN (
                SELECT o.operation_id
                FROM operations o
                WHERE o.status = 'RetryPending'
                  AND o.next_retry_at <= strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                ORDER BY o.next_retry_at ASC
                LIMIT 5
            )
            RETURNING
                operation_id,
                operation_type,
                resource_kind,
                resource_id,
                desired_generation,
                correlation_id
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to claim retryable operations: {e}"),
        })?;

        // Resolve node_id for each claimed operation
        let mut rows = Vec::with_capacity(claimed_rows.len() + retryable_rows.len());
        for claimed in claimed_rows.into_iter().chain(retryable_rows.into_iter()) {
            let node_id: Option<String> = sqlx::query_scalar(
                r#"
                SELECT COALESCE(vds.target_node_id, vol.node_id, net.node_id)
                FROM operations o
                LEFT JOIN vm_desired_state vds ON o.resource_id = vds.vm_id
                LEFT JOIN volumes vol ON o.resource_id = vol.volume_id
                LEFT JOIN networks net ON o.resource_id = net.network_id
                WHERE o.operation_id = ?
                "#,
            )
            .bind(&claimed.operation_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!(
                    "failed to resolve node_id for operation {}: {e}",
                    claimed.operation_id
                ),
            })?
            .flatten();

            rows.push(AcceptedOperationRow {
                operation_id: claimed.operation_id,
                operation_type: claimed.operation_type,
                resource_kind: claimed.resource_kind,
                resource_id: claimed.resource_id,
                desired_generation: claimed.desired_generation,
                correlation_id: claimed.correlation_id,
                node_id,
            });
        }

        metrics::gauge!("orchestrator_operations_accepted").set(rows.len() as f64);

        type DispatchFut<'a> = std::pin::Pin<
            Box<dyn std::future::Future<Output = (usize, Result<(), ChvError>, f64)> + Send + 'a>,
        >;
        let mut futs: Vec<DispatchFut<'_>> = Vec::with_capacity(rows.len());

        for (idx, row) in rows.iter().enumerate() {
            futs.push(Box::pin(async move {
                let start = std::time::Instant::now();
                let result = self.dispatch_operation(row).await;
                let duration = start.elapsed().as_secs_f64();
                (idx, result, duration)
            }));
        }

        let dispatch_results = futures::future::join_all(futs).await;

        for (idx, dispatch_result, duration) in dispatch_results {
            let row = &rows[idx];
            let status_label = if dispatch_result.is_ok() {
                "success"
            } else {
                "failure"
            };
            metrics::counter!(
                "orchestrator_operations_dispatched_total",
                "type" => row.operation_type.clone(),
                "status" => status_label,
            )
            .increment(1);
            metrics::histogram!(
                "orchestrator_dispatch_duration_seconds",
                "type" => row.operation_type.clone(),
            )
            .record(duration);
            metrics::histogram!(
                CHV_OPERATION_DURATION_SECONDS,
                "operation" => row.operation_type.clone(),
            )
            .record(duration);

            if let Err(e) = dispatch_result {
                warn!(
                    operation_id = %row.operation_id,
                    operation_type = %row.operation_type,
                    error = %e,
                    "dispatch failed"
                );

                // Check current retry count
                let retry_count: i32 =
                    sqlx::query_scalar("SELECT retry_count FROM operations WHERE operation_id = ?")
                        .bind(&row.operation_id)
                        .fetch_one(&self.pool)
                        .await
                        .unwrap_or(0);

                let new_retry_count = retry_count + 1;
                if new_retry_count <= MAX_DISPATCH_RETRIES {
                    // Schedule retry with exponential backoff: 10s, 20s, 40s
                    let backoff_secs = 10i64 * (1 << (new_retry_count - 1));
                    let next_retry = chrono::Utc::now() + chrono::Duration::seconds(backoff_secs);
                    let op_id = OperationId::new(row.operation_id.clone()).map_err(|e| {
                        ChvError::Internal {
                            reason: format!("invalid operation_id: {e}"),
                        }
                    })?;
                    if let Err(retry_err) = self
                        .operation_repo
                        .mark_for_retry(
                            &op_id,
                            new_retry_count,
                            &next_retry.to_rfc3339(),
                            &e.to_string(),
                            now_unix_ms(),
                        )
                        .await
                    {
                        error!(
                            operation_id = %row.operation_id,
                            error = %retry_err,
                            "failed to mark operation for retry"
                        );
                    } else {
                        info!(
                            operation_id = %row.operation_id,
                            retry = new_retry_count,
                            next_retry_at = %next_retry.to_rfc3339(),
                            "operation scheduled for retry"
                        );
                    }
                } else {
                    // Permanently failed after exhausting retries
                    if let Err(update_err) = self
                        .operation_repo
                        .update_status(&OperationStatusUpdateInput {
                            operation_id: OperationId::new(row.operation_id.clone()).map_err(
                                |e| ChvError::Internal {
                                    reason: format!("invalid operation_id: {e}"),
                                },
                            )?,
                            status: OperationStatus::Failed,
                            error_code: Some("DISPATCH_FAILED".into()),
                            error_message: Some(format!(
                                "permanently failed after {} retries: {}",
                                MAX_DISPATCH_RETRIES, e
                            )),
                            observed_generation: None,
                            updated_by: Some("orchestrator".into()),
                            updated_unix_ms: now_unix_ms(),
                        })
                        .await
                    {
                        error!(
                            operation_id = %row.operation_id,
                            error = %update_err,
                            "failed to update operation status after exhausting retries"
                        );
                    }
                }
            }
        }

        Ok(())
    }

    async fn reap_stuck_operations(&self) -> Result<u64, ChvError> {
        // Operations stuck in Running for more than 60 seconds are likely orphaned
        // (orchestrator crashed between claiming and completing dispatch).
        // Transition them back to Accepted for re-dispatch.
        let result = sqlx::query(
            r#"
            UPDATE operations SET status = 'Accepted', updated_by = 'reaper'
            WHERE status = 'Running'
              AND updated_at < strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-60 seconds')
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to reap stuck operations: {e}"),
        })?;

        let reaped = result.rows_affected();
        if reaped > 0 {
            warn!(
                count = reaped,
                "reaped stuck Running operations back to Accepted"
            );
        }
        Ok(reaped)
    }

    /// Detect nodes that have not reported observed state within 60 seconds and mark
    /// them as Unreachable so the scheduler will not place new VMs there.
    async fn check_node_liveness(&self) -> Result<(), ChvError> {
        let stale_nodes: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT node_id FROM node_observed_state
            WHERE observed_state NOT IN ('Unreachable', 'Failed')
              AND last_seen_at < strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-60 seconds')
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to query stale nodes: {e}"),
        })?;

        for (node_id,) in &stale_nodes {
            warn!(node_id = %node_id, "node has not reported in 60s, marking Unreachable");
            sqlx::query(
                r#"UPDATE node_observed_state
                   SET observed_state = 'Unreachable',
                       updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                   WHERE node_id = ?"#,
            )
            .bind(node_id)
            .execute(&self.pool)
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("failed to mark node {node_id} as Unreachable: {e}"),
            })?;

            // Evict from connection pool since the agent is likely dead
            self.node_client_pool.evict(node_id);
        }

        Ok(())
    }

    async fn dispatch_operation(&self, row: &AcceptedOperationRow) -> Result<(), ChvError> {
        let node_id = row
            .node_id
            .as_deref()
            .ok_or_else(|| ChvError::InvalidArgument {
                field: "node_id".to_string(),
                reason: format!("operation {} has no target node", row.operation_id),
            })?;

        // Schedulability check: operations that place new workloads require TenantReady
        if Self::requires_schedulable_node(&row.operation_type) {
            self.require_node_schedulable(node_id).await?;
        }

        let socket_path = self.resolve_agent_socket(node_id);
        let mut client = self
            .node_client_pool
            .get_or_connect(node_id, &socket_path)
            .await?;

        let generation = match row.desired_generation {
            Some(g) => g.to_string(),
            None => {
                // Fetch the node's current observed_generation from the DB
                // rather than defaulting to "1" which could cause stale operations.
                let observed: Option<i64> = sqlx::query_scalar(
                    "SELECT observed_generation FROM vm_observed_state WHERE vm_id = ?",
                )
                .bind(&row.resource_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| ChvError::Internal {
                    reason: format!(
                        "failed to fetch observed_generation for {}: {e}",
                        row.resource_id
                    ),
                })?
                .flatten();
                observed.unwrap_or(1).to_string()
            }
        };

        // Status already set to Running by the atomic claim in tick()

        let ack = match row.operation_type.as_str() {
            "create" | "CreateVm" | "ResizeVm" => {
                // Desired-state path: build full agent spec and dispatch ApplyVmDesiredState
                let vm_spec_json = self.build_agent_vm_spec(&row.resource_id).await?;
                client
                    .apply_vm_desired_state(
                        node_id,
                        &row.resource_id,
                        &generation,
                        vm_spec_json.into_bytes(),
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "StartVm" => {
                client
                    .start_vm(
                        node_id,
                        &row.resource_id,
                        &generation,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "StopVm" => {
                client
                    .stop_vm(
                        node_id,
                        &row.resource_id,
                        &generation,
                        false,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "ForceStopVm" => {
                client
                    .stop_vm(
                        node_id,
                        &row.resource_id,
                        &generation,
                        true,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "RebootVm" => {
                let force_reboot = row
                    .correlation_id
                    .as_deref()
                    .map(|s| s.contains("force=true"))
                    .unwrap_or(false);
                client
                    .reboot_vm(
                        node_id,
                        &row.resource_id,
                        &generation,
                        force_reboot,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "DeleteVm" => {
                client
                    .delete_vm(
                        node_id,
                        &row.resource_id,
                        &generation,
                        false,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "SnapshotVm" => {
                let destination = row.correlation_id.as_deref().unwrap_or("");
                client
                    .snapshot_vm(
                        node_id,
                        &row.resource_id,
                        &generation,
                        destination,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "RestoreSnapshot" => {
                let source = row.correlation_id.as_deref().unwrap_or("");
                client
                    .restore_snapshot(
                        node_id,
                        &row.resource_id,
                        &generation,
                        source,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "AttachVolume" => {
                let corr = row.correlation_id.as_deref().unwrap_or("");
                let vm_id = corr.strip_prefix("vm=").unwrap_or(corr);
                client
                    .attach_volume(
                        node_id,
                        &row.resource_id,
                        vm_id,
                        &generation,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "DetachVolume" => {
                let corr = row.correlation_id.as_deref().unwrap_or("");
                let vm_id = corr
                    .strip_prefix("vm=")
                    .and_then(|s| s.split(':').next())
                    .unwrap_or(corr);
                let force = corr.contains("force=true");
                client
                    .detach_volume(
                        node_id,
                        &row.resource_id,
                        vm_id,
                        &generation,
                        force,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "ResizeVolume" => {
                let new_size = row
                    .correlation_id
                    .as_deref()
                    .and_then(|s| s.strip_prefix("size="))
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);
                client
                    .resize_volume(
                        node_id,
                        &row.resource_id,
                        &generation,
                        new_size,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "SnapshotVolume" => {
                let snapshot_name = row.correlation_id.as_deref().unwrap_or("");
                client
                    .snapshot_volume(
                        node_id,
                        &row.resource_id,
                        &generation,
                        snapshot_name,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "RestoreVolume" => {
                let snapshot_name = row.correlation_id.as_deref().unwrap_or("");
                client
                    .restore_volume(
                        node_id,
                        &row.resource_id,
                        &generation,
                        snapshot_name,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "DeleteVolumeSnapshot" => {
                let snapshot_name = row.correlation_id.as_deref().unwrap_or("");
                client
                    .delete_volume_snapshot(
                        node_id,
                        &row.resource_id,
                        &generation,
                        snapshot_name,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "CloneVolume" => {
                let source = row.correlation_id.as_deref().unwrap_or("");
                let source_volume_id = source.strip_prefix("source=").unwrap_or(source);
                client
                    .clone_volume(
                        node_id,
                        source_volume_id,
                        &row.resource_id,
                        &generation,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "StartNetwork" => {
                client
                    .start_network(
                        node_id,
                        &row.resource_id,
                        &generation,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "StopNetwork" => {
                client
                    .stop_network(
                        node_id,
                        &row.resource_id,
                        &generation,
                        false,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "ForceStopNetwork" => {
                client
                    .stop_network(
                        node_id,
                        &row.resource_id,
                        &generation,
                        true,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "RestartNetwork" => {
                client
                    .restart_network(
                        node_id,
                        &row.resource_id,
                        &generation,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "PauseVm" => {
                client
                    .pause_vm(
                        node_id,
                        &row.resource_id,
                        &generation,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "ResumeVm" => {
                client
                    .resume_vm(
                        node_id,
                        &row.resource_id,
                        &generation,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "PowerButtonVm" => {
                client
                    .power_button_vm(
                        node_id,
                        &row.resource_id,
                        &generation,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "CoredumpVm" => {
                let destination = row.correlation_id.as_deref().unwrap_or("");
                client
                    .coredump_vm(
                        node_id,
                        &row.resource_id,
                        &generation,
                        destination,
                        &row.operation_id,
                        None,
                    )
                    .await
            }
            "MigrateVm" => {
                // MigrateVm is a long-running operation driven by the migration state machine.
                // Parse correlation_id to extract source, dest, and config.
                let corr = row.correlation_id.as_deref().unwrap_or("");
                let (source_node_id, dest_node_id, config) =
                    crate::migration::MigrationConfig::from_correlation_id(corr);

                if source_node_id.is_empty() || dest_node_id.is_empty() {
                    return Err(ChvError::InvalidArgument {
                        field: "correlation_id".to_string(),
                        reason: format!(
                            "MigrateVm requires source= and dest= in correlation_id, got: {}",
                            corr
                        ),
                    });
                }

                let migration_id = format!("mig-{}", &row.operation_id);
                let mut state = crate::migration::MigrationState {
                    migration_id: migration_id.clone(),
                    operation_id: row.operation_id.clone(),
                    vm_id: row.resource_id.clone(),
                    source_node_id: source_node_id.clone(),
                    dest_node_id: dest_node_id.clone(),
                    phase: crate::migration::MigrationPhase::Pending,
                    config,
                    bytes_transferred: 0,
                    total_bytes: 0,
                    convergence_round: 0,
                    dirty_blocks_remaining: 0,
                };

                // Create migration record in DB
                crate::migration::create_migration_record(&self.pool, &state).await?;

                // Execute the migration state machine
                let result = crate::migration::execute_migration(
                    &self.pool,
                    &self.node_client_pool,
                    &self.agent_socket_pattern,
                    &mut state,
                )
                .await;

                // Mark the operation based on migration result
                let (final_status, error_message) = match &result {
                    Ok(()) => (OperationStatus::Succeeded, None),
                    Err(e) => (OperationStatus::Failed, Some(e.to_string())),
                };

                self.operation_repo
                    .update_status(&OperationStatusUpdateInput {
                        operation_id: OperationId::new(row.operation_id.clone()).map_err(|e| {
                            ChvError::Internal {
                                reason: format!("invalid operation_id: {e}"),
                            }
                        })?,
                        status: final_status,
                        error_code: if result.is_err() {
                            Some("MIGRATION_FAILED".into())
                        } else {
                            None
                        },
                        error_message,
                        observed_generation: None,
                        updated_by: Some("orchestrator".into()),
                        updated_unix_ms: now_unix_ms(),
                    })
                    .await
                    .map_err(|e| ChvError::Internal {
                        reason: format!("failed to mark migration operation terminal: {e}"),
                    })?;

                // Return Ok since we handled the status update ourselves
                return match result {
                    Ok(()) => Ok(()),
                    Err(e) => Err(e),
                };
            }
            other => {
                return Err(ChvError::Internal {
                    reason: format!("unsupported operation_type for dispatch: {other}"),
                });
            }
        };

        match ack {
            Ok(result) => {
                let status = result
                    .result
                    .as_ref()
                    .map(|r| r.status.as_str())
                    .unwrap_or("OK");
                let accepted = status.eq_ignore_ascii_case("ok");
                let final_status = if accepted {
                    OperationStatus::Succeeded
                } else {
                    OperationStatus::Failed
                };
                let error_message = if accepted {
                    None
                } else {
                    result.result.map(|r| r.human_summary)
                };
                self.operation_repo
                    .update_status(&OperationStatusUpdateInput {
                        operation_id: OperationId::new(row.operation_id.clone()).map_err(|e| {
                            ChvError::Internal {
                                reason: format!("invalid operation_id: {e}"),
                            }
                        })?,
                        status: final_status,
                        error_code: None,
                        error_message,
                        observed_generation: None,
                        updated_by: Some("orchestrator".into()),
                        updated_unix_ms: now_unix_ms(),
                    })
                    .await
                    .map_err(|e| ChvError::Internal {
                        reason: format!("failed to mark operation terminal: {e}"),
                    })?;

                // For successful resize, apply the new size to volumes.capacity_bytes
                if accepted && row.operation_type == "ResizeVolume" {
                    if let Some(new_size) = row
                        .correlation_id
                        .as_deref()
                        .and_then(|s| s.strip_prefix("size="))
                        .and_then(|s| s.parse::<i64>().ok())
                    {
                        let volume_id = &row.resource_id;
                        if let Err(e) =
                            sqlx::query("UPDATE volumes SET capacity_bytes = ? WHERE volume_id = ?")
                                .bind(new_size)
                                .bind(volume_id)
                                .execute(&self.pool)
                                .await
                        {
                            tracing::error!(
                                operation_id = %row.operation_id,
                                volume_id = %volume_id,
                                new_size = new_size,
                                error = %e,
                                "failed to persist resized capacity after successful dispatch"
                            );
                        }
                        if let Err(e) = sqlx::query(
                            "UPDATE volume_desired_state SET resize_to_bytes = NULL WHERE volume_id = ?"
                        )
                        .bind(volume_id)
                        .execute(&self.pool)
                        .await
                        {
                            tracing::error!(
                                operation_id = %row.operation_id,
                                volume_id = %volume_id,
                                error = %e,
                                "failed to clear resize_to_bytes after successful dispatch"
                            );
                        }
                    }
                }

                info!(
                    operation_id = %row.operation_id,
                    operation_type = %row.operation_type,
                    node_id = %node_id,
                    "dispatch succeeded"
                );
                Ok(())
            }
            Err(e) => {
                if matches!(e, ChvError::BackendUnavailable { .. }) {
                    self.node_client_pool.evict(node_id);
                }
                self.operation_repo
                    .update_status(&OperationStatusUpdateInput {
                        operation_id: OperationId::new(row.operation_id.clone()).map_err(|e| {
                            ChvError::Internal {
                                reason: format!("invalid operation_id: {e}"),
                            }
                        })?,
                        status: OperationStatus::Failed,
                        error_code: Some("AGENT_REJECTED".into()),
                        error_message: Some(e.to_string()),
                        observed_generation: None,
                        updated_by: Some("orchestrator".into()),
                        updated_unix_ms: now_unix_ms(),
                    })
                    .await
                    .map_err(|e2| ChvError::Internal {
                        reason: format!("agent rejected operation and status update failed: {e2}"),
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

    fn requires_schedulable_node(operation_type: &str) -> bool {
        matches!(
            operation_type,
            "create" | "CreateVm" | "MigrateVm" | "ResizeVm"
        )
    }

    async fn require_node_schedulable(&self, node_id: &str) -> Result<(), ChvError> {
        let observed_state: Option<String> =
            sqlx::query_scalar("SELECT observed_state FROM node_observed_state WHERE node_id = ?")
                .bind(node_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| ChvError::Internal {
                    reason: format!("failed to check node state for {}: {e}", node_id),
                })?;

        let scheduling_paused: Option<bool> = sqlx::query_scalar(
            "SELECT scheduling_paused FROM node_desired_state WHERE node_id = ?",
        )
        .bind(node_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to check scheduling_paused for {}: {e}", node_id),
        })?;

        if scheduling_paused.unwrap_or(false) {
            return Err(ChvError::InvalidArgument {
                field: "node_id".to_string(),
                reason: format!("node {} has scheduling paused", node_id),
            });
        }

        match observed_state.as_deref() {
            Some("TenantReady") => Ok(()),
            Some(state) => Err(ChvError::InvalidArgument {
                field: "node_id".to_string(),
                reason: format!(
                    "node {} is in state '{}', must be TenantReady for placement",
                    node_id, state
                ),
            }),
            None => Err(ChvError::InvalidArgument {
                field: "node_id".to_string(),
                reason: format!(
                    "node {} has no observed state, cannot accept placements",
                    node_id
                ),
            }),
        }
    }

    /// Build the agent-compatible VmSpec JSON from control-plane DB records.
    pub(crate) async fn build_agent_vm_spec(&self, vm_id: &str) -> Result<String, ChvError> {
        let vm_row = sqlx::query_as::<_, VmDesiredStateRow>(
            r#"
            SELECT
                v.display_name,
                vds.cpu_count,
                vds.memory_bytes,
                vds.image_ref,
                vds.desired_power_state,
                vds.cloud_init_userdata,
                v.hv_cpu_nested,
                v.hv_cpu_amx,
                v.hv_cpu_kvm_hyperv,
                v.hv_memory_mergeable,
                v.hv_memory_hugepages,
                v.hv_memory_shared,
                v.hv_memory_prefault,
                v.hv_iommu,
                v.hv_rng_src,
                v.hv_watchdog,
                v.hv_landlock_enable,
                v.hv_serial_mode,
                v.hv_console_mode,
                v.hv_pvpanic,
                v.hv_tpm_type,
                v.hv_tpm_socket_path
            FROM vms v
            JOIN vm_desired_state vds ON v.vm_id = vds.vm_id
            WHERE v.vm_id = ?
            "#,
        )
        .bind(vm_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to query vm desired state: {e}"),
        })?
        .ok_or_else(|| ChvError::NotFound {
            resource: "vm_desired_state".to_string(),
            id: vm_id.to_string(),
        })?;

        let global = HypervisorSettingsRepository::new(self.pool.clone())
            .get_settings()
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(vm_id = %vm_id, error = %e, "failed to fetch hypervisor_settings, using defaults");
                HypervisorSettingsRow {
                    id: 1,
                    cpu_nested: chv_common::hypervisor::DEFAULT_CPU_NESTED,
                    cpu_amx: chv_common::hypervisor::DEFAULT_CPU_AMX,
                    cpu_kvm_hyperv: chv_common::hypervisor::DEFAULT_CPU_KVM_HYPERV,
                    memory_mergeable: chv_common::hypervisor::DEFAULT_MEMORY_MERGEABLE,
                    memory_hugepages: chv_common::hypervisor::DEFAULT_MEMORY_HUGEPAGES,
                    memory_shared: chv_common::hypervisor::DEFAULT_MEMORY_SHARED,
                    memory_prefault: chv_common::hypervisor::DEFAULT_MEMORY_PREFAULT,
                    iommu: chv_common::hypervisor::DEFAULT_IOMMU,
                    rng_src: chv_common::hypervisor::DEFAULT_RNG_SRC.to_string(),
                    watchdog: chv_common::hypervisor::DEFAULT_WATCHDOG,
                    landlock_enable: chv_common::hypervisor::DEFAULT_LANDLOCK_ENABLE,
                    serial_mode: chv_common::hypervisor::DEFAULT_SERIAL_MODE.to_string(),
                    console_mode: chv_common::hypervisor::DEFAULT_CONSOLE_MODE.to_string(),
                    pvpanic: chv_common::hypervisor::DEFAULT_PVPANIC,
                    tpm_type: chv_common::hypervisor::DEFAULT_TPM_TYPE.map(|s| s.to_string()),
                    tpm_socket_path: chv_common::hypervisor::DEFAULT_TPM_SOCKET_PATH.map(|s| s.to_string()),
                    profile_id: None,
                    updated_at: String::new(),
                }
            });

        let volume_rows = sqlx::query_as::<_, VolumeDesiredStateRow>(
            r#"
            SELECT
                vds.volume_id,
                vds.read_only,
                v.capacity_bytes
            FROM volume_desired_state vds
            JOIN volumes v ON v.volume_id = vds.volume_id
            WHERE vds.attached_vm_id = ?
            ORDER BY vds.volume_id
            "#,
        )
        .bind(vm_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to query volume desired state: {e}"),
        })?;

        let nic_rows = sqlx::query_as::<_, VmNicRow>(
            r#"
            SELECT
                network_id,
                mac_address,
                ip_address
            FROM vm_nic_desired_state
            WHERE vm_id = ?
            ORDER BY nic_id
            "#,
        )
        .bind(vm_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to query vm nic desired state: {e}"),
        })?;

        let kernel_path = if let Some(ref image_ref) = vm_row.image_ref {
            self.resolve_kernel_path(image_ref)
        } else {
            self.kernel_path.clone()
        };

        let disks: Vec<AgentDiskSpec> =
            volume_rows
                .into_iter()
                .map(|v| AgentDiskSpec {
                    volume_id: v.volume_id,
                    read_only: v.read_only.unwrap_or(false),
                    size_bytes: v.capacity_bytes.and_then(|b| {
                        if b > 0 {
                            Some(b as u64)
                        } else {
                            None
                        }
                    }),
                })
                .collect();

        let mut network_configs: std::collections::HashMap<String, (String, String)> =
            std::collections::HashMap::new();
        let unique_network_ids: Vec<&str> = nic_rows
            .iter()
            .map(|n| n.network_id.as_str())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        if !unique_network_ids.is_empty() {
            let placeholders = unique_network_ids
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");
            let query_str = format!(
                "SELECT network_id, cidr, gateway FROM network_desired_state WHERE network_id IN ({})",
                placeholders
            );
            let mut query = sqlx::query_as::<_, NetworkDesiredStateWithIdRow>(&query_str);
            for id in &unique_network_ids {
                query = query.bind(id);
            }
            let net_rows = query
                .fetch_all(&self.pool)
                .await
                .map_err(|e| ChvError::Internal {
                    reason: format!("failed to query network desired states: {e}"),
                })?;
            for nr in net_rows {
                network_configs.insert(
                    nr.network_id,
                    (nr.cidr.unwrap_or_default(), nr.gateway.unwrap_or_default()),
                );
            }
        }

        let nics: Vec<AgentNicSpec> = nic_rows
            .into_iter()
            .map(|n| {
                let (cidr, gateway) = network_configs
                    .get(&n.network_id)
                    .cloned()
                    .unwrap_or_default();
                AgentNicSpec {
                    network_id: n.network_id,
                    mac_address: n.mac_address.unwrap_or_default(),
                    ip_address: n.ip_address.unwrap_or_default(),
                    cidr,
                    gateway,
                }
            })
            .collect();

        let desired_state = vm_row
            .desired_power_state
            .unwrap_or_else(|| "Running".to_string());

        let overrides = HypervisorOverrides {
            cpu_nested: Some(vm_row.hv_cpu_nested.unwrap_or(global.cpu_nested)),
            cpu_amx: Some(vm_row.hv_cpu_amx.unwrap_or(global.cpu_amx)),
            cpu_kvm_hyperv: Some(vm_row.hv_cpu_kvm_hyperv.unwrap_or(global.cpu_kvm_hyperv)),
            memory_mergeable: Some(
                vm_row
                    .hv_memory_mergeable
                    .unwrap_or(global.memory_mergeable),
            ),
            memory_hugepages: Some(
                vm_row
                    .hv_memory_hugepages
                    .unwrap_or(global.memory_hugepages),
            ),
            memory_shared: Some(vm_row.hv_memory_shared.unwrap_or(global.memory_shared)),
            memory_prefault: Some(vm_row.hv_memory_prefault.unwrap_or(global.memory_prefault)),
            iommu: Some(vm_row.hv_iommu.unwrap_or(global.iommu)),
            rng_src: Some(vm_row.hv_rng_src.unwrap_or_else(|| global.rng_src.clone())),
            watchdog: Some(vm_row.hv_watchdog.unwrap_or(global.watchdog)),
            landlock_enable: Some(vm_row.hv_landlock_enable.unwrap_or(global.landlock_enable)),
            serial_mode: Some(
                vm_row
                    .hv_serial_mode
                    .unwrap_or_else(|| global.serial_mode.clone()),
            ),
            console_mode: Some(
                vm_row
                    .hv_console_mode
                    .unwrap_or_else(|| global.console_mode.clone()),
            ),
            pvpanic: Some(vm_row.hv_pvpanic.unwrap_or(global.pvpanic)),
            tpm_type: vm_row
                .hv_tpm_type
                .clone()
                .or_else(|| global.tpm_type.clone())
                .or_else(|| chv_common::hypervisor::DEFAULT_TPM_TYPE.map(|s| s.to_string())),
            tpm_socket_path: vm_row
                .hv_tpm_socket_path
                .clone()
                .or_else(|| global.tpm_socket_path.clone())
                .or_else(|| chv_common::hypervisor::DEFAULT_TPM_SOCKET_PATH.map(|s| s.to_string())),
        };

        if let Err(e) = validate_merged_overrides(&overrides) {
            return Err(ChvError::InvalidArgument {
                field: "hypervisor_overrides".to_string(),
                reason: e,
            });
        }

        // Validate disk seed image path exists before dispatching to agent.
        // NOTE: This validation only works in all-in-one deployments where controlplane
        // and agent share a filesystem. In multi-node setups, the agent-side reconciler
        // handles missing images via backoff retry.
        let disk_seed_path = self.resolve_disk_seed_path(vm_row.image_ref.as_deref());
        if let Some(ref seed_path) = disk_seed_path {
            let path = std::path::Path::new(seed_path);
            if !path.exists() {
                return Err(ChvError::InvalidArgument {
                    field: "image_ref".to_string(),
                    reason: format!(
                        "image file not found at resolved path: {}. Import the image first.",
                        seed_path
                    ),
                });
            }
        }

        let spec = AgentVmSpec {
            name: vm_row.display_name.unwrap_or_else(|| vm_id.to_string()),
            cpus: vm_row.cpu_count.unwrap_or(1) as u32,
            memory_bytes: vm_row.memory_bytes.unwrap_or(512 * 1024 * 1024) as u64,
            kernel_path,
            firmware_path: Some(self.firmware_path.clone()),
            disk_seed_path,
            disks,
            nics,
            desired_state,
            cloud_init_userdata: vm_row.cloud_init_userdata,
            hypervisor_overrides: Some(overrides),
        };

        serde_json::to_string(&spec).map_err(|e| ChvError::Internal {
            reason: format!("failed to serialize agent vm spec: {e}"),
        })
    }

    fn resolve_kernel_path(&self, image_ref: &str) -> String {
        // For the first VM milestone, use a simple config-based mapping.
        // In production this would query an image registry.
        // If image_ref looks like a disk image path (absolute path or file:// URI),
        // use the default kernel path instead.
        if image_ref == "default"
            || image_ref.is_empty()
            || image_ref.starts_with('/')
            || image_ref.starts_with("file://")
        {
            self.kernel_path.clone()
        } else {
            format!("/var/lib/chv/kernels/{}", image_ref)
        }
    }

    fn resolve_disk_seed_path(&self, image_ref: Option<&str>) -> Option<String> {
        let image_ref = image_ref?.trim();
        if image_ref.is_empty() || image_ref == "default" {
            return None;
        }
        if let Some(path) = image_ref.strip_prefix("file://") {
            return Some(path.to_string());
        }
        if image_ref.starts_with('/') {
            return Some(image_ref.to_string());
        }
        Some(format!("/var/lib/chv/images/{}", image_ref))
    }
}

fn validate_merged_overrides(overrides: &HypervisorOverrides) -> Result<(), String> {
    if overrides.iommu == Some(true) && overrides.memory_shared != Some(true) {
        return Err("iommu=true requires memory_shared=true".to_string());
    }
    Ok(())
}

#[derive(sqlx::FromRow)]
struct ClaimedOperationRow {
    operation_id: String,
    operation_type: String,
    resource_kind: String,
    resource_id: String,
    desired_generation: Option<i64>,
    correlation_id: Option<String>,
}

#[derive(sqlx::FromRow)]
struct AcceptedOperationRow {
    operation_id: String,
    operation_type: String,
    #[allow(dead_code)]
    resource_kind: String,
    resource_id: String,
    desired_generation: Option<i64>,
    node_id: Option<String>,
    correlation_id: Option<String>,
}

#[derive(sqlx::FromRow)]
struct VmDesiredStateRow {
    display_name: Option<String>,
    cpu_count: Option<i32>,
    memory_bytes: Option<i64>,
    image_ref: Option<String>,
    desired_power_state: Option<String>,
    cloud_init_userdata: Option<String>,
    hv_cpu_nested: Option<bool>,
    hv_cpu_amx: Option<bool>,
    hv_cpu_kvm_hyperv: Option<bool>,
    hv_memory_mergeable: Option<bool>,
    hv_memory_hugepages: Option<bool>,
    hv_memory_shared: Option<bool>,
    hv_memory_prefault: Option<bool>,
    hv_iommu: Option<bool>,
    hv_rng_src: Option<String>,
    hv_watchdog: Option<bool>,
    hv_landlock_enable: Option<bool>,
    hv_serial_mode: Option<String>,
    hv_console_mode: Option<String>,
    hv_pvpanic: Option<bool>,
    hv_tpm_type: Option<String>,
    hv_tpm_socket_path: Option<String>,
}

#[derive(sqlx::FromRow)]
struct VolumeDesiredStateRow {
    volume_id: String,
    read_only: Option<bool>,
    capacity_bytes: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct VmNicRow {
    network_id: String,
    mac_address: Option<String>,
    ip_address: Option<String>,
}

#[derive(sqlx::FromRow)]
struct NetworkDesiredStateWithIdRow {
    network_id: String,
    cidr: Option<String>,
    gateway: Option<String>,
}

#[derive(serde::Serialize)]
struct AgentVmSpec {
    name: String,
    cpus: u32,
    memory_bytes: u64,
    kernel_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    firmware_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    disk_seed_path: Option<String>,
    disks: Vec<AgentDiskSpec>,
    nics: Vec<AgentNicSpec>,
    desired_state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cloud_init_userdata: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hypervisor_overrides: Option<HypervisorOverrides>,
}

#[derive(serde::Serialize)]
struct AgentDiskSpec {
    volume_id: String,
    read_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    size_bytes: Option<u64>,
}

#[derive(serde::Serialize)]
struct AgentNicSpec {
    network_id: String,
    mac_address: String,
    ip_address: String,
    cidr: String,
    gateway: String,
}

fn now_unix_ms() -> i64 {
    chv_common::now_unix_ms()
}
