use crate::cache::NodeCache;
use crate::control_plane::ControlPlaneClient;
use crate::daemon_clients::{NwdClient, StordClient};
use crate::state_machine::{NodeState, StateMachine};
use chv_errors::ChvError;
use std::path::PathBuf;
use tracing::info;

pub struct Reconciler {
    pub cache: NodeCache,
    pub state_machine: StateMachine,
    pub stord_socket: PathBuf,
    pub nwd_socket: PathBuf,
}

impl Reconciler {
    pub fn new(
        cache: NodeCache,
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
            stord_socket,
            nwd_socket,
        }
    }

    pub async fn run_once(&mut self) -> Result<(), ChvError> {
        info!(
            state = %self.state_machine.current().as_str(),
            "reconcile tick"
        );
        // Phase 1: skeleton only. Health probes and state transitions happen
        // in the binary's main loop for now.
        Ok(())
    }
}
