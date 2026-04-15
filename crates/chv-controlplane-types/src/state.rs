use crate::domain::{DesiredState, Generation, NodeId, NodeState, ObservedState, ResourceRef};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionedState<T> {
    pub desired: DesiredState<T>,
    pub observed: Option<ObservedState<T>>,
}

impl<T> VersionedState<T> {
    pub fn new(desired: DesiredState<T>) -> Self {
        Self {
            desired,
            observed: None,
        }
    }

    pub fn with_observed(mut self, observed: ObservedState<T>) -> Self {
        self.observed = Some(observed);
        self
    }

    pub fn desired_version(&self) -> Generation {
        self.desired.version
    }

    pub fn observed_version(&self) -> Option<Generation> {
        self.observed.as_ref().map(|observed| observed.version)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceState<T> {
    pub resource: ResourceRef,
    pub state: VersionedState<T>,
}

impl<T> ResourceState<T> {
    pub fn new(resource: ResourceRef, desired: DesiredState<T>) -> Self {
        Self {
            resource,
            state: VersionedState::new(desired),
        }
    }

    pub fn with_observed(mut self, observed: ObservedState<T>) -> Self {
        self.state = self.state.with_observed(observed);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeLifecycleState {
    pub node_id: NodeId,
    pub state: VersionedState<NodeState>,
}

impl NodeLifecycleState {
    pub fn new(node_id: NodeId, desired: DesiredState<NodeState>) -> Self {
        Self {
            node_id,
            state: VersionedState::new(desired),
        }
    }

    pub fn with_observed(mut self, observed: ObservedState<NodeState>) -> Self {
        self.state = self.state.with_observed(observed);
        self
    }

    pub fn is_schedulable(&self) -> bool {
        self.state
            .observed
            .as_ref()
            .map(|observed| observed.value.is_schedulable())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use crate::domain::{
        DesiredState, Generation, NodeId, NodeState, ObservedState, ResourceId, ResourceKind,
        ResourceRef,
    };

    use super::{NodeLifecycleState, ResourceState, VersionedState};

    fn sample_time(offset_secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(offset_secs)
    }

    #[test]
    fn versioned_state_keeps_desired_and_observed_generations_separate() {
        let desired = DesiredState::new(Generation::new(7), "desired", sample_time(1), None);
        let observed = ObservedState::new(Generation::new(6), "observed", sample_time(2), None);

        let state = VersionedState::new(desired).with_observed(observed);

        assert_eq!(state.desired_version(), Generation::new(7));
        assert_eq!(state.observed_version(), Some(Generation::new(6)));
    }

    #[test]
    fn resource_state_wraps_a_typed_resource_reference() {
        let resource = ResourceRef::new(ResourceKind::Volume, ResourceId::new("vol-1").unwrap());
        let desired = DesiredState::new(Generation::new(3), "payload", sample_time(3), None);

        let state = ResourceState::new(resource.clone(), desired);

        assert_eq!(state.resource, resource);
        assert_eq!(state.state.desired_version(), Generation::new(3));
    }

    #[test]
    fn node_lifecycle_uses_observed_state_for_schedulability() {
        let desired = DesiredState::new(
            Generation::new(11),
            NodeState::TenantReady,
            sample_time(4),
            Some(crate::domain::ActorId::new("control-plane").unwrap()),
        );
        let observed = ObservedState::new(
            Generation::new(10),
            NodeState::Bootstrapping,
            sample_time(5),
            None,
        );

        let state = NodeLifecycleState::new(NodeId::new("node-a").unwrap(), desired)
            .with_observed(observed);

        assert!(!state.is_schedulable());
    }
}
