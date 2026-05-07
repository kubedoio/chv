//! Agent-side migration coordination.
//!
//! This module handles live VM migration at the agent level. When the control plane
//! dispatches a `MigrateVm` RPC to an agent, the agent determines its role (source or
//! destination) based on matching its own node_id against `source_node_id` /
//! `destination_node_id` in the request and executes the appropriate Cloud Hypervisor
//! REST API calls.

use crate::vm_runtime::VmRuntime;
use control_plane_node_api::control_plane_node_api as proto;
use std::net::TcpListener;
use tracing::{error, info};

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
/// Tries each port in the range until one is available.
pub fn allocate_migration_port() -> Result<u16, &'static str> {
    for port in MIGRATION_PORT_RANGE_START..=MIGRATION_PORT_RANGE_END {
        if TcpListener::bind(("0.0.0.0", port)).is_ok() {
            return Ok(port);
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
        let port = result.unwrap();
        assert!(port >= MIGRATION_PORT_RANGE_START);
        assert!(port <= MIGRATION_PORT_RANGE_END);
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
}
