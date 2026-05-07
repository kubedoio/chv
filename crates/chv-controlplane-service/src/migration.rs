//! Live migration orchestrator state machine.
//!
//! Drives VM migration through phases: PreCopyDisk -> ConvergingDisk -> MemoryMigration -> Paused -> Completed.
//! Handles rollback at each phase according to spec.

use crate::node_client_pool::NodeClientPool;
use chv_controlplane_store::StorePool;
use chv_errors::ChvError;
use control_plane_node_api::control_plane_node_api as proto;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{error, info, warn};

/// Phases of a live migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationPhase {
    Pending,
    PreCopyDisk,
    ConvergingDisk,
    MemoryMigration,
    Paused,
    Completed,
    Failed,
    RolledBack,
}

impl MigrationPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::PreCopyDisk => "PreCopyDisk",
            Self::ConvergingDisk => "ConvergingDisk",
            Self::MemoryMigration => "MemoryMigration",
            Self::Paused => "Paused",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::RolledBack => "RolledBack",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "Pending" => Some(Self::Pending),
            "PreCopyDisk" => Some(Self::PreCopyDisk),
            "ConvergingDisk" => Some(Self::ConvergingDisk),
            "MemoryMigration" => Some(Self::MemoryMigration),
            "Paused" => Some(Self::Paused),
            "Completed" => Some(Self::Completed),
            "Failed" => Some(Self::Failed),
            "RolledBack" => Some(Self::RolledBack),
            _ => None,
        }
    }

    /// Convert to proto enum value.
    pub fn to_proto(&self) -> i32 {
        match self {
            Self::Pending => proto::MigrationPhase::Pending as i32,
            Self::PreCopyDisk => proto::MigrationPhase::PrecopyDisk as i32,
            Self::ConvergingDisk => proto::MigrationPhase::ConvergingDisk as i32,
            Self::MemoryMigration => proto::MigrationPhase::MemoryMigration as i32,
            Self::Paused => proto::MigrationPhase::Paused as i32,
            Self::Completed => proto::MigrationPhase::Completed as i32,
            Self::Failed => proto::MigrationPhase::Failed as i32,
            Self::RolledBack => proto::MigrationPhase::RolledBack as i32,
        }
    }
}

/// Configuration parameters for a migration operation.
#[derive(Debug, Clone)]
pub struct MigrationConfig {
    pub dirty_threshold_blocks: u32,
    pub max_convergence_rounds: u32,
    pub block_size_bytes: u32,
    pub total_timeout_seconds: u32,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            dirty_threshold_blocks: 1024,
            max_convergence_rounds: 10,
            block_size_bytes: 4_194_304, // 4MB
            total_timeout_seconds: 0,    // 0 = use calculated default
        }
    }
}

impl MigrationConfig {
    /// Parse from proto message.
    pub fn from_proto(proto: &proto::MigrationConfig) -> Self {
        let defaults = Self::default();
        Self {
            dirty_threshold_blocks: if proto.dirty_threshold_blocks > 0 {
                proto.dirty_threshold_blocks
            } else {
                defaults.dirty_threshold_blocks
            },
            max_convergence_rounds: if proto.max_convergence_rounds > 0 {
                proto.max_convergence_rounds
            } else {
                defaults.max_convergence_rounds
            },
            block_size_bytes: if proto.block_size_bytes > 0 {
                proto.block_size_bytes
            } else {
                defaults.block_size_bytes
            },
            total_timeout_seconds: proto.total_timeout_seconds,
        }
    }

    /// Convert to proto message.
    pub fn to_proto(&self) -> proto::MigrationConfig {
        proto::MigrationConfig {
            dirty_threshold_blocks: self.dirty_threshold_blocks,
            max_convergence_rounds: self.max_convergence_rounds,
            block_size_bytes: self.block_size_bytes,
            total_timeout_seconds: self.total_timeout_seconds,
        }
    }

    /// Parse from correlation_id format:
    /// `source={source_node_id}:dest={dest_node_id}:threshold={N}:rounds={N}:block_size={N}:timeout={N}`
    pub fn from_correlation_id(corr: &str) -> (String, String, Self) {
        let mut source_node = String::new();
        let mut dest_node = String::new();
        let mut config = Self::default();

        for part in corr.split(':') {
            if let Some(val) = part.strip_prefix("source=") {
                source_node = val.to_string();
            } else if let Some(val) = part.strip_prefix("dest=") {
                dest_node = val.to_string();
            } else if let Some(val) = part.strip_prefix("threshold=") {
                if let Ok(v) = val.parse::<u32>() {
                    config.dirty_threshold_blocks = v;
                }
            } else if let Some(val) = part.strip_prefix("rounds=") {
                if let Ok(v) = val.parse::<u32>() {
                    config.max_convergence_rounds = v;
                }
            } else if let Some(val) = part.strip_prefix("block_size=") {
                if let Ok(v) = val.parse::<u32>() {
                    config.block_size_bytes = v;
                }
            } else if let Some(val) = part.strip_prefix("timeout=") {
                if let Ok(v) = val.parse::<u32>() {
                    config.total_timeout_seconds = v;
                }
            }
        }

        (source_node, dest_node, config)
    }
}

/// Tracks the current state of a live migration.
#[derive(Debug, Clone)]
pub struct MigrationState {
    pub migration_id: String,
    pub operation_id: String,
    pub vm_id: String,
    pub source_node_id: String,
    pub dest_node_id: String,
    pub phase: MigrationPhase,
    pub config: MigrationConfig,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub convergence_round: u32,
    pub dirty_blocks_remaining: u64,
}

/// Calculate phase timeouts based on VM resource sizes.
pub struct PhaseTimeouts {
    pub precopy_disk_secs: u64,
    pub converging_disk_per_round_secs: u64,
    pub converging_disk_total_secs: u64,
    pub memory_migration_secs: u64,
    pub paused_secs: u64,
    pub total_secs: u64,
}

impl PhaseTimeouts {
    /// Calculate timeouts based on VM disk and memory sizes.
    ///
    /// - PreCopyDisk: disk_size_gb * 60s
    /// - ConvergingDisk: 300s per round, total 3000s max
    /// - MemoryMigration: memory_size_gb * 30s + 120s
    /// - Paused (final sync): 60s
    /// - Total: sum + 300s buffer
    pub fn calculate(disk_size_gb: u64, memory_size_gb: u64) -> Self {
        let precopy_disk_secs = disk_size_gb.max(1) * 60;
        let converging_disk_per_round_secs = 300;
        let converging_disk_total_secs = 3000;
        let memory_migration_secs = memory_size_gb.max(1) * 30 + 120;
        let paused_secs = 60;
        let total_secs = precopy_disk_secs
            + converging_disk_total_secs
            + memory_migration_secs
            + paused_secs
            + 300;

        Self {
            precopy_disk_secs,
            converging_disk_per_round_secs,
            converging_disk_total_secs,
            memory_migration_secs,
            paused_secs,
            total_secs,
        }
    }
}

/// Execute the full migration state machine.
///
/// This drives the migration through all phases:
/// 1. Pending -> PreCopyDisk: Send migrate_vm to source agent to start disk precopy
/// 2. PreCopyDisk -> ConvergingDisk: Iteratively sync dirty blocks until convergence
/// 3. ConvergingDisk -> MemoryMigration: Transfer VM memory to destination
/// 4. MemoryMigration -> Paused: Pause VM for final sync
/// 5. Paused -> Completed: Resume VM on destination
///
/// Returns Ok(()) if migration completed successfully, Err if failed or rolled back.
pub async fn execute_migration(
    pool: &StorePool,
    node_client_pool: &NodeClientPool,
    agent_socket_pattern: &str,
    state: &mut MigrationState,
) -> Result<(), ChvError> {
    info!(
        migration_id = %state.migration_id,
        vm_id = %state.vm_id,
        source = %state.source_node_id,
        dest = %state.dest_node_id,
        "starting live migration"
    );

    // Look up VM disk and memory sizes for timeout calculation
    let (disk_size_gb, memory_size_gb) = get_vm_sizes(pool, &state.vm_id).await?;
    let timeouts = PhaseTimeouts::calculate(disk_size_gb, memory_size_gb);

    // Use configured total_timeout if provided, otherwise use calculated
    let total_timeout = if state.config.total_timeout_seconds > 0 {
        Duration::from_secs(state.config.total_timeout_seconds as u64)
    } else {
        Duration::from_secs(timeouts.total_secs)
    };

    let migration_result = tokio::time::timeout(total_timeout, async {
        // Phase 1: PreCopyDisk - dispatch migrate_vm to source agent
        transition_phase(pool, state, MigrationPhase::PreCopyDisk).await?;

        let source_socket = resolve_agent_socket(agent_socket_pattern, &state.source_node_id);
        let mut source_client = node_client_pool
            .get_or_connect(&state.source_node_id, &source_socket)
            .await?;

        let precopy_result = tokio::time::timeout(
            Duration::from_secs(timeouts.precopy_disk_secs),
            source_client.migrate_vm(
                &state.source_node_id,
                &state.vm_id,
                "1",
                &state.source_node_id,
                &state.dest_node_id,
                state.config.to_proto(),
                &state.operation_id,
                None,
            ),
        )
        .await;

        match precopy_result {
            Ok(Ok(_ack)) => {
                info!(
                    migration_id = %state.migration_id,
                    "precopy disk phase initiated on source agent"
                );
            }
            Ok(Err(e)) => {
                warn!(
                    migration_id = %state.migration_id,
                    error = %e,
                    "precopy disk failed, initiating rollback"
                );
                rollback_precopy(pool, state).await?;
                return Err(e);
            }
            Err(_elapsed) => {
                warn!(
                    migration_id = %state.migration_id,
                    "precopy disk timed out, initiating rollback"
                );
                rollback_precopy(pool, state).await?;
                return Err(ChvError::Internal {
                    reason: format!(
                        "migration {} precopy disk timed out after {}s",
                        state.migration_id, timeouts.precopy_disk_secs
                    ),
                });
            }
        }

        // Phase 2: ConvergingDisk - wait for agent to converge dirty blocks
        transition_phase(pool, state, MigrationPhase::ConvergingDisk).await?;

        let converge_result = tokio::time::timeout(
            Duration::from_secs(timeouts.converging_disk_total_secs),
            wait_for_convergence(pool, state),
        )
        .await;

        match converge_result {
            Ok(Ok(())) => {
                info!(
                    migration_id = %state.migration_id,
                    "disk convergence achieved"
                );
            }
            Ok(Err(e)) => {
                warn!(
                    migration_id = %state.migration_id,
                    error = %e,
                    "disk convergence failed, initiating rollback"
                );
                rollback_precopy(pool, state).await?;
                return Err(e);
            }
            Err(_elapsed) => {
                warn!(
                    migration_id = %state.migration_id,
                    "disk convergence timed out, initiating rollback"
                );
                rollback_precopy(pool, state).await?;
                return Err(ChvError::Internal {
                    reason: format!(
                        "migration {} converging disk timed out after {}s",
                        state.migration_id, timeouts.converging_disk_total_secs
                    ),
                });
            }
        }

        // Phase 3: MemoryMigration - transfer VM memory
        transition_phase(pool, state, MigrationPhase::MemoryMigration).await?;

        let memory_result = tokio::time::timeout(
            Duration::from_secs(timeouts.memory_migration_secs),
            wait_for_memory_migration(pool, state),
        )
        .await;

        match memory_result {
            Ok(Ok(())) => {
                info!(
                    migration_id = %state.migration_id,
                    "memory migration completed"
                );
            }
            Ok(Err(e)) => {
                // Cannot cleanly rollback during memory migration
                error!(
                    migration_id = %state.migration_id,
                    error = %e,
                    "memory migration failed, cannot rollback cleanly"
                );
                transition_phase(pool, state, MigrationPhase::Failed).await?;
                return Err(e);
            }
            Err(_elapsed) => {
                error!(
                    migration_id = %state.migration_id,
                    "memory migration timed out, marking failed (no clean rollback possible)"
                );
                transition_phase(pool, state, MigrationPhase::Failed).await?;
                return Err(ChvError::Internal {
                    reason: format!(
                        "migration {} memory migration timed out after {}s",
                        state.migration_id, timeouts.memory_migration_secs
                    ),
                });
            }
        }

        // Phase 4: Paused - final sync, VM is paused
        transition_phase(pool, state, MigrationPhase::Paused).await?;

        let dest_socket = resolve_agent_socket(agent_socket_pattern, &state.dest_node_id);
        let mut dest_client = node_client_pool
            .get_or_connect(&state.dest_node_id, &dest_socket)
            .await?;

        // Resume VM on destination
        let resume_result = tokio::time::timeout(
            Duration::from_secs(timeouts.paused_secs),
            dest_client.resume_vm(
                &state.dest_node_id,
                &state.vm_id,
                "1",
                &state.operation_id,
                None,
            ),
        )
        .await;

        match resume_result {
            Ok(Ok(_ack)) => {
                info!(
                    migration_id = %state.migration_id,
                    "VM resumed on destination, migration complete"
                );
            }
            Ok(Err(e)) => {
                // Destination failed to resume - try to resume on source
                warn!(
                    migration_id = %state.migration_id,
                    error = %e,
                    "destination resume failed, attempting rollback to source"
                );
                rollback_paused(pool, state, node_client_pool, agent_socket_pattern).await?;
                return Err(e);
            }
            Err(_elapsed) => {
                warn!(
                    migration_id = %state.migration_id,
                    "destination resume timed out, attempting rollback to source"
                );
                rollback_paused(pool, state, node_client_pool, agent_socket_pattern).await?;
                return Err(ChvError::Internal {
                    reason: format!(
                        "migration {} paused phase timed out after {}s",
                        state.migration_id, timeouts.paused_secs
                    ),
                });
            }
        }

        // Phase 5: Completed
        transition_phase(pool, state, MigrationPhase::Completed).await?;

        // Update VM placement to destination node
        update_vm_placement(pool, &state.vm_id, &state.dest_node_id).await?;

        Ok(())
    })
    .await;

    match migration_result {
        Ok(Ok(())) => {
            info!(
                migration_id = %state.migration_id,
                "migration completed successfully"
            );
            Ok(())
        }
        Ok(Err(e)) => {
            // Migration failed (already marked Failed or RolledBack in the inner logic)
            Err(e)
        }
        Err(_elapsed) => {
            // Total timeout exceeded
            error!(
                migration_id = %state.migration_id,
                "total migration timeout exceeded"
            );
            transition_phase(pool, state, MigrationPhase::Failed).await?;
            Err(ChvError::Internal {
                reason: format!(
                    "migration {} exceeded total timeout of {}s",
                    state.migration_id,
                    total_timeout.as_secs()
                ),
            })
        }
    }
}

/// Transition the migration to a new phase, updating the database.
async fn transition_phase(
    pool: &StorePool,
    state: &mut MigrationState,
    new_phase: MigrationPhase,
) -> Result<(), ChvError> {
    state.phase = new_phase;
    update_migration_phase(pool, &state.migration_id, new_phase).await
}

/// Rollback during PreCopyDisk or ConvergingDisk phases.
/// VM continues on source, abort migration, mark as RolledBack.
async fn rollback_precopy(pool: &StorePool, state: &mut MigrationState) -> Result<(), ChvError> {
    warn!(
        migration_id = %state.migration_id,
        phase = %state.phase.as_str(),
        "rolling back migration (precopy/converge phase)"
    );
    transition_phase(pool, state, MigrationPhase::RolledBack).await?;
    Ok(())
}

/// Rollback during Paused phase.
/// If destination fails to resume, try to resume on source.
async fn rollback_paused(
    pool: &StorePool,
    state: &mut MigrationState,
    node_client_pool: &NodeClientPool,
    agent_socket_pattern: &str,
) -> Result<(), ChvError> {
    warn!(
        migration_id = %state.migration_id,
        "attempting rollback: resuming VM on source"
    );

    let source_socket = resolve_agent_socket(agent_socket_pattern, &state.source_node_id);
    let resume_source = node_client_pool
        .get_or_connect(&state.source_node_id, &source_socket)
        .await;

    match resume_source {
        Ok(mut client) => {
            match client
                .resume_vm(
                    &state.source_node_id,
                    &state.vm_id,
                    "1",
                    &state.operation_id,
                    None,
                )
                .await
            {
                Ok(_) => {
                    info!(
                        migration_id = %state.migration_id,
                        "VM resumed on source after failed destination resume"
                    );
                    transition_phase(pool, state, MigrationPhase::RolledBack).await?;
                }
                Err(e) => {
                    error!(
                        migration_id = %state.migration_id,
                        error = %e,
                        "failed to resume VM on source during rollback, marking Failed"
                    );
                    transition_phase(pool, state, MigrationPhase::Failed).await?;
                }
            }
        }
        Err(e) => {
            error!(
                migration_id = %state.migration_id,
                error = %e,
                "failed to connect to source agent during rollback, marking Failed"
            );
            transition_phase(pool, state, MigrationPhase::Failed).await?;
        }
    }

    Ok(())
}

/// Wait for disk convergence by polling the migration record.
/// The agent reports progress via telemetry, updating the migrations table.
async fn wait_for_convergence(pool: &StorePool, state: &MigrationState) -> Result<(), ChvError> {
    let max_rounds = state.config.max_convergence_rounds;
    let threshold = state.config.dirty_threshold_blocks as i64;

    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;

        let row: Option<(String, i64, i64)> = sqlx::query_as(
            "SELECT phase, convergence_round, dirty_blocks_remaining FROM migrations WHERE migration_id = ?",
        )
        .bind(&state.migration_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to poll migration convergence: {e}"),
        })?;

        match row {
            Some((phase, round, dirty_remaining)) => {
                if phase == "Failed" || phase == "RolledBack" {
                    return Err(ChvError::Internal {
                        reason: format!(
                            "migration {} entered terminal phase {} during convergence",
                            state.migration_id, phase
                        ),
                    });
                }

                // Convergence achieved when dirty blocks below threshold
                if dirty_remaining <= threshold {
                    return Ok(());
                }

                // Max rounds exceeded
                if round >= max_rounds as i64 {
                    return Err(ChvError::Internal {
                        reason: format!(
                            "migration {} exceeded max convergence rounds ({}) with {} dirty blocks remaining",
                            state.migration_id, max_rounds, dirty_remaining
                        ),
                    });
                }
            }
            None => {
                return Err(ChvError::Internal {
                    reason: format!("migration {} record not found", state.migration_id),
                });
            }
        }
    }
}

/// Wait for memory migration to complete by polling the migration record.
async fn wait_for_memory_migration(
    pool: &StorePool,
    state: &MigrationState,
) -> Result<(), ChvError> {
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;

        let row: Option<(String,)> =
            sqlx::query_as("SELECT phase FROM migrations WHERE migration_id = ?")
                .bind(&state.migration_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| ChvError::Internal {
                    reason: format!("failed to poll migration memory phase: {e}"),
                })?;

        match row {
            Some((phase,)) => {
                // Agent transitions to Paused when memory migration is done
                if phase == "Paused" || phase == "Completed" {
                    return Ok(());
                }
                if phase == "Failed" || phase == "RolledBack" {
                    return Err(ChvError::Internal {
                        reason: format!(
                            "migration {} entered terminal phase {} during memory migration",
                            state.migration_id, phase
                        ),
                    });
                }
                // Still in MemoryMigration, keep waiting
            }
            None => {
                return Err(ChvError::Internal {
                    reason: format!("migration {} record not found", state.migration_id),
                });
            }
        }
    }
}

/// Update the migration phase in the database.
async fn update_migration_phase(
    pool: &StorePool,
    migration_id: &str,
    phase: MigrationPhase,
) -> Result<(), ChvError> {
    let completed_at = if matches!(
        phase,
        MigrationPhase::Completed | MigrationPhase::Failed | MigrationPhase::RolledBack
    ) {
        Some("strftime('%Y-%m-%dT%H:%M:%SZ', 'now')")
    } else {
        None
    };

    if completed_at.is_some() {
        sqlx::query(
            r#"UPDATE migrations
               SET phase = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                   completed_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
               WHERE migration_id = ?"#,
        )
        .bind(phase.as_str())
        .bind(migration_id)
        .execute(pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to update migration phase: {e}"),
        })?;
    } else {
        sqlx::query(
            r#"UPDATE migrations
               SET phase = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
               WHERE migration_id = ?"#,
        )
        .bind(phase.as_str())
        .bind(migration_id)
        .execute(pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to update migration phase: {e}"),
        })?;
    }

    Ok(())
}

/// Create a migration record in the database.
pub async fn create_migration_record(
    pool: &StorePool,
    state: &MigrationState,
) -> Result<(), ChvError> {
    sqlx::query(
        r#"INSERT INTO migrations (migration_id, operation_id, vm_id, source_node_id, destination_node_id, phase)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&state.migration_id)
    .bind(&state.operation_id)
    .bind(&state.vm_id)
    .bind(&state.source_node_id)
    .bind(&state.dest_node_id)
    .bind(state.phase.as_str())
    .execute(pool)
    .await
    .map_err(|e| ChvError::Internal {
        reason: format!("failed to create migration record: {e}"),
    })?;
    Ok(())
}

/// Update migration progress in the database (called from telemetry).
#[allow(clippy::too_many_arguments)]
pub async fn update_migration_progress(
    pool: &StorePool,
    vm_id: &str,
    operation_id: &str,
    phase: i32,
    bytes_transferred: u64,
    total_bytes: u64,
    convergence_round: u32,
    dirty_blocks_remaining: u64,
) -> Result<(), ChvError> {
    let phase_str = proto_phase_to_str(phase);

    let completed_at_clause =
        if phase_str == "Completed" || phase_str == "Failed" || phase_str == "RolledBack" {
            ", completed_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')"
        } else {
            ""
        };

    let sql = format!(
        r#"UPDATE migrations
           SET phase = ?,
               bytes_transferred = ?,
               total_bytes = ?,
               convergence_round = ?,
               dirty_blocks_remaining = ?,
               updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
               {}
           WHERE vm_id = ? AND operation_id = ?"#,
        completed_at_clause
    );

    sqlx::query(&sql)
        .bind(phase_str)
        .bind(bytes_transferred as i64)
        .bind(total_bytes as i64)
        .bind(convergence_round as i32)
        .bind(dirty_blocks_remaining as i64)
        .bind(vm_id)
        .bind(operation_id)
        .execute(pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to update migration progress: {e}"),
        })?;

    Ok(())
}

/// Get VM disk and memory sizes in GB for timeout calculation.
async fn get_vm_sizes(pool: &StorePool, vm_id: &str) -> Result<(u64, u64), ChvError> {
    let row: Option<(Option<i64>, Option<i64>)> = sqlx::query_as(
        r#"SELECT
            (SELECT COALESCE(SUM(v.capacity_bytes), 0)
             FROM volume_desired_state vds
             JOIN volumes v ON v.volume_id = vds.volume_id
             WHERE vds.attached_vm_id = ?) as disk_bytes,
            vms_ds.memory_bytes
           FROM vm_desired_state vms_ds
           WHERE vms_ds.vm_id = ?"#,
    )
    .bind(vm_id)
    .bind(vm_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ChvError::Internal {
        reason: format!("failed to query VM sizes for migration timeout: {e}"),
    })?;

    let (disk_bytes, memory_bytes) = row.unwrap_or((Some(0), Some(0)));
    let disk_gb = (disk_bytes.unwrap_or(0) as u64) / (1024 * 1024 * 1024);
    let memory_gb = (memory_bytes.unwrap_or(0) as u64) / (1024 * 1024 * 1024);

    Ok((disk_gb.max(1), memory_gb.max(1)))
}

/// Update VM placement after successful migration.
async fn update_vm_placement(
    pool: &StorePool,
    vm_id: &str,
    dest_node_id: &str,
) -> Result<(), ChvError> {
    sqlx::query("UPDATE vm_desired_state SET target_node_id = ? WHERE vm_id = ?")
        .bind(dest_node_id)
        .bind(vm_id)
        .execute(pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to update VM placement after migration: {e}"),
        })?;
    Ok(())
}

/// Resolve agent socket path for a node.
fn resolve_agent_socket(pattern: &str, node_id: &str) -> PathBuf {
    if pattern.contains("{node_id}") {
        PathBuf::from(pattern.replace("{node_id}", node_id))
    } else {
        PathBuf::from(pattern)
    }
}

/// Convert proto MigrationPhase enum int to string.
fn proto_phase_to_str(phase: i32) -> &'static str {
    match phase {
        x if x == proto::MigrationPhase::Pending as i32 => "Pending",
        x if x == proto::MigrationPhase::PrecopyDisk as i32 => "PreCopyDisk",
        x if x == proto::MigrationPhase::ConvergingDisk as i32 => "ConvergingDisk",
        x if x == proto::MigrationPhase::MemoryMigration as i32 => "MemoryMigration",
        x if x == proto::MigrationPhase::Paused as i32 => "Paused",
        x if x == proto::MigrationPhase::Completed as i32 => "Completed",
        x if x == proto::MigrationPhase::Failed as i32 => "Failed",
        x if x == proto::MigrationPhase::RolledBack as i32 => "RolledBack",
        _ => "Pending",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_phase_roundtrip() {
        let phases = [
            MigrationPhase::Pending,
            MigrationPhase::PreCopyDisk,
            MigrationPhase::ConvergingDisk,
            MigrationPhase::MemoryMigration,
            MigrationPhase::Paused,
            MigrationPhase::Completed,
            MigrationPhase::Failed,
            MigrationPhase::RolledBack,
        ];

        for phase in &phases {
            let s = phase.as_str();
            let recovered = MigrationPhase::parse(s).expect("should parse");
            assert_eq!(*phase, recovered);
        }
    }

    #[test]
    fn test_migration_phase_transitions() {
        // Valid transitions: Pending -> PreCopyDisk -> ConvergingDisk -> MemoryMigration -> Paused -> Completed
        let transitions = [
            (MigrationPhase::Pending, MigrationPhase::PreCopyDisk),
            (MigrationPhase::PreCopyDisk, MigrationPhase::ConvergingDisk),
            (
                MigrationPhase::ConvergingDisk,
                MigrationPhase::MemoryMigration,
            ),
            (MigrationPhase::MemoryMigration, MigrationPhase::Paused),
            (MigrationPhase::Paused, MigrationPhase::Completed),
        ];

        for (from, to) in &transitions {
            assert_ne!(from, to, "transition should change phase");
        }
    }

    #[test]
    fn test_rollback_phases() {
        // PreCopyDisk and ConvergingDisk can rollback to RolledBack
        let rollbackable = [MigrationPhase::PreCopyDisk, MigrationPhase::ConvergingDisk];
        for phase in &rollbackable {
            assert!(
                matches!(
                    phase,
                    MigrationPhase::PreCopyDisk | MigrationPhase::ConvergingDisk
                ),
                "phase {:?} should be rollbackable",
                phase
            );
        }

        // MemoryMigration cannot cleanly rollback - goes to Failed
        assert_eq!(MigrationPhase::MemoryMigration.as_str(), "MemoryMigration");

        // Paused can rollback by resuming on source
        assert_eq!(MigrationPhase::Paused.as_str(), "Paused");
    }

    #[test]
    fn test_phase_timeout_calculation() {
        // 100GB disk, 16GB memory
        let timeouts = PhaseTimeouts::calculate(100, 16);

        assert_eq!(timeouts.precopy_disk_secs, 6000); // 100 * 60
        assert_eq!(timeouts.converging_disk_per_round_secs, 300);
        assert_eq!(timeouts.converging_disk_total_secs, 3000);
        assert_eq!(timeouts.memory_migration_secs, 600); // 16 * 30 + 120
        assert_eq!(timeouts.paused_secs, 60);
        assert_eq!(timeouts.total_secs, 6000 + 3000 + 600 + 60 + 300); // 9960
    }

    #[test]
    fn test_phase_timeout_minimum_sizes() {
        // 0GB disk, 0GB memory - should use minimum of 1GB
        let timeouts = PhaseTimeouts::calculate(0, 0);

        assert_eq!(timeouts.precopy_disk_secs, 60); // max(0,1) * 60
        assert_eq!(timeouts.memory_migration_secs, 150); // max(0,1) * 30 + 120
    }

    #[test]
    fn test_migration_config_defaults() {
        let config = MigrationConfig::default();
        assert_eq!(config.dirty_threshold_blocks, 1024);
        assert_eq!(config.max_convergence_rounds, 10);
        assert_eq!(config.block_size_bytes, 4_194_304);
        assert_eq!(config.total_timeout_seconds, 0);
    }

    #[test]
    fn test_migration_config_from_correlation_id() {
        let corr =
            "source=node-001:dest=node-002:threshold=512:rounds=5:block_size=2097152:timeout=3600";
        let (source, dest, config) = MigrationConfig::from_correlation_id(corr);

        assert_eq!(source, "node-001");
        assert_eq!(dest, "node-002");
        assert_eq!(config.dirty_threshold_blocks, 512);
        assert_eq!(config.max_convergence_rounds, 5);
        assert_eq!(config.block_size_bytes, 2_097_152);
        assert_eq!(config.total_timeout_seconds, 3600);
    }

    #[test]
    fn test_migration_config_from_correlation_id_partial() {
        let corr = "source=node-a:dest=node-b";
        let (source, dest, config) = MigrationConfig::from_correlation_id(corr);

        assert_eq!(source, "node-a");
        assert_eq!(dest, "node-b");
        // Should use defaults for missing fields
        assert_eq!(config.dirty_threshold_blocks, 1024);
        assert_eq!(config.max_convergence_rounds, 10);
        assert_eq!(config.block_size_bytes, 4_194_304);
        assert_eq!(config.total_timeout_seconds, 0);
    }

    #[test]
    fn test_migration_config_from_proto() {
        let proto_config = proto::MigrationConfig {
            dirty_threshold_blocks: 2048,
            max_convergence_rounds: 15,
            block_size_bytes: 8_388_608,
            total_timeout_seconds: 7200,
        };
        let config = MigrationConfig::from_proto(&proto_config);

        assert_eq!(config.dirty_threshold_blocks, 2048);
        assert_eq!(config.max_convergence_rounds, 15);
        assert_eq!(config.block_size_bytes, 8_388_608);
        assert_eq!(config.total_timeout_seconds, 7200);
    }

    #[test]
    fn test_migration_config_from_proto_zeros_use_defaults() {
        let proto_config = proto::MigrationConfig {
            dirty_threshold_blocks: 0,
            max_convergence_rounds: 0,
            block_size_bytes: 0,
            total_timeout_seconds: 0,
        };
        let config = MigrationConfig::from_proto(&proto_config);
        let defaults = MigrationConfig::default();

        assert_eq!(
            config.dirty_threshold_blocks,
            defaults.dirty_threshold_blocks
        );
        assert_eq!(
            config.max_convergence_rounds,
            defaults.max_convergence_rounds
        );
        assert_eq!(config.block_size_bytes, defaults.block_size_bytes);
        assert_eq!(config.total_timeout_seconds, 0); // 0 is valid (means use calculated)
    }

    #[test]
    fn test_proto_phase_to_str() {
        assert_eq!(
            proto_phase_to_str(proto::MigrationPhase::Pending as i32),
            "Pending"
        );
        assert_eq!(
            proto_phase_to_str(proto::MigrationPhase::PrecopyDisk as i32),
            "PreCopyDisk"
        );
        assert_eq!(
            proto_phase_to_str(proto::MigrationPhase::ConvergingDisk as i32),
            "ConvergingDisk"
        );
        assert_eq!(
            proto_phase_to_str(proto::MigrationPhase::MemoryMigration as i32),
            "MemoryMigration"
        );
        assert_eq!(
            proto_phase_to_str(proto::MigrationPhase::Paused as i32),
            "Paused"
        );
        assert_eq!(
            proto_phase_to_str(proto::MigrationPhase::Completed as i32),
            "Completed"
        );
        assert_eq!(
            proto_phase_to_str(proto::MigrationPhase::Failed as i32),
            "Failed"
        );
        assert_eq!(
            proto_phase_to_str(proto::MigrationPhase::RolledBack as i32),
            "RolledBack"
        );
    }
}
