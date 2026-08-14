//! Overlay network management — VTEP registry and VNI lifecycle.
//!
//! Coordinates VXLAN overlay state across participating nodes:
//! - VM migration completion -> FDB re-pointing across peers
//! - Per-node overlay updates via UpdateOverlay RPC

use crate::migration::resolve_agent_socket;
use crate::node_client_pool::NodeClientPool;
use chv_controlplane_store::VtepRepository;
use chv_errors::ChvError;
use tracing::{info, warn};

/// Manages VXLAN overlay state across cluster nodes.
///
/// The overlay is eventually consistent: individual node failures are logged
/// but do not fail the overall operation. Nodes that miss an update will
/// reconcile on the next heartbeat cycle.
#[derive(Clone)]
pub struct OverlayManager {
    vtep_repo: VtepRepository,
    node_pool: NodeClientPool,
    agent_socket_pattern: String,
}

impl OverlayManager {
    pub fn new(
        vtep_repo: VtepRepository,
        node_pool: NodeClientPool,
        agent_socket_pattern: String,
    ) -> Self {
        Self {
            vtep_repo,
            node_pool,
            agent_socket_pattern,
        }
    }

    /// Called after migration completes. Re-points FDB and sends gratuitous ARP.
    #[allow(clippy::too_many_arguments)]
    #[tracing::instrument(skip(self), fields(network_id = %network_id, vm_mac = %vm_mac, new_node_id = %new_node_id, vni = vni))]
    pub async fn on_vm_migrated(
        &self,
        network_id: &str,
        vm_mac: &str,
        _vm_ip: &str,
        new_node_id: &str,
        _bridge_name: &str,
        vni: i32,
        operation_id: &str,
    ) -> Result<(), ChvError> {
        // 1. Get new node's VTEP
        let new_vtep = match self.vtep_repo.get_vtep(new_node_id).await {
            Ok(vtep) => vtep,
            Err(e) => {
                warn!(
                    node_id = %new_node_id,
                    error = %e,
                    "new node has no VTEP registered; skipping post-migration overlay update"
                );
                return Ok(());
            }
        };

        // 2. Get all VTEPs for this network
        let vteps = self
            .vtep_repo
            .get_vteps_for_network(network_id)
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("failed to get VTEPs for network {network_id}: {e}"),
            })?;

        info!(
            network_id = %network_id,
            new_node_id = %new_node_id,
            vm_mac = %vm_mac,
            vni = vni,
            operation_id = %operation_id,
            "re-pointing overlay FDB after migration"
        );

        // 3. Send updated FDB entry to all peers (vm_mac -> new_vtep_ip)
        let fdb_entries = vec![FdbEntry {
            mac_address: vm_mac.to_string(),
            vtep_ip: new_vtep.vtep_ip.clone(),
        }];

        for vtep in &vteps {
            if let Err(e) = self
                .send_overlay_update(&vtep.node_id, network_id, vni, &vteps, &fdb_entries)
                .await
            {
                warn!(
                    node_id = %vtep.node_id,
                    error = %e,
                    "failed to send FDB re-point to peer after migration"
                );
            }
        }

        // Note: gratuitous ARP is handled by the agent on the new node as part of
        // the network desired state reconciliation after migration completes.

        Ok(())
    }

    /// Resolve the Unix socket path for a given node.
    /// Send an overlay update to a specific node via the node client pool.
    ///
    /// Uses the UpdateOverlay RPC on the agent's lifecycle service, which proxies
    /// the request to nwd's UpdateOverlay endpoint.
    async fn send_overlay_update(
        &self,
        node_id: &str,
        network_id: &str,
        vni: i32,
        vteps: &[chv_controlplane_store::VtepEntry],
        fdb_entries: &[FdbEntry],
    ) -> Result<(), ChvError> {
        use control_plane_node_api::control_plane_node_api as proto;

        let socket_path = resolve_agent_socket(&self.agent_socket_pattern, node_id);
        let mut client = self.node_pool.get_or_connect(node_id, &socket_path).await?;

        let vtep_endpoints: Vec<proto::VtepEndpoint> = vteps
            .iter()
            .map(|v| proto::VtepEndpoint {
                node_id: v.node_id.clone(),
                vtep_ip: v.vtep_ip.clone(),
                vtep_port: v.vtep_port as u32,
            })
            .collect();

        let proto_fdb_entries: Vec<proto::FdbEntry> = fdb_entries
            .iter()
            .map(|f| proto::FdbEntry {
                mac_address: f.mac_address.clone(),
                vtep_ip: f.vtep_ip.clone(),
            })
            .collect();

        let operation_id = format!("overlay-{network_id}");

        client
            .update_overlay(
                node_id,
                network_id,
                vni as u32,
                vtep_endpoints,
                proto_fdb_entries,
                &operation_id,
                Some("control-plane"),
            )
            .await?;

        Ok(())
    }
}

/// Internal FDB entry representation used for overlay updates.
#[derive(Debug, Clone)]
pub struct FdbEntry {
    pub mac_address: String,
    pub vtep_ip: String,
}
