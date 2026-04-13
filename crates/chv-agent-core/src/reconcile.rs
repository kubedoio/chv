use crate::cache::NodeCache;
use crate::state_machine::{NodeState, StateMachine};
use crate::vm_runtime::VmRuntime;
use chv_errors::ChvError;
use std::path::PathBuf;
use tracing::info;

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
        let initial = cache
            .node_state
            .parse()
            .unwrap_or(NodeState::Bootstrapping);
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
        // Phase 2: iterate over cached VM fragments and ensure they match runtime.
        // For now, log the divergence count without acting.
        let vm_count = self.cache.vm_fragments.len();
        let runtime_count = self.vm_runtime.list().len();
        if vm_count != runtime_count {
            info!(
                cached_vms = vm_count,
                runtime_vms = runtime_count,
                "reconcile divergence detected"
            );
        }
        Ok(())
    }
}
