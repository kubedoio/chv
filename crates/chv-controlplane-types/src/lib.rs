#![deny(unsafe_code)]

pub mod architecture;
pub mod constants;
pub mod domain;
pub mod fragment;
pub mod state;

pub use domain::{
    ActorId, DesiredState, EventRecord, EventSeverity, EventType, Generation, IdentifierError,
    NodeId, NodeState, ObservedState, OperationId, OperationRecord, OperationStatus, ResourceId,
    ResourceKind, ResourceRef,
};
pub use fragment::{NetworkExposureSpec, NetworkSpec, NodeSpec, VmSpec, VolumeSpec};
