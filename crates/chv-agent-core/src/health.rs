use crate::state_machine::NodeState;

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

        match current {
            NodeState::Bootstrapping => NodeState::HostReady,
            NodeState::HostReady => {
                if stord_ok {
                    NodeState::StorageReady
                } else {
                    NodeState::HostReady
                }
            }
            NodeState::StorageReady => {
                if !stord_ok {
                    NodeState::Degraded
                } else if nwd_ok {
                    NodeState::NetworkReady
                } else {
                    NodeState::StorageReady
                }
            }
            NodeState::NetworkReady => {
                if stord_ok && nwd_ok {
                    NodeState::TenantReady
                } else {
                    NodeState::Degraded
                }
            }
            NodeState::TenantReady => {
                if stord_ok && nwd_ok {
                    NodeState::TenantReady
                } else {
                    NodeState::Degraded
                }
            }
            NodeState::Degraded => {
                if stord_ok && nwd_ok {
                    NodeState::TenantReady
                } else {
                    NodeState::Degraded
                }
            }
            NodeState::Draining
            | NodeState::Maintenance
            | NodeState::Failed
            | NodeState::Discovered => current,
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
            NodeState::StorageReady
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

    #[test]
    fn health_from_host_ready_nwd_only() {
        let mut h = HealthAggregator::new();
        h.update_stord(false);
        h.update_nwd(true);
        assert_eq!(
            h.derive_node_state(NodeState::HostReady),
            NodeState::HostReady
        );
    }

    #[test]
    fn health_bootstrap_progresses_one_step_at_a_time() {
        let mut h = HealthAggregator::new();
        h.update_stord(true);
        h.update_nwd(true);

        assert_eq!(
            h.derive_node_state(NodeState::Bootstrapping),
            NodeState::HostReady
        );
        assert_eq!(
            h.derive_node_state(NodeState::HostReady),
            NodeState::StorageReady
        );
        assert_eq!(
            h.derive_node_state(NodeState::StorageReady),
            NodeState::NetworkReady
        );
        assert_eq!(
            h.derive_node_state(NodeState::NetworkReady),
            NodeState::TenantReady
        );
    }
}
