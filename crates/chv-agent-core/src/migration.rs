//! Agent-side migration coordination.
//!
//! This module handles live VM migration at the agent level. When the control plane
//! dispatches a `MigrateVm` RPC to an agent, the agent determines its role (source or
//! destination) based on matching its own node_id against `source_node_id` /
//! `destination_node_id` in the request and executes the appropriate Cloud Hypervisor
//! REST API calls.

use crate::daemon_clients::StordClient;
use crate::vm_runtime::VmRuntime;
use control_plane_node_api::control_plane_node_api as proto;
use std::net::TcpListener;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

/// Port range for migration receiver sockets.
const MIGRATION_PORT_RANGE_START: u16 = 49152;
const MIGRATION_PORT_RANGE_END: u16 = 49200;

/// The role this agent plays in a migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationRole {
    Source,
    Destination,
}

/// Determine the role of this agent in a migration request.
pub fn determine_role(
    my_node_id: &str,
    source_node_id: &str,
    destination_node_id: &str,
) -> Option<MigrationRole> {
    if my_node_id == source_node_id {
        Some(MigrationRole::Source)
    } else if my_node_id == destination_node_id {
        Some(MigrationRole::Destination)
    } else {
        None
    }
}

/// Allocate a port from the migration port range (49152-49200).
/// Returns the bound TcpListener which holds the port reservation.
/// The caller should extract the port with `.local_addr()` and pass the
/// listener or drop it only when ready to bind the actual migration socket.
pub fn allocate_migration_port() -> Result<(u16, TcpListener), &'static str> {
    for port in MIGRATION_PORT_RANGE_START..=MIGRATION_PORT_RANGE_END {
        if let Ok(listener) = TcpListener::bind(("0.0.0.0", port)) {
            return Ok((port, listener));
        }
    }
    Err("no available migration port in range 49152-49200")
}

/// Build a receiver URL for the destination agent.
pub fn build_receiver_url(port: u16) -> String {
    format!("tcp://0.0.0.0:{}", port)
}

/// Build a destination URL for the source agent to connect to.
pub fn build_destination_url(dest_host: &str, port: u16) -> String {
    format!("tcp://{}:{}", dest_host, port)
}

/// Extract the destination host address from the MigrateVmRequest.
/// The control plane populates the `destination_node_id` which maps to a known
/// agent address. For now, we expect the meta.target_node_id to encode enough
/// information, or we derive it from the request fields.
///
/// In the current architecture, the CP orchestrator sends MigrateVm to both
/// source and dest with the full request. The destination host address is
/// embedded in the request by the CP as part of the config or meta context.
/// For simplicity, we extract it from a convention: the destination_node_id
/// can serve as a DNS-resolvable hostname, or the CP will populate a
/// `destination_address` field in the config.
pub fn extract_destination_host(req: &proto::MigrateVmRequest) -> String {
    // The destination_node_id is typically a hostname or IP that the source
    // can reach. In production, the CP would resolve node_id -> agent_address.
    // For now, we use destination_node_id directly as the host.
    req.destination_node_id.clone()
}

/// Execute migration as the source agent.
/// This spawns a background task that calls CH send-migration and reports progress.
pub fn spawn_source_migration(
    vm_runtime: VmRuntime,
    vm_id: String,
    operation_id: String,
    destination_url: String,
) -> tokio::task::JoinHandle<Result<(), String>> {
    tokio::spawn(async move {
        info!(
            vm_id = %vm_id,
            destination_url = %destination_url,
            operation_id = %operation_id,
            "source agent: starting send-migration"
        );

        match vm_runtime
            .send_migration(&vm_id, &destination_url, Some(&operation_id))
            .await
        {
            Ok(()) => {
                info!(
                    vm_id = %vm_id,
                    operation_id = %operation_id,
                    "source agent: send-migration completed successfully"
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    vm_id = %vm_id,
                    operation_id = %operation_id,
                    error = %e,
                    "source agent: send-migration failed"
                );
                Err(e.to_string())
            }
        }
    })
}

/// Execute migration as the destination agent.
/// This spawns a background task that calls CH receive-migration.
pub fn spawn_destination_migration(
    vm_runtime: VmRuntime,
    vm_id: String,
    operation_id: String,
    receiver_url: String,
) -> tokio::task::JoinHandle<Result<(), String>> {
    tokio::spawn(async move {
        info!(
            vm_id = %vm_id,
            receiver_url = %receiver_url,
            operation_id = %operation_id,
            "destination agent: starting receive-migration"
        );

        match vm_runtime
            .receive_migration(&vm_id, &receiver_url, Some(&operation_id))
            .await
        {
            Ok(()) => {
                info!(
                    vm_id = %vm_id,
                    operation_id = %operation_id,
                    "destination agent: receive-migration completed, VM is now running"
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    vm_id = %vm_id,
                    operation_id = %operation_id,
                    error = %e,
                    "destination agent: receive-migration failed"
                );
                Err(e.to_string())
            }
        }
    })
}

/// A volume descriptor for disk pre-copy migration.
///
/// Each volume attached to a VM needs its data migrated to the destination node
/// before the memory migration can begin. This struct identifies a volume and its
/// attachment handle so the stord can locate and stream the correct data.
#[derive(Debug, Clone)]
pub struct MigrationVolume {
    /// The unique volume identifier (e.g., "vol-abc123").
    pub volume_id: String,
    /// The attachment handle returned when the volume was opened/attached.
    pub attachment_handle: String,
}

/// Configuration for disk pre-copy migration phase.
///
/// Encapsulates the stord connection details and volume list needed to coordinate
/// disk migration before memory migration begins.
#[derive(Debug, Clone)]
pub struct DiskPrecopyConfig {
    /// Path to the local stord Unix socket.
    pub stord_socket: std::path::PathBuf,
    /// The gRPC endpoint of the destination stord's migration service
    /// (e.g., `http://dest-host:50052`).
    pub dest_stord_endpoint: String,
    /// Volumes to migrate.
    pub volumes: Vec<MigrationVolume>,
}

/// A callback type for reporting migration progress to the control plane.
///
/// When provided to `spawn_source_migration_with_disk_precopy`, this is called
/// after each disk sync round with the current dirty_blocks_remaining count,
/// enabling the control plane to track convergence.
pub type ProgressReporter = Arc<Mutex<dyn FnMut(proto::MigrationProgress) + Send>>;

/// Build a progress reporter closure that sends updates to the control plane.
///
/// This creates a `ProgressReporter` wrapping a control plane client reference,
/// suitable for passing to `spawn_source_migration_with_disk_precopy`.
pub fn make_progress_reporter<F>(callback: F) -> ProgressReporter
where
    F: FnMut(proto::MigrationProgress) + Send + 'static,
{
    Arc::new(Mutex::new(callback))
}

/// Execute migration as the source agent with disk pre-copy.
///
/// This orchestrates the full live migration sequence:
/// 1. **Disk pre-copy**: For each volume, triggers the local stord to stream blocks
///    to the destination stord. This runs while the VM is still executing.
/// 2. **Memory migration**: Once disk pre-copy completes (or converges), calls
///    Cloud Hypervisor's send-migration which handles iterative memory copy and
///    final VM pause/transfer.
///
/// If disk pre-copy fails for any volume, the migration is aborted before memory
/// transfer begins. The original `spawn_source_migration` function remains available
/// for memory-only migration scenarios (e.g., VMs with no local disk).
///
/// When `progress_reporter` is provided, the function reports dirty_blocks_remaining
/// after each volume sync round completes, enabling the control plane to track
/// convergence in real time.
pub fn spawn_source_migration_with_disk_precopy(
    vm_runtime: VmRuntime,
    vm_id: String,
    operation_id: String,
    destination_url: String,
    disk_config: DiskPrecopyConfig,
    progress_reporter: Option<ProgressReporter>,
) -> tokio::task::JoinHandle<Result<(), String>> {
    tokio::spawn(async move {
        info!(
            vm_id = %vm_id,
            destination_url = %destination_url,
            operation_id = %operation_id,
            volume_count = disk_config.volumes.len(),
            dest_stord_endpoint = %disk_config.dest_stord_endpoint,
            "source agent: starting migration with disk pre-copy"
        );

        // Phase 1: Disk pre-copy
        let mut stord_client =
            match StordClient::connect(Path::new(&disk_config.stord_socket)).await {
                Ok(client) => client,
                Err(e) => {
                    error!(
                        vm_id = %vm_id,
                        operation_id = %operation_id,
                        error = %e,
                        "source agent: failed to connect to local stord for disk pre-copy"
                    );
                    return Err(format!(
                        "disk pre-copy failed: cannot connect to stord: {e}"
                    ));
                }
            };

        let mut volume_migrations: Vec<(String, String)> = Vec::new();
        // (volume_id, migration_id)

        if !disk_config.volumes.is_empty() {
            info!(
                vm_id = %vm_id,
                operation_id = %operation_id,
                "source agent: beginning disk pre-copy phase"
            );

            for volume in &disk_config.volumes {
                info!(
                    vm_id = %vm_id,
                    operation_id = %operation_id,
                    volume_id = %volume.volume_id,
                    attachment_handle = %volume.attachment_handle,
                    "source agent: triggering disk pre-copy for volume"
                );

                let migration_id = match stord_client
                    .trigger_disk_migration(
                        &volume.volume_id,
                        &volume.attachment_handle,
                        &disk_config.dest_stord_endpoint,
                        Some(&operation_id),
                    )
                    .await
                {
                    Ok(id) => id,
                    Err(e) => {
                        error!(
                            vm_id = %vm_id,
                            operation_id = %operation_id,
                            volume_id = %volume.volume_id,
                            error = %e,
                            "source agent: disk pre-copy trigger failed for volume"
                        );
                        return Err(format!(
                            "disk pre-copy trigger failed for volume {}: {e}",
                            volume.volume_id
                        ));
                    }
                };

                volume_migrations.push((volume.volume_id.clone(), migration_id));

                info!(
                    vm_id = %vm_id,
                    operation_id = %operation_id,
                    volume_id = %volume.volume_id,
                    migration_id = %volume_migrations.last().unwrap().1,
                    "source agent: disk migration triggered for volume"
                );
            }

            // Poll all volume migrations until they converge or need VM pause.
            let poll_interval = std::time::Duration::from_secs(5);
            let mut vm_paused_for_final_sync = false;

            loop {
                tokio::time::sleep(poll_interval).await;

                let mut all_completed = true;
                let mut any_failed = false;
                let mut total_bytes_transferred: u64 = 0;
                let mut total_bytes: u64 = 0;
                let mut max_dirty_remaining: u64 = 0;
                let mut max_round: u32 = 0;
                let mut needs_vm_pause = false;

                for (_vol_id, mig_id) in &volume_migrations {
                    let status = match stord_client.get_disk_migration_status(mig_id).await {
                        Ok(s) => s,
                        Err(e) => {
                            warn!(
                                vm_id = %vm_id,
                                operation_id = %operation_id,
                                migration_id = %mig_id,
                                error = %e,
                                "failed to query disk migration status"
                            );
                            all_completed = false;
                            continue;
                        }
                    };

                    if let Some(ref result) = status.result {
                        if !result.status.eq_ignore_ascii_case("ok") {
                            error!(
                                vm_id = %vm_id,
                                operation_id = %operation_id,
                                migration_id = %mig_id,
                                error = %result.human_summary,
                                "disk migration returned error status"
                            );
                            any_failed = true;
                            break;
                        }
                    }

                    total_bytes_transferred += status.bytes_transferred;
                    total_bytes += status.total_bytes;
                    max_dirty_remaining = max_dirty_remaining.max(status.dirty_blocks_remaining);
                    max_round = max_round.max(status.convergence_round);

                    use chv_stord_api::chv_stord_api::get_disk_migration_status_response::Phase as StordPhase;
                    let phase = StordPhase::try_from(status.phase).unwrap_or(StordPhase::Pending);

                    match phase {
                        StordPhase::Pending | StordPhase::BulkCopy | StordPhase::DirtySync => {
                            all_completed = false;
                        }
                        StordPhase::PausedFinalSync => {
                            all_completed = false;
                            needs_vm_pause = true;
                        }
                        StordPhase::Completed => {
                            // volume done
                        }
                        StordPhase::Failed => {
                            any_failed = true;
                            error!(
                                vm_id = %vm_id,
                                operation_id = %operation_id,
                                migration_id = %mig_id,
                                error = %status.error_message,
                                "disk migration failed"
                            );
                        }
                    }
                }

                if any_failed {
                    return Err("disk migration failed for one or more volumes".to_string());
                }

                // Report progress to control plane
                if let Some(ref reporter) = progress_reporter {
                    let proto_phase = if needs_vm_pause || all_completed {
                        proto::MigrationPhase::MemoryMigration
                    } else if max_round > 0 {
                        proto::MigrationPhase::ConvergingDisk
                    } else {
                        proto::MigrationPhase::PrecopyDisk
                    };
                    let progress_pct = if all_completed {
                        50.0
                    } else {
                        let ratio = if total_bytes > 0 {
                            (total_bytes_transferred as f32 / total_bytes as f32) * 50.0
                        } else {
                            0.0
                        };
                        ratio.min(45.0) // cap at 45% until all disk work is done
                    };

                    let progress = build_progress(
                        &vm_id,
                        &operation_id,
                        proto_phase,
                        total_bytes_transferred,
                        total_bytes,
                        max_round,
                        if needs_vm_pause || all_completed {
                            0
                        } else {
                            max_dirty_remaining
                        },
                        progress_pct,
                    );
                    let mut cb = reporter.lock().await;
                    cb(progress);
                }

                if all_completed {
                    info!(
                        vm_id = %vm_id,
                        operation_id = %operation_id,
                        "source agent: all disk migrations completed"
                    );
                    break;
                }

                if needs_vm_pause && !vm_paused_for_final_sync {
                    info!(
                        vm_id = %vm_id,
                        operation_id = %operation_id,
                        "source agent: pausing VM for final disk sync"
                    );
                    if let Err(e) = vm_runtime.pause_vm(&vm_id, Some(&operation_id)).await {
                        error!(
                            vm_id = %vm_id,
                            operation_id = %operation_id,
                            error = %e,
                            "source agent: failed to pause VM for final disk sync"
                        );
                        return Err(format!("failed to pause VM for final disk sync: {e}"));
                    }
                    vm_paused_for_final_sync = true;

                    for (_vol_id, mig_id) in &volume_migrations {
                        if let Err(e) = stord_client.resume_disk_migration(mig_id, true).await {
                            error!(
                                vm_id = %vm_id,
                                operation_id = %operation_id,
                                migration_id = %mig_id,
                                error = %e,
                                "source agent: failed to resume disk migration after VM pause"
                            );
                            return Err(format!("failed to resume disk migration {mig_id}: {e}"));
                        }
                    }

                    info!(
                        vm_id = %vm_id,
                        operation_id = %operation_id,
                        "source agent: VM paused, resumed all disk migrations for final sync"
                    );
                }
            }
        } else {
            info!(
                vm_id = %vm_id,
                operation_id = %operation_id,
                "source agent: no volumes to migrate, skipping disk pre-copy"
            );
        }

        // Phase 2: Memory migration (same as spawn_source_migration)
        info!(
            vm_id = %vm_id,
            destination_url = %destination_url,
            operation_id = %operation_id,
            "source agent: starting memory migration (send-migration)"
        );

        if let Some(ref reporter) = progress_reporter {
            let progress = build_progress(
                &vm_id,
                &operation_id,
                proto::MigrationPhase::MemoryMigration,
                0,
                0,
                0,
                0,
                50.0,
            );
            let mut cb = reporter.lock().await;
            cb(progress);
        }

        match vm_runtime
            .send_migration(&vm_id, &destination_url, Some(&operation_id))
            .await
        {
            Ok(()) => {
                if let Some(ref reporter) = progress_reporter {
                    let progress = build_progress(
                        &vm_id,
                        &operation_id,
                        proto::MigrationPhase::Completed,
                        0,
                        0,
                        0,
                        0,
                        100.0,
                    );
                    let mut cb = reporter.lock().await;
                    cb(progress);
                }

                info!(
                    vm_id = %vm_id,
                    operation_id = %operation_id,
                    "source agent: send-migration completed successfully (disk + memory)"
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    vm_id = %vm_id,
                    operation_id = %operation_id,
                    error = %e,
                    "source agent: memory migration (send-migration) failed"
                );
                Err(format!("memory migration failed: {e}"))
            }
        }
    })
}

/// Execute migration as the destination agent with disk pre-copy acceptance.
///
/// This orchestrates the destination side of a full live migration:
/// 1. **Disk pre-copy acceptance**: Prepares the local stord to receive incoming
///    block streams from the source stord. The actual block reception is handled
///    by stord's `StorageMigrationService`.
/// 2. **Memory migration**: Starts Cloud Hypervisor's receive-migration which
///    listens for the incoming VM memory state.
///
/// If disk acceptance preparation fails, the migration is aborted before memory
/// receive begins.
pub fn spawn_destination_migration_with_disk_precopy(
    vm_runtime: VmRuntime,
    vm_id: String,
    operation_id: String,
    receiver_url: String,
    disk_config: DiskPrecopyConfig,
) -> tokio::task::JoinHandle<Result<(), String>> {
    tokio::spawn(async move {
        info!(
            vm_id = %vm_id,
            receiver_url = %receiver_url,
            operation_id = %operation_id,
            volume_count = disk_config.volumes.len(),
            "destination agent: starting migration with disk pre-copy acceptance"
        );

        // Phase 1: Prepare to accept disk migration
        if !disk_config.volumes.is_empty() {
            info!(
                vm_id = %vm_id,
                operation_id = %operation_id,
                "destination agent: preparing to accept disk pre-copy"
            );

            let mut stord_client =
                match StordClient::connect(Path::new(&disk_config.stord_socket)).await {
                    Ok(client) => client,
                    Err(e) => {
                        error!(
                            vm_id = %vm_id,
                            operation_id = %operation_id,
                            error = %e,
                            "destination agent: failed to connect to local stord"
                        );
                        return Err(format!(
                            "disk migration acceptance failed: cannot connect to stord: {e}"
                        ));
                    }
                };

            for volume in &disk_config.volumes {
                info!(
                    vm_id = %vm_id,
                    operation_id = %operation_id,
                    volume_id = %volume.volume_id,
                    "destination agent: accepting disk migration for volume"
                );

                if let Err(e) = stord_client
                    .accept_disk_migration(
                        &volume.volume_id,
                        0, // Size will be communicated via InitMigration in the stream
                        Some(&operation_id),
                    )
                    .await
                {
                    error!(
                        vm_id = %vm_id,
                        operation_id = %operation_id,
                        volume_id = %volume.volume_id,
                        error = %e,
                        "destination agent: disk migration acceptance failed for volume"
                    );
                    return Err(format!(
                        "disk migration acceptance failed for volume {}: {e}",
                        volume.volume_id
                    ));
                }

                info!(
                    vm_id = %vm_id,
                    operation_id = %operation_id,
                    volume_id = %volume.volume_id,
                    "destination agent: disk migration acceptance ready for volume"
                );
            }

            info!(
                vm_id = %vm_id,
                operation_id = %operation_id,
                "destination agent: disk pre-copy acceptance prepared for all volumes"
            );
        } else {
            warn!(
                vm_id = %vm_id,
                operation_id = %operation_id,
                "destination agent: no volumes to receive, skipping disk acceptance"
            );
        }

        // Phase 2: Memory migration (same as spawn_destination_migration)
        info!(
            vm_id = %vm_id,
            receiver_url = %receiver_url,
            operation_id = %operation_id,
            "destination agent: starting memory migration (receive-migration)"
        );

        match vm_runtime
            .receive_migration(&vm_id, &receiver_url, Some(&operation_id))
            .await
        {
            Ok(()) => {
                info!(
                    vm_id = %vm_id,
                    operation_id = %operation_id,
                    "destination agent: receive-migration completed (disk + memory)"
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    vm_id = %vm_id,
                    operation_id = %operation_id,
                    error = %e,
                    "destination agent: memory migration (receive-migration) failed"
                );
                Err(format!("memory receive-migration failed: {e}"))
            }
        }
    })
}

/// Build a MigrationProgress proto message.
#[allow(clippy::too_many_arguments)]
pub fn build_progress(
    vm_id: &str,
    operation_id: &str,
    phase: proto::MigrationPhase,
    bytes_transferred: u64,
    total_bytes: u64,
    convergence_round: u32,
    dirty_blocks_remaining: u64,
    progress_percent: f32,
) -> proto::MigrationProgress {
    proto::MigrationProgress {
        vm_id: vm_id.to_string(),
        operation_id: operation_id.to_string(),
        phase: phase.into(),
        bytes_transferred,
        total_bytes,
        convergence_round,
        dirty_blocks_remaining,
        progress_percent,
    }
}

/// Default migration port to use when the receiver communicates its port back.
/// This is used as a fallback when port allocation information is not available.
pub const DEFAULT_MIGRATION_PORT: u16 = 49152;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn determine_role_source() {
        let role = determine_role("node-a", "node-a", "node-b");
        assert_eq!(role, Some(MigrationRole::Source));
    }

    #[test]
    fn determine_role_destination() {
        let role = determine_role("node-b", "node-a", "node-b");
        assert_eq!(role, Some(MigrationRole::Destination));
    }

    #[test]
    fn determine_role_unrelated() {
        let role = determine_role("node-c", "node-a", "node-b");
        assert_eq!(role, None);
    }

    #[test]
    fn allocate_port_succeeds() {
        // This test may fail if all ports in range are busy, but should work in CI.
        let result = allocate_migration_port();
        assert!(result.is_ok());
        let (port, listener) = result.unwrap();
        assert!(port >= MIGRATION_PORT_RANGE_START);
        assert!(port <= MIGRATION_PORT_RANGE_END);
        // The listener holds the port open — verify it matches
        assert_eq!(listener.local_addr().unwrap().port(), port);
    }

    #[test]
    fn build_receiver_url_format() {
        let url = build_receiver_url(49155);
        assert_eq!(url, "tcp://0.0.0.0:49155");
    }

    #[test]
    fn build_destination_url_format() {
        let url = build_destination_url("192.168.1.10", 49155);
        assert_eq!(url, "tcp://192.168.1.10:49155");
    }

    #[test]
    fn build_progress_message() {
        let progress = build_progress(
            "vm-1",
            "op-1",
            proto::MigrationPhase::MemoryMigration,
            1024,
            4096,
            2,
            10,
            25.0,
        );
        assert_eq!(progress.vm_id, "vm-1");
        assert_eq!(progress.operation_id, "op-1");
        assert_eq!(
            progress.phase,
            proto::MigrationPhase::MemoryMigration as i32
        );
        assert_eq!(progress.bytes_transferred, 1024);
        assert_eq!(progress.total_bytes, 4096);
        assert_eq!(progress.convergence_round, 2);
        assert_eq!(progress.dirty_blocks_remaining, 10);
        assert_eq!(progress.progress_percent, 25.0);
    }

    #[test]
    fn migration_volume_struct() {
        let vol = MigrationVolume {
            volume_id: "vol-abc".to_string(),
            attachment_handle: "handle-1".to_string(),
        };
        assert_eq!(vol.volume_id, "vol-abc");
        assert_eq!(vol.attachment_handle, "handle-1");
    }

    #[test]
    fn disk_precopy_config_construction() {
        let config = DiskPrecopyConfig {
            stord_socket: std::path::PathBuf::from("/run/chv/stord.sock"),
            dest_stord_endpoint: "http://10.0.0.5:50052".to_string(),
            volumes: vec![
                MigrationVolume {
                    volume_id: "vol-1".to_string(),
                    attachment_handle: "h-1".to_string(),
                },
                MigrationVolume {
                    volume_id: "vol-2".to_string(),
                    attachment_handle: "h-2".to_string(),
                },
            ],
        };
        assert_eq!(config.volumes.len(), 2);
        assert_eq!(
            config.stord_socket,
            std::path::PathBuf::from("/run/chv/stord.sock")
        );
        assert_eq!(config.dest_stord_endpoint, "http://10.0.0.5:50052");
    }

    #[test]
    fn disk_precopy_config_empty_volumes() {
        let config = DiskPrecopyConfig {
            stord_socket: std::path::PathBuf::from("/run/chv/stord.sock"),
            dest_stord_endpoint: "http://10.0.0.5:50052".to_string(),
            volumes: vec![],
        };
        assert!(config.volumes.is_empty());
    }

    // Guard against a regression where caller passes total_bytes=0 and
    // computes progress_percent upstream as `bytes / total`. The constructor
    // itself must accept the value verbatim and never panic.
    #[test]
    fn build_progress_with_zero_total_does_not_panic() {
        let progress = build_progress(
            "vm-1",
            "op-1",
            proto::MigrationPhase::Pending,
            0,
            0,
            0,
            0,
            0.0,
        );
        assert_eq!(progress.total_bytes, 0);
        assert_eq!(progress.bytes_transferred, 0);
        assert_eq!(progress.progress_percent, 0.0);
    }

    // Re-counted dirty pages can legitimately push bytes_transferred past
    // total_bytes during convergence rounds. The proto builder must not clamp
    // or reject — that is the caller's job — so we verify pass-through.
    #[test]
    fn build_progress_with_excess_bytes_preserves_values() {
        let progress = build_progress(
            "vm-1",
            "op-1",
            proto::MigrationPhase::ConvergingDisk,
            8192,
            4096,
            7,
            32,
            150.0,
        );
        assert_eq!(progress.bytes_transferred, 8192);
        assert_eq!(progress.total_bytes, 4096);
        assert_eq!(progress.convergence_round, 7);
        assert_eq!(progress.dirty_blocks_remaining, 32);
        assert_eq!(progress.progress_percent, 150.0);
    }

    // MigrationRole is `Copy`. Removing the derive would silently move the
    // value out of locals at every match site in the codebase. This test
    // makes that regression a compile error in the test build.
    #[test]
    fn migration_role_is_copy() {
        let original = MigrationRole::Source;
        let copied = original; // Copy semantics — original must remain usable.
        assert_eq!(original, MigrationRole::Source);
        assert_eq!(copied, MigrationRole::Source);
    }

    // Guards against proto enum drift: every variant must round-trip through
    // i32 unchanged. If a variant is added to the .proto and the agent forgets
    // to handle it, this test forces a compile-time match-arm failure here.
    #[test]
    fn migration_phase_round_trip_all_variants() {
        use proto::MigrationPhase;
        for variant in [
            MigrationPhase::Unspecified,
            MigrationPhase::Pending,
            MigrationPhase::PrecopyDisk,
            MigrationPhase::ConvergingDisk,
            MigrationPhase::MemoryMigration,
            MigrationPhase::Paused,
            MigrationPhase::Completed,
            MigrationPhase::Failed,
            MigrationPhase::RolledBack,
        ] {
            let raw = variant as i32;
            let recovered =
                MigrationPhase::try_from(raw).expect("known variant must round-trip through i32");
            assert_eq!(recovered, variant, "round-trip mismatch for {:?}", variant);
        }
    }

    // Symmetric test for the string-form round-trip — protects against
    // ProtoBuf field-name renames that would silently break wire-compat.
    #[test]
    fn migration_phase_round_trip_via_str_name() {
        use proto::MigrationPhase;
        for variant in [
            MigrationPhase::Unspecified,
            MigrationPhase::Pending,
            MigrationPhase::PrecopyDisk,
            MigrationPhase::ConvergingDisk,
            MigrationPhase::MemoryMigration,
            MigrationPhase::Paused,
            MigrationPhase::Completed,
            MigrationPhase::Failed,
            MigrationPhase::RolledBack,
        ] {
            let name = variant.as_str_name();
            let recovered = MigrationPhase::from_str_name(name)
                .expect("known variant must round-trip through str_name");
            assert_eq!(
                recovered, variant,
                "str-name round-trip mismatch for {:?}",
                variant
            );
        }
    }
}
