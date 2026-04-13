use chv_errors::ChvError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeState {
    Discovered,
    Bootstrapping,
    HostReady,
    StorageReady,
    NetworkReady,
    TenantReady,
    Degraded,
    Draining,
    Maintenance,
    Failed,
}

impl NodeState {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeState::Discovered => "Discovered",
            NodeState::Bootstrapping => "Bootstrapping",
            NodeState::HostReady => "HostReady",
            NodeState::StorageReady => "StorageReady",
            NodeState::NetworkReady => "NetworkReady",
            NodeState::TenantReady => "TenantReady",
            NodeState::Degraded => "Degraded",
            NodeState::Draining => "Draining",
            NodeState::Maintenance => "Maintenance",
            NodeState::Failed => "Failed",
        }
    }
}

impl std::str::FromStr for NodeState {
    type Err = ChvError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Discovered" => Ok(NodeState::Discovered),
            "Bootstrapping" => Ok(NodeState::Bootstrapping),
            "HostReady" => Ok(NodeState::HostReady),
            "StorageReady" => Ok(NodeState::StorageReady),
            "NetworkReady" => Ok(NodeState::NetworkReady),
            "TenantReady" => Ok(NodeState::TenantReady),
            "Degraded" => Ok(NodeState::Degraded),
            "Draining" => Ok(NodeState::Draining),
            "Maintenance" => Ok(NodeState::Maintenance),
            "Failed" => Ok(NodeState::Failed),
            _ => Err(ChvError::InvalidArgument {
                field: "node_state".to_string(),
                reason: format!("unknown state: {}", s),
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StateMachine {
    current: NodeState,
}

impl StateMachine {
    pub fn new(initial: NodeState) -> Self {
        Self { current: initial }
    }

    pub fn current(&self) -> NodeState {
        self.current
    }

    pub fn transition(&mut self, to: NodeState) -> Result<(), ChvError> {
        if self.is_valid_transition(to) {
            self.current = to;
            Ok(())
        } else {
            Err(ChvError::InvalidArgument {
                field: "node_state".to_string(),
                reason: format!(
                    "invalid transition from {} to {}",
                    self.current.as_str(),
                    to.as_str()
                ),
            })
        }
    }

    fn is_valid_transition(&self, to: NodeState) -> bool {
        use NodeState::*;
        match (self.current, to) {
            // Boot sequence
            (Discovered, Bootstrapping) => true,
            (Bootstrapping, HostReady) => true,
            (HostReady, StorageReady) => true,
            (HostReady, NetworkReady) => true,
            (StorageReady, NetworkReady) => true,
            (NetworkReady, StorageReady) => true,
            (StorageReady, TenantReady) => true,
            (NetworkReady, TenantReady) => true,
            (TenantReady, TenantReady) => true,
            // Recovery / degradation
            (TenantReady, Degraded) => true,
            (Degraded, TenantReady) => true,
            (Degraded, Failed) => true,
            (Degraded, HostReady) => true,
            (HostReady, Degraded) => true,
            // Operator modes
            (TenantReady, Draining) => true,
            (Degraded, Draining) => true,
            (Draining, Degraded) => true,
            (_, Maintenance) => true,
            (Maintenance, Bootstrapping) => true,
            (Maintenance, HostReady) => true,
            // Self transitions are always valid
            (s1, s2) if s1 == s2 => true,
            _ => false,
        }
    }

    pub fn is_schedulable(&self) -> bool {
        self.current == NodeState::TenantReady
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_transitions() {
        let mut sm = StateMachine::new(NodeState::Discovered);
        assert!(sm.transition(NodeState::Bootstrapping).is_ok());
        assert!(sm.transition(NodeState::HostReady).is_ok());
        assert!(sm.transition(NodeState::StorageReady).is_ok());
        assert!(sm.transition(NodeState::NetworkReady).is_ok());
        assert!(sm.transition(NodeState::TenantReady).is_ok());
    }

    #[test]
    fn invalid_transition() {
        let mut sm = StateMachine::new(NodeState::Discovered);
        assert!(sm.transition(NodeState::TenantReady).is_err());
    }

    #[test]
    fn schedulable_only_tenant_ready() {
        assert!(!StateMachine::new(NodeState::HostReady).is_schedulable());
        assert!(StateMachine::new(NodeState::TenantReady).is_schedulable());
        assert!(!StateMachine::new(NodeState::Degraded).is_schedulable());
    }

    #[test]
    fn self_transition_ok() {
        let mut sm = StateMachine::new(NodeState::TenantReady);
        assert!(sm.transition(NodeState::TenantReady).is_ok());
    }
}
