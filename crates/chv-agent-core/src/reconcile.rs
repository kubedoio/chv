use crate::cache::NodeCache;
use crate::daemon_clients::{NwdClient, StordClient};
use crate::state_machine::{NodeState, StateMachine};
use crate::vm_runtime::VmRuntime;
use chv_errors::ChvError;
use std::path::PathBuf;
use tracing::{info, warn};

pub struct Reconciler {
    pub cache: NodeCache,
    pub state_machine: StateMachine,
    pub vm_runtime: VmRuntime,
    pub stord_socket: PathBuf,
    pub nwd_socket: PathBuf,
}

impl Reconciler {
    pub fn new(
        cache: NodeCache,
        vm_runtime: VmRuntime,
        stord_socket: PathBuf,
        nwd_socket: PathBuf,
    ) -> Self {
        let initial = cache.node_state.parse().unwrap_or(NodeState::Bootstrapping);
        Self {
            cache,
            state_machine: StateMachine::new(initial),
            vm_runtime,
            stord_socket,
            nwd_socket,
        }
    }

    pub async fn run_once(&mut self) -> Result<(), ChvError> {
        info!(
            state = %self.state_machine.current().as_str(),
            "reconcile tick"
        );

        // Only act when tenant-ready
        if !matches!(self.state_machine.current(), NodeState::TenantReady) {
            return Ok(());
        }

        self.reconcile_networks().await?;
        self.reconcile_volumes().await?;
        self.reconcile_vms().await?;
        Ok(())
    }

    async fn reconcile_networks(&mut self) -> Result<(), ChvError> {
        let network_ids: std::collections::HashSet<String> = self.cache.vm_network_ids().into_iter().collect();
        if network_ids.is_empty() {
            return Ok(());
        }
        let mut nwd = NwdClient::connect(&self.nwd_socket).await?;
        for net_id in network_ids {
            let bridge = format!("br-{}", net_id);
            let cidr = "10.0.0.0/24"; // TODO: derive from fragment when available
            if let Err(e) = nwd.ensure_network_topology(&net_id, &bridge, cidr, None).await {
                warn!(network_id = %net_id, error = %e, "failed to ensure network topology");
            }
        }
        Ok(())
    }

    async fn reconcile_volumes(&mut self) -> Result<(), ChvError> {
        let pairs: std::collections::HashSet<(String, String)> = self.cache.vm_volume_handles().into_iter().collect();
        if pairs.is_empty() {
            return Ok(());
        }
        let mut stord = StordClient::connect(&self.stord_socket).await?;
        for (vm_id, volume_id) in pairs {
            // Open volume if not already open (best-effort)
            let locator = format!("{}.img", volume_id);
            match stord.open_volume(&volume_id, "local", &locator, None).await {
                Ok((_, handle, _)) => {
                    if let Err(e) = stord.attach_volume_to_vm(&volume_id, &vm_id, &handle, None).await {
                        warn!(volume_id = %volume_id, vm_id = %vm_id, error = %e, "failed to attach volume");
                    }
                }
                Err(e) => {
                    warn!(volume_id = %volume_id, error = %e, "failed to open volume during reconcile");
                }
            }
        }
        Ok(())
    }

    async fn reconcile_vms(&mut self) -> Result<(), ChvError> {
        // NOTE: Full VM runtime reconciliation (create/delete/resize) is out of scope
        // for this task. We only log divergence so the gap is visible.
        let vm_count = self.cache.vm_fragments.len();
        let runtime_count = self.vm_runtime.list().len();
        if vm_count != runtime_count {
            info!(
                cached_vms = vm_count,
                runtime_vms = runtime_count,
                "reconcile divergence detected (VM runtime reconciliation not yet implemented)"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_cache() -> NodeCache {
        use crate::cache::DesiredStateFragment;
        NodeCache {
            node_state: "TenantReady".to_string(),
            vm_fragments: {
                let mut m = HashMap::new();
                m.insert("vm-1".to_string(), DesiredStateFragment {
                    id: "vm-1".to_string(),
                    kind: "vm".to_string(),
                    generation: "1".to_string(),
                    spec_json: br#"{"network_id":"net-1","volumes":[{"volume_id":"vol-1"}]}"#.to_vec(),
                    policy_json: vec![],
                    updated_at: "2024-01-01T00:00:00Z".to_string(),
                    updated_by: "cp".to_string(),
                });
                m
            },
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn reconciler_skips_when_not_tenant_ready() {
        let mut cache = test_cache();
        cache.node_state = "Bootstrapping".to_string();
        let mut rec = Reconciler::new(
            cache,
            VmRuntime::new(std::sync::Arc::new(chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter::default())),
            PathBuf::from("/tmp/fake-stord.sock"),
            PathBuf::from("/tmp/fake-nwd.sock"),
        );
        assert!(rec.run_once().await.is_ok());
    }
}
