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
/// The overlay is eventually consistent for recoverable failures: a
/// fan-out with mixed per-node failures (or non-refusal failures only)
/// aggregates into one `Internal` error and the operation stays on the
/// shared retry curve — nodes that miss an update are corrected by the
/// next plan dispatch. The exception is the all-refusals shape (#378 §7
/// fast-fail): when EVERY failure in the fan-out is an `Unimplemented`
/// refusal — including the partial-success shape where some nodes applied
/// their plan and only the failing nodes refused — the refusal identity is
/// preserved and the operation fails terminally (`Failed` /
/// `UNSUPPORTED_BY_AGENT`) on the first dispatch, with no re-dispatch.
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
        // #378 §7 fast-fail, overlay leg: whether every per-node failure
        // was an `Unimplemented` refusal (the all-core-managed shape).
        let mut all_refusals = true;

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
                    if !matches!(e, ChvError::Unimplemented { .. }) {
                        all_refusals = false;
                    }
                    failures.push(format!("{node_id}: {e}"));
                }
            }
        }

        if failures.is_empty() {
            Ok(())
        } else if all_refusals {
            // #378 §7 fast-fail, overlay leg: when EVERY per-node failure
            // is an `Unimplemented` refusal, the aggregation preserves the
            // refusal identity instead of flattening to Internal, so the
            // orchestrator's tick handler fast-fails the `UpdateOverlay`
            // operation terminal (`Failed` / `UNSUPPORTED_BY_AGENT`, no
            // retry) exactly like the single-node dispatch path. The
            // reason is the per-node roll-up in fan-out order (the plans
            // arrive in a deterministic compile order: the planner walks
            // the peer list, which the store returns `ORDER BY node_id`),
            // so the operation record still shows which agents refused.
            // Mixed failures (some refusals, some other classes)
            // deliberately keep the Internal aggregation below: a partial
            // refusal is not terminal-class for the operation — the
            // non-refusing nodes may still fail transiently and succeed
            // on retry — so the shared retry curve must stay in charge.
            Err(ChvError::Unimplemented {
                reason: format!(
                    "fabric update for network {network_id} refused by all {} failing node(s): {}",
                    failures.len(),
                    failures.join("; ")
                ),
            })
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
