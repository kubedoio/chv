//! Overlay network management — fabric plan fan-out (ADR-021).
//!
//! Fans out control-plane-compiled stretched-L2 fabric plans to the
//! participating node agents over the CP → agent → nwd path. The legacy
//! per-VTEP FDB update path (pre-ADR-021 nolearning VXLAN) was retired:
//! kernel MAC learning plus gratuitous ARP provide correctness, and plan
//! (re-)dispatch is owned by the orchestrator's UpdateOverlay arm.

use crate::migration::resolve_agent_socket;
use crate::node_client_pool::NodeClientPool;
use chv_errors::ChvError;
use tracing::{info, warn};

/// Manages fabric plan fan-out across cluster nodes.
///
/// The overlay is eventually consistent: individual node failures are
/// logged but do not fail the overall operation. Nodes that miss an update
/// are corrected by the next plan dispatch.
#[derive(Clone)]
pub struct OverlayManager {
    node_pool: NodeClientPool,
    agent_socket_pattern: String,
}

impl OverlayManager {
    pub fn new(node_pool: NodeClientPool, agent_socket_pattern: String) -> Self {
        Self {
            node_pool,
            agent_socket_pattern,
        }
    }

    /// Resolve the Unix socket path for a node and connect through the
    /// client pool for the ADR-021 fabric fan-out.
    async fn connect_node(
        &self,
        node_id: &str,
    ) -> Result<crate::node_client::NodeClient, ChvError> {
        let socket_path = resolve_agent_socket(&self.agent_socket_pattern, node_id)?;
        self.node_pool.get_or_connect(node_id, &socket_path).await
    }

    /// Fan out compiled ADR-021 fabric plans: every participating node
    /// receives its own plan (its peer list excludes itself), with the
    /// network's plan generation as the `desired_state_version` fence.
    ///
    /// All nodes are attempted even if some fail; the returned error
    /// carries per-node detail so the operation record shows exactly which
    /// agents missed the update.
    pub async fn send_fabric_update(
        &self,
        network_id: &str,
        plans: &[crate::fabric_planner::CompiledFabricPlan],
        operation_id: &str,
    ) -> Result<(), ChvError> {
        let mut failures: Vec<String> = Vec::new();

        for compiled in plans {
            let node_id = compiled.node_id.clone();
            match self
                .send_fabric_plan_to_node(network_id, compiled, operation_id)
                .await
            {
                Ok(()) => {
                    info!(
                        network_id = network_id,
                        node_id = %node_id,
                        operation_id = operation_id,
                        peers = compiled.plan.peers.len(),
                        "fabric plan dispatched to node"
                    );
                }
                Err(e) => {
                    warn!(
                        network_id = network_id,
                        node_id = %node_id,
                        operation_id = operation_id,
                        error = %e,
                        "failed to dispatch fabric plan to node"
                    );
                    failures.push(format!("{node_id}: {e}"));
                }
            }
        }

        if failures.is_empty() {
            Ok(())
        } else {
            Err(ChvError::Internal {
                reason: format!(
                    "fabric update for network {network_id} failed on {} node(s): {}",
                    failures.len(),
                    failures.join("; ")
                ),
            })
        }
    }

    /// Deliver one compiled plan to its node. The deprecated legacy
    /// VTEP/FDB fields are not set: when `fabric` is present, nwd takes
    /// the fabric path (ADR-021) and the legacy fields are ignored.
    async fn send_fabric_plan_to_node(
        &self,
        network_id: &str,
        compiled: &crate::fabric_planner::CompiledFabricPlan,
        operation_id: &str,
    ) -> Result<(), ChvError> {
        let mut client = self.connect_node(&compiled.node_id).await?;
        client
            .update_overlay(
                &compiled.node_id,
                network_id,
                compiled.vni,
                operation_id,
                Some("control-plane"),
                Some(compiled.plan.clone()),
                &compiled.plan.plan_generation.to_string(),
            )
            .await?;
        Ok(())
    }
}
