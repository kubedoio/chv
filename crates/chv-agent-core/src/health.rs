use crate::state_machine::{NodeState, StateMachine};

#[derive(Debug, Clone, Default)]
pub struct HealthAggregator {
    stord: Option<bool>,
    nwd: Option<bool>,
}

impl HealthAggregator {
    pub fn new() -> Self {
        Self {
            stord: None,
            nwd: None,
        }
    }

    pub fn update_stord(&mut self, healthy: bool) {
        self.stord = Some(healthy);
    }

    pub fn update_nwd(&mut self, healthy: bool) {
        self.nwd = Some(healthy);
    }

    pub fn derive_node_state(&self, current: NodeState) -> NodeState {
        let stord_ok = self.stord.unwrap_or(false);
        let nwd_ok = self.nwd.unwrap_or(false);

        match (stord_ok, nwd_ok) {
            (true, true) => match current {
                NodeState::HostReady
                | NodeState::StorageReady
                | NodeState::NetworkReady
                | NodeState::Degraded => NodeState::TenantReady,
                _ => current,
            },
            (false, false) => match current {
                NodeState::TenantReady | NodeState::StorageReady | NodeState::NetworkReady => {
                    NodeState::Degraded
                }
                _ => current,
            },
            (true, false) => match current {
                NodeState::TenantReady | NodeState::NetworkReady => NodeState::Degraded,
                NodeState::HostReady => NodeState::StorageReady,
                _ => current,
            },
            (false, true) => match current {
                NodeState::TenantReady | NodeState::StorageReady => NodeState::Degraded,
                NodeState::HostReady => NodeState::NetworkReady,
                _ => current,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_both_ready_goes_tenant_ready() {
        let mut h = HealthAggregator::new();
        h.update_stord(true);
        h.update_nwd(true);
        assert_eq!(
            h.derive_node_state(NodeState::HostReady),
            NodeState::TenantReady
        );
    }

    #[test]
    fn health_stord_down_degrades() {
        let mut h = HealthAggregator::new();
        h.update_stord(false);
        h.update_nwd(true);
        assert_eq!(
            h.derive_node_state(NodeState::TenantReady),
            NodeState::Degraded
        );
    }

    #[test]
    fn health_nwd_down_degrades() {
        let mut h = HealthAggregator::new();
        h.update_stord(true);
        h.update_nwd(false);
        assert_eq!(
            h.derive_node_state(NodeState::TenantReady),
            NodeState::Degraded
        );
    }

    #[test]
    fn health_both_down_degrades() {
        let mut h = HealthAggregator::new();
        h.update_stord(false);
        h.update_nwd(false);
        assert_eq!(
            h.derive_node_state(NodeState::TenantReady),
            NodeState::Degraded
        );
    }

    #[test]
    fn health_from_host_ready_partial() {
        let mut h = HealthAggregator::new();
        h.update_stord(true);
        h.update_nwd(false);
        assert_eq!(
            h.derive_node_state(NodeState::HostReady),
            NodeState::StorageReady
        );
    }
}
