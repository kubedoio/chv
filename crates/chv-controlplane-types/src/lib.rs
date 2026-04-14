#![deny(unsafe_code)]

pub mod config;
pub mod domain;
pub mod state;

pub use config::{ApiConfig, ControlPlaneConfig, PersistenceConfig};
pub use domain::{
    ActorId, DesiredState, EventRecord, EventSeverity, EventType, Generation, IdentifierError,
    NodeId, NodeState, ObservedState, OperationId, OperationRecord, OperationStatus, ResourceId,
    ResourceKind, ResourceRef,
};
pub use state::{NodeLifecycleState, ResourceState, VersionedState};
