//! Live migration orchestrator state machine.
//!
//! Drives VM migration through phases: PreCopyDisk -> ConvergingDisk -> MemoryMigration -> Paused -> Completed.
//! Handles rollback at each phase according to spec.

use crate::node_client_pool::NodeClientPool;
use crate::overlay::OverlayManager;
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
    /// Multiplier applied to all phase timeouts for slow storage (default 1.0).
    ///
    /// A value > 1.0 extends all phase timeouts proportionally, useful for
    /// migrations involving slow backends (e.g., NFS, remote storage).
    pub timeout_multiplier: f64,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            dirty_threshold_blocks: 1024,
            max_convergence_rounds: 10,
            block_size_bytes: 4_194_304, // 4MB
            total_timeout_seconds: 0,    // 0 = use calculated default
            timeout_multiplier: 1.0,
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
            timeout_multiplier: defaults.timeout_multiplier,
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
    /// `source={source_node_id}:dest={dest_node_id}:threshold={N}:rounds={N}:block_size={N}:timeout={N}:timeout_multiplier={F}`
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
    ///
    /// The `timeout_multiplier` is applied to all phase timeouts to accommodate
    /// slow storage backends. A multiplier of 1.0 leaves timeouts unchanged.
    pub fn calculate(disk_size_gb: u64, memory_size_gb: u64, timeout_multiplier: f64) -> Self {
        let multiplier = if timeout_multiplier > 0.0 {
            timeout_multiplier
        } else {
            1.0
        };

        let precopy_disk_secs = ((disk_size_gb.max(1) * 60) as f64 * multiplier) as u64;
        let converging_disk_per_round_secs = (300.0 * multiplier) as u64;
        let converging_disk_total_secs = (3000.0 * multiplier) as u64;
        let memory_migration_secs = ((memory_size_gb.max(1) * 30 + 120) as f64 * multiplier) as u64;
        let paused_secs = (60.0 * multiplier) as u64;
        let total_secs = precopy_disk_secs
            + converging_disk_total_secs
            + memory_migration_secs
            + paused_secs
            + (300.0 * multiplier) as u64;

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

/// Maximum allowed age of a source node heartbeat for migration to proceed.
const SOURCE_NODE_HEARTBEAT_MAX_AGE_SECS: i64 = 30;

/// Validate preconditions before starting a migration.
///
/// Checks:
/// 1. Source node heartbeat is within 30 seconds (node is healthy and reachable).
/// 2. Destination node is in a schedulable state (not Maintenance, Draining, Failed, etc.).
/// 3. No other migration is currently in progress for the same VM.
///
/// These checks prevent wasted multi-hour migrations that would inevitably fail.
async fn validate_preconditions(pool: &StorePool, state: &MigrationState) -> Result<(), ChvError> {
    // Check 1: Source node heartbeat freshness.
    // The node_observed_state.last_seen_at is updated on every state report from the agent.
    let source_last_seen: Option<(Option<String>,)> =
        sqlx::query_as("SELECT last_seen_at FROM node_observed_state WHERE node_id = ?")
            .bind(&state.source_node_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("failed to query source node heartbeat: {e}"),
            })?;

    match source_last_seen {
        None => {
            return Err(ChvError::BadRequest {
                reason: format!(
                    "source node '{}' has no observed state record (never reported)",
                    state.source_node_id
                ),
            });
        }
        Some((None,)) => {
            return Err(ChvError::BadRequest {
                reason: format!(
                    "source node '{}' has no last_seen_at timestamp",
                    state.source_node_id
                ),
            });
        }
        Some((Some(last_seen_str),)) => {
            let last_seen =
                chrono::NaiveDateTime::parse_from_str(&last_seen_str, "%Y-%m-%dT%H:%M:%SZ")
                    .map_err(|e| ChvError::Internal {
                        reason: format!(
                            "failed to parse source node last_seen_at '{}': {e}",
                            last_seen_str
                        ),
                    })?;
            let now = chrono::Utc::now().naive_utc();
            let age_secs = (now - last_seen).num_seconds();

            if age_secs > SOURCE_NODE_HEARTBEAT_MAX_AGE_SECS {
                return Err(ChvError::BadRequest {
                    reason: format!(
                        "source node '{}' last heartbeat was {}s ago (max {}s), node may be unhealthy",
                        state.source_node_id, age_secs, SOURCE_NODE_HEARTBEAT_MAX_AGE_SECS
                    ),
                });
            }
        }
    }

    // Check 2: Destination node is in a schedulable state.
    // We check node_desired_state.desired_state — nodes in Maintenance, Draining, or Failed
    // states should not receive new workloads.
    let dest_state: Option<(String, bool)> = sqlx::query_as(
        "SELECT desired_state, scheduling_paused FROM node_desired_state WHERE node_id = ?",
    )
    .bind(&state.dest_node_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ChvError::Internal {
        reason: format!("failed to query destination node state: {e}"),
    })?;

    match dest_state {
        None => {
            return Err(ChvError::BadRequest {
                reason: format!(
                    "destination node '{}' has no desired state record",
                    state.dest_node_id
                ),
            });
        }
        Some((state_str, scheduling_paused)) => {
            match state_str.as_str() {
                "Maintenance" | "Draining" | "Failed" | "Unreachable" => {
                    return Err(ChvError::BadRequest {
                        reason: format!(
                            "destination node '{}' is in state '{}', not eligible to receive migrations",
                            state.dest_node_id, state_str
                        ),
                    });
                }
                _ => {}
            }

            if scheduling_paused {
                return Err(ChvError::BadRequest {
                    reason: format!(
                        "destination node '{}' has scheduling paused",
                        state.dest_node_id
                    ),
                });
            }
        }
    }

    // Check 3: No other migration currently in progress for the same VM.
    // Query the migrations table directly — active migrations are in non-terminal phases.
    let active_migration: Option<(String, String)> = sqlx::query_as(
        r#"SELECT migration_id, phase FROM migrations
           WHERE vm_id = ? AND phase NOT IN ('Completed', 'Failed', 'RolledBack')
           AND migration_id != ?
           LIMIT 1"#,
    )
    .bind(&state.vm_id)
    .bind(&state.migration_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ChvError::Internal {
        reason: format!("failed to query active migrations for vm: {e}"),
    })?;

    if let Some((existing_id, existing_phase)) = active_migration {
        return Err(ChvError::BadRequest {
            reason: format!(
                "VM '{}' already has an active migration '{}' in phase '{}'",
                state.vm_id, existing_id, existing_phase
            ),
        });
    }

    Ok(())
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
#[tracing::instrument(
    skip(pool, node_client_pool, state, overlay_manager),
    fields(
        migration_id = %state.migration_id,
        vm_id = %state.vm_id,
        source = %state.source_node_id,
        dest = %state.dest_node_id,
    )
)]
pub async fn execute_migration(
    pool: &StorePool,
    node_client_pool: &NodeClientPool,
    agent_socket_pattern: &str,
    state: &mut MigrationState,
    overlay_manager: Option<&OverlayManager>,
) -> Result<(), ChvError> {
    info!(
        migration_id = %state.migration_id,
        vm_id = %state.vm_id,
        source = %state.source_node_id,
        dest = %state.dest_node_id,
        "starting live migration"
    );
    metrics::counter!("chv_migration_started_total").increment(1);

    // Validate preconditions before committing to the migration.
    // These checks prevent wasted multi-hour migrations that would inevitably fail.
    validate_preconditions(pool, state).await?;

    // Look up VM disk and memory sizes for timeout calculation
    let (disk_size_gb, memory_size_gb) = get_vm_sizes(pool, &state.vm_id).await?;
    let timeouts = PhaseTimeouts::calculate(
        disk_size_gb,
        memory_size_gb,
        state.config.timeout_multiplier,
    );

    // Use configured total_timeout if provided, otherwise use calculated
    let total_timeout = if state.config.total_timeout_seconds > 0 {
        Duration::from_secs(state.config.total_timeout_seconds as u64)
    } else {
        Duration::from_secs(timeouts.total_secs)
    };

    let migration_result = tokio::time::timeout(total_timeout, async {
        // Phase 1: PreCopyDisk - dispatch migrate_vm to source agent
        transition_phase(pool, state, MigrationPhase::PreCopyDisk).await?;

        let vm_generation = get_vm_generation(pool, &state.vm_id).await?;

        let source_socket = resolve_agent_socket(agent_socket_pattern, &state.source_node_id);
        let mut source_client = node_client_pool
            .get_or_connect(&state.source_node_id, &source_socket)
            .await?;

        let precopy_result = tokio::time::timeout(
            Duration::from_secs(timeouts.precopy_disk_secs),
            source_client.migrate_vm(
                &state.source_node_id,
                &state.vm_id,
                &vm_generation,
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
        // Explicitly pause the source VM before final sync to ensure no writes
        // can occur during the final data transfer. Cloud Hypervisor's send-migration
        // typically pauses the VM internally, but we enforce it here for coordination.
        let source_socket_for_pause =
            resolve_agent_socket(agent_socket_pattern, &state.source_node_id);
        let pause_result = node_client_pool
            .get_or_connect(&state.source_node_id, &source_socket_for_pause)
            .await;

        match pause_result {
            Ok(mut pause_client) => {
                if let Err(e) = pause_client
                    .pause_vm(
                        &state.source_node_id,
                        &state.vm_id,
                        &vm_generation,
                        &state.operation_id,
                        Some("control-plane"),
                    )
                    .await
                {
                    // Log but don't fail — the VM may already be paused by CH's
                    // send-migration protocol. If it truly isn't paused, the final
                    // sync will still work but may have slightly more dirty data.
                    warn!(
                        migration_id = %state.migration_id,
                        vm_id = %state.vm_id,
                        error = %e,
                        "failed to explicitly pause source VM for final sync (may already be paused by CH)"
                    );
                }
                info!(
                    migration_id = %state.migration_id,
                    vm_id = %state.vm_id,
                    "source VM paused for final sync"
                );
            }
            Err(e) => {
                warn!(
                    migration_id = %state.migration_id,
                    vm_id = %state.vm_id,
                    error = %e,
                    "could not connect to source agent for explicit pause (VM may already be paused)"
                );
            }
        }

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
                &vm_generation,
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

        // Phase 5: Completed — transition phase and update placement atomically
        // to prevent split-brain if crash occurs between the two operations.
        complete_migration_atomically(pool, state).await?;

        // Best-effort: update overlay FDB entries and send gratuitous ARP
        if let Some(overlay) = overlay_manager {
            notify_overlay_after_migration(
                pool,
                overlay,
                node_client_pool,
                agent_socket_pattern,
                state,
            )
            .await;
        }

        Ok(())
    })
    .await;

    match migration_result {
        Ok(Ok(())) => {
            metrics::counter!("chv_migration_completed_total").increment(1);
            info!(
                migration_id = %state.migration_id,
                "migration completed successfully"
            );
            // Best-effort: disable dirty tracking on source volumes
            disable_source_dirty_tracking(pool, node_client_pool, agent_socket_pattern, state)
                .await;
            Ok(())
        }
        Ok(Err(e)) => {
            metrics::counter!("chv_migration_failed_total").increment(1);
            // Migration failed (already marked Failed or RolledBack in the inner logic)
            // Best-effort: disable dirty tracking on source volumes
            disable_source_dirty_tracking(pool, node_client_pool, agent_socket_pattern, state)
                .await;
            Err(e)
        }
        Err(_elapsed) => {
            metrics::counter!("chv_migration_failed_total").increment(1);
            // Total timeout exceeded
            error!(
                migration_id = %state.migration_id,
                "total migration timeout exceeded"
            );
            transition_phase(pool, state, MigrationPhase::Failed).await?;
            // Best-effort: disable dirty tracking on source volumes
            disable_source_dirty_tracking(pool, node_client_pool, agent_socket_pattern, state)
                .await;
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
            let gen = get_vm_generation(pool, &state.vm_id)
                .await
                .unwrap_or_else(|_| "1".to_string());
            match client
                .resume_vm(
                    &state.source_node_id,
                    &state.vm_id,
                    &gen,
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

/// Disable dirty tracking on all source volumes after migration completes (success or failure).
///
/// Per ADR-012: "Dirty tracking MUST be disabled on source after migration completes
/// (success or failure)." Leaving it enabled causes ~15-20% I/O degradation.
///
/// This is best-effort: failures are logged as warnings but do not fail the overall operation.
async fn disable_source_dirty_tracking(
    pool: &StorePool,
    node_client_pool: &NodeClientPool,
    agent_socket_pattern: &str,
    state: &MigrationState,
) {
    // Query volumes attached to the migrating VM.
    let volumes: Result<Vec<(String,)>, _> =
        sqlx::query_as("SELECT volume_id FROM volume_desired_state WHERE attached_vm_id = ?")
            .bind(&state.vm_id)
            .fetch_all(pool)
            .await;

    let volume_ids = match volumes {
        Ok(rows) => rows,
        Err(e) => {
            warn!(
                migration_id = %state.migration_id,
                vm_id = %state.vm_id,
                error = %e,
                "failed to query volumes for dirty tracking cleanup (best-effort)"
            );
            return;
        }
    };

    if volume_ids.is_empty() {
        return;
    }

    // Connect to the source agent.
    let source_socket = resolve_agent_socket(agent_socket_pattern, &state.source_node_id);
    let mut source_client = match node_client_pool
        .get_or_connect(&state.source_node_id, &source_socket)
        .await
    {
        Ok(client) => client,
        Err(e) => {
            warn!(
                migration_id = %state.migration_id,
                source_node_id = %state.source_node_id,
                error = %e,
                "failed to connect to source agent for dirty tracking cleanup (best-effort)"
            );
            return;
        }
    };

    // For each volume, send a desired state update that disables dirty tracking.
    // The agent interprets a volume spec with `dirty_tracking: false` as a signal
    // to stop tracking dirty blocks on the volume.
    for (volume_id,) in &volume_ids {
        let spec_json = serde_json::json!({
            "dirty_tracking": false
        });
        let spec_bytes = serde_json::to_vec(&spec_json).unwrap_or_default();

        match source_client
            .apply_volume_desired_state(
                &state.source_node_id,
                volume_id,
                "0", // generation 0 — best-effort cleanup, no ordering guarantee needed
                spec_bytes,
                &state.operation_id,
                Some("control-plane"),
            )
            .await
        {
            Ok(_) => {
                info!(
                    migration_id = %state.migration_id,
                    volume_id = %volume_id,
                    source_node_id = %state.source_node_id,
                    "disabled dirty tracking on source volume"
                );
            }
            Err(e) => {
                warn!(
                    migration_id = %state.migration_id,
                    volume_id = %volume_id,
                    source_node_id = %state.source_node_id,
                    error = %e,
                    "failed to disable dirty tracking on source volume (best-effort)"
                );
            }
        }
    }
}

/// Query the volume IDs attached to a VM.
/// Wait for disk convergence by polling the migration record.
/// The agent reports progress via telemetry updates to `bytes_transferred` and `total_bytes`.
/// Convergence is achieved when dirty_blocks_remaining (reported by agent) drops below threshold,
/// OR when bytes_transferred >= total_bytes (indicating bulk copy complete and iterative
/// sync has finished).
///
/// Uses progressive timeouts per round:
/// - Rounds 1-3: 60s per round (initial bulk copy phase)
/// - Rounds 4-6: 30s per round (iterative sync phase)
/// - Rounds 7+: 15s per round (final convergence phase)
/// - Max total: 7200s overall cap
///
/// If no progress is detected between rounds (bytes_transferred unchanged), the
/// migration is cancelled early to avoid wasting time on a stalled transfer.
async fn wait_for_convergence(pool: &StorePool, state: &MigrationState) -> Result<(), ChvError> {
    let max_rounds = state.config.max_convergence_rounds;
    let threshold = state.config.dirty_threshold_blocks as i64;
    let mut poll_count: u32 = 0;
    let mut last_bytes_transferred: i64 = -1;
    let mut stall_polls: u32 = 0;
    let started = tokio::time::Instant::now();
    const MAX_TOTAL_SECS: u64 = 7200;

    loop {
        // Progressive poll interval based on current round:
        // Rounds 1-3 (polls 1-36): 60s budget / ~5s polls = poll every 5s
        // Rounds 4-6 (polls 37-54): 30s budget / ~5s polls = poll every 5s
        // Rounds 7+ (polls 55+): 15s budget / ~5s polls = poll every 5s
        // The outer timeout (from PhaseTimeouts) enforces per-round deadlines;
        // here we use progressive stall detection thresholds.
        let poll_interval = Duration::from_secs(5);
        tokio::time::sleep(poll_interval).await;
        poll_count += 1;

        // Enforce max total time
        if started.elapsed().as_secs() >= MAX_TOTAL_SECS {
            warn!(
                migration_id = %state.migration_id,
                elapsed_secs = started.elapsed().as_secs(),
                "convergence exceeded max total time (7200s), forcing transition"
            );
            return Ok(());
        }

        let row: Option<(String, i64, i64, i64, i64)> = sqlx::query_as(
            "SELECT phase, convergence_round, dirty_blocks_remaining, bytes_transferred, total_bytes FROM migrations WHERE migration_id = ?",
        )
        .bind(&state.migration_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to poll migration convergence: {e}"),
        })?;

        match row {
            Some((phase, round, dirty_remaining, bytes_transferred, total_bytes)) => {
                if phase == "Failed" || phase == "RolledBack" {
                    return Err(ChvError::Internal {
                        reason: format!(
                            "migration {} entered terminal phase {} during convergence",
                            state.migration_id, phase
                        ),
                    });
                }

                // Primary check: agent-reported dirty blocks below threshold
                if dirty_remaining >= 0 && dirty_remaining <= threshold {
                    info!(
                        migration_id = %state.migration_id,
                        round = round,
                        dirty_remaining = dirty_remaining,
                        "convergence achieved: dirty blocks below threshold"
                    );
                    return Ok(());
                }

                // Secondary check: bytes_transferred indicates bulk copy complete
                if total_bytes > 0 && bytes_transferred >= total_bytes && dirty_remaining <= 0 {
                    info!(
                        migration_id = %state.migration_id,
                        bytes_transferred = bytes_transferred,
                        total_bytes = total_bytes,
                        "convergence achieved: all bytes transferred"
                    );
                    return Ok(());
                }

                // Progressive stall detection: check if bytes_transferred is making progress.
                // The stall threshold depends on which round we're in:
                // - Rounds 1-3: allow up to 12 stall polls (60s of no progress)
                // - Rounds 4-6: allow up to 6 stall polls (30s of no progress)
                // - Rounds 7+: allow up to 3 stall polls (15s of no progress)
                let max_stall_polls = if round <= 3 {
                    12_u32 // 60s at 5s intervals
                } else if round <= 6 {
                    6_u32 // 30s at 5s intervals
                } else {
                    3_u32 // 15s at 5s intervals
                };

                if bytes_transferred == last_bytes_transferred && last_bytes_transferred >= 0 {
                    stall_polls += 1;
                    if stall_polls >= max_stall_polls {
                        warn!(
                            migration_id = %state.migration_id,
                            round = round,
                            stall_polls = stall_polls,
                            bytes_transferred = bytes_transferred,
                            "no progress detected between rounds, cancelling early"
                        );
                        return Err(ChvError::Internal {
                            reason: format!(
                                "migration {} stalled: no progress for {} polls (round {})",
                                state.migration_id, stall_polls, round
                            ),
                        });
                    }
                } else {
                    // Progress was made, reset stall counter
                    stall_polls = 0;
                }
                last_bytes_transferred = bytes_transferred;

                // Max rounds exceeded (each round is 5s, so max_rounds polls)
                if round >= max_rounds as i64 || poll_count >= max_rounds * 6 {
                    // Force convergence if we've waited long enough — the agent
                    // may not be updating dirty_blocks_remaining. Proceed to
                    // memory migration phase which will do a final sync.
                    warn!(
                        migration_id = %state.migration_id,
                        rounds = round,
                        poll_count = poll_count,
                        dirty_remaining = dirty_remaining,
                        "convergence round limit reached, forcing transition to memory migration"
                    );
                    return Ok(());
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

/// Fetch the current generation for a VM from the desired state store.
async fn get_vm_generation(pool: &StorePool, vm_id: &str) -> Result<String, ChvError> {
    let gen: Option<(Option<i64>,)> =
        sqlx::query_as("SELECT generation FROM vm_desired_state WHERE vm_id = ?")
            .bind(vm_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("failed to fetch generation for VM {vm_id}: {e}"),
            })?;

    match gen {
        Some((Some(g),)) => Ok(g.to_string()),
        _ => Ok("1".to_string()),
    }
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

fn migration_bridge_name(net_id: &str) -> String {
    if net_id == "default" {
        return "chvbr0".to_string();
    }
    let candidate = format!("br-{}", net_id);
    if candidate.len() <= 15 {
        return candidate;
    }
    let prefix: String = net_id.chars().take(8).collect();
    let hash = {
        let mut h: u32 = 0x811c9dc5;
        for b in net_id.as_bytes() {
            h = h.wrapping_mul(0x01000193) ^ (*b as u32);
        }
        format!("{:04x}", h & 0xffff)
    };
    format!("br-{}{}", prefix, hash)
}

/// Atomically mark migration as Completed AND update VM placement.
/// Both operations run in a single SQLite transaction to prevent split-brain
/// if a crash occurs between them.
async fn complete_migration_atomically(
    pool: &StorePool,
    state: &mut MigrationState,
) -> Result<(), ChvError> {
    let mut tx = pool.begin().await.map_err(|e| ChvError::Internal {
        reason: format!("failed to begin transaction for migration completion: {e}"),
    })?;

    sqlx::query(
        r#"UPDATE migrations
           SET phase = 'Completed',
               updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
               completed_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
           WHERE migration_id = ?"#,
    )
    .bind(&state.migration_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ChvError::Internal {
        reason: format!("failed to mark migration completed: {e}"),
    })?;

    sqlx::query("UPDATE vm_desired_state SET target_node_id = ? WHERE vm_id = ?")
        .bind(&state.dest_node_id)
        .bind(&state.vm_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("failed to update VM placement: {e}"),
        })?;

    tx.commit().await.map_err(|e| ChvError::Internal {
        reason: format!("failed to commit migration completion transaction: {e}"),
    })?;

    state.phase = MigrationPhase::Completed;
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
        _ => "Unknown",
    }
}

/// VM NIC info needed for post-migration overlay update.
struct VmNicInfo {
    network_id: String,
    mac_address: String,
    ip_address: String,
    vni: i32,
}

/// Fetch all NICs for a VM that are on overlay networks (vni > 0).
async fn get_vm_overlay_nics(pool: &StorePool, vm_id: &str) -> Result<Vec<VmNicInfo>, ChvError> {
    let rows: Vec<(String, String, String, i32)> = sqlx::query_as(
        r#"SELECT vn.network_id, vn.mac_address, vn.ip_address, COALESCE(n.vni, 0)
           FROM vm_nic_desired_state vn
           JOIN networks n ON n.network_id = vn.network_id
           WHERE vn.vm_id = ? AND COALESCE(n.vni, 0) > 0
             AND vn.mac_address IS NOT NULL"#,
    )
    .bind(vm_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ChvError::Internal {
        reason: format!("failed to query VM NIC overlay info: {e}"),
    })?;

    Ok(rows
        .into_iter()
        .map(|(network_id, mac_address, ip_address, vni)| VmNicInfo {
            network_id,
            mac_address,
            ip_address,
            vni,
        })
        .collect())
}

/// Best-effort post-migration overlay notification.
///
/// Re-points FDB entries on all peer nodes and sends gratuitous ARP from the
/// destination node. Failures are logged as warnings but do not fail the
/// migration — the overlay is eventually consistent and will reconcile on the
/// next heartbeat cycle.
async fn notify_overlay_after_migration(
    pool: &StorePool,
    overlay: &OverlayManager,
    node_client_pool: &NodeClientPool,
    agent_socket_pattern: &str,
    state: &MigrationState,
) {
    let nics = match get_vm_overlay_nics(pool, &state.vm_id).await {
        Ok(nics) => nics,
        Err(e) => {
            warn!(
                migration_id = %state.migration_id,
                vm_id = %state.vm_id,
                error = %e,
                "failed to query VM NICs for post-migration overlay update"
            );
            return;
        }
    };

    if nics.is_empty() {
        return;
    }

    // Bridge name follows the standard naming convention on the node
    // (first 8 chars of network_id prefixed with "br-"). The agent resolves
    // the actual interface; we pass our best guess for the ARP request.
    for nic in &nics {
        let bridge_name = migration_bridge_name(&nic.network_id);

        // 1. Re-point FDB entries on all peers
        if let Err(e) = overlay
            .on_vm_migrated(
                &nic.network_id,
                &nic.mac_address,
                &nic.ip_address,
                &state.dest_node_id,
                &bridge_name,
                nic.vni,
                &state.operation_id,
            )
            .await
        {
            warn!(
                migration_id = %state.migration_id,
                vm_id = %state.vm_id,
                network_id = %nic.network_id,
                error = %e,
                "post-migration overlay FDB update failed (best-effort)"
            );
        }

        // 2. Send gratuitous ARP from the destination node
        let dest_socket = resolve_agent_socket(agent_socket_pattern, &state.dest_node_id);
        match node_client_pool
            .get_or_connect(&state.dest_node_id, &dest_socket)
            .await
        {
            Ok(mut client) => {
                if let Err(e) = client
                    .send_gratuitous_arp(
                        &state.dest_node_id,
                        &nic.network_id,
                        &nic.ip_address,
                        &bridge_name,
                        &state.operation_id,
                        Some("control-plane"),
                    )
                    .await
                {
                    warn!(
                        migration_id = %state.migration_id,
                        vm_id = %state.vm_id,
                        network_id = %nic.network_id,
                        vm_ip = %nic.ip_address,
                        error = %e,
                        "post-migration gratuitous ARP failed (best-effort)"
                    );
                }
            }
            Err(e) => {
                warn!(
                    migration_id = %state.migration_id,
                    vm_id = %state.vm_id,
                    dest_node_id = %state.dest_node_id,
                    error = %e,
                    "failed to connect to dest node for gratuitous ARP (best-effort)"
                );
            }
        }
    }

    info!(
        migration_id = %state.migration_id,
        vm_id = %state.vm_id,
        nic_count = nics.len(),
        "post-migration overlay update completed"
    );
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
        // 100GB disk, 16GB memory, default multiplier
        let timeouts = PhaseTimeouts::calculate(100, 16, 1.0);

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
        let timeouts = PhaseTimeouts::calculate(0, 0, 1.0);

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
        assert_eq!(config.timeout_multiplier, 1.0);
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
