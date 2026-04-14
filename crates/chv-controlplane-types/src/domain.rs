use std::error::Error;
use std::fmt;
use std::str::FromStr;
use std::time::SystemTime;

macro_rules! string_id_newtype {
    ($name:ident, $field_name:literal) => {
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(IdentifierError::empty($field_name));
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl TryFrom<String> for $name {
            type Error = IdentifierError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = IdentifierError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }
    };
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentifierError {
    field: &'static str,
}

impl IdentifierError {
    pub fn empty(field: &'static str) -> Self {
        Self { field }
    }
}

impl fmt::Display for IdentifierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} cannot be empty", self.field)
    }
}

impl Error for IdentifierError {}

string_id_newtype!(ActorId, "actor_id");
string_id_newtype!(NodeId, "node_id");
string_id_newtype!(OperationId, "operation_id");
string_id_newtype!(ResourceId, "resource_id");

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Generation(u64);

impl Generation {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Generation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseGenerationError;

impl fmt::Display for ParseGenerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("generation must be a decimal string")
    }
}

impl Error for ParseGenerationError {}

impl From<u64> for Generation {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl FromStr for Generation {
    type Err = ParseGenerationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty() || !value.chars().all(|c| c.is_ascii_digit()) {
            return Err(ParseGenerationError);
        }

        value
            .parse::<u64>()
            .map(Self)
            .map_err(|_| ParseGenerationError)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ResourceKind {
    Node,
    Vm,
    Volume,
    Network,
}

impl ResourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Vm => "vm",
            Self::Volume => "volume",
            Self::Network => "network",
        }
    }
}

impl fmt::Display for ResourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseResourceKindError;

impl fmt::Display for ParseResourceKindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown resource kind")
    }
}

impl Error for ParseResourceKindError {}

impl FromStr for ResourceKind {
    type Err = ParseResourceKindError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "node" | "Node" => Ok(Self::Node),
            "vm" | "Vm" => Ok(Self::Vm),
            "volume" | "Volume" => Ok(Self::Volume),
            "network" | "Network" => Ok(Self::Network),
            _ => Err(ParseResourceKindError),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ResourceRef {
    pub kind: ResourceKind,
    pub id: ResourceId,
}

impl ResourceRef {
    pub fn new(kind: ResourceKind, id: ResourceId) -> Self {
        Self { kind, id }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
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
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Discovered => "Discovered",
            Self::Bootstrapping => "Bootstrapping",
            Self::HostReady => "HostReady",
            Self::StorageReady => "StorageReady",
            Self::NetworkReady => "NetworkReady",
            Self::TenantReady => "TenantReady",
            Self::Degraded => "Degraded",
            Self::Draining => "Draining",
            Self::Maintenance => "Maintenance",
            Self::Failed => "Failed",
        }
    }

    pub const fn is_schedulable(self) -> bool {
        matches!(self, Self::TenantReady)
    }
}

impl fmt::Display for NodeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseNodeStateError;

impl fmt::Display for ParseNodeStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown node state")
    }
}

impl Error for ParseNodeStateError {}

impl FromStr for NodeState {
    type Err = ParseNodeStateError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "Discovered" => Ok(Self::Discovered),
            "Bootstrapping" => Ok(Self::Bootstrapping),
            "HostReady" => Ok(Self::HostReady),
            "StorageReady" => Ok(Self::StorageReady),
            "NetworkReady" => Ok(Self::NetworkReady),
            "TenantReady" => Ok(Self::TenantReady),
            "Degraded" => Ok(Self::Degraded),
            "Draining" => Ok(Self::Draining),
            "Maintenance" => Ok(Self::Maintenance),
            "Failed" => Ok(Self::Failed),
            _ => Err(ParseNodeStateError),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum OperationStatus {
    Pending,
    Accepted,
    Running,
    Succeeded,
    Failed,
    Rejected,
    Stale,
    Conflict,
}

impl OperationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Accepted => "Accepted",
            Self::Running => "Running",
            Self::Succeeded => "Succeeded",
            Self::Failed => "Failed",
            Self::Rejected => "Rejected",
            Self::Stale => "Stale",
            Self::Conflict => "Conflict",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Rejected | Self::Stale | Self::Conflict
        )
    }
}

impl fmt::Display for OperationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseOperationStatusError;

impl fmt::Display for ParseOperationStatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown operation status")
    }
}

impl Error for ParseOperationStatusError {}

impl FromStr for OperationStatus {
    type Err = ParseOperationStatusError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "Pending" => Ok(Self::Pending),
            "Accepted" => Ok(Self::Accepted),
            "Running" => Ok(Self::Running),
            "Succeeded" => Ok(Self::Succeeded),
            "Failed" => Ok(Self::Failed),
            "Rejected" => Ok(Self::Rejected),
            "Stale" => Ok(Self::Stale),
            "Conflict" => Ok(Self::Conflict),
            _ => Err(ParseOperationStatusError),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum EventSeverity {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

impl EventSeverity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "Debug",
            Self::Info => "Info",
            Self::Warning => "Warning",
            Self::Error => "Error",
            Self::Critical => "Critical",
        }
    }
}

impl fmt::Display for EventSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseEventSeverityError;

impl fmt::Display for ParseEventSeverityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown event severity")
    }
}

impl Error for ParseEventSeverityError {}

impl FromStr for EventSeverity {
    type Err = ParseEventSeverityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "Debug" => Ok(Self::Debug),
            "Info" => Ok(Self::Info),
            "Warning" => Ok(Self::Warning),
            "Error" => Ok(Self::Error),
            "Critical" => Ok(Self::Critical),
            _ => Err(ParseEventSeverityError),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum EventType {
    Audit,
    Enrollment,
    DesiredStateApplied,
    DesiredStateRejected,
    ObservedStateReported,
    StateTransition,
    OperationStarted,
    OperationCompleted,
    OperationFailed,
    Maintenance,
    Health,
}

impl EventType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Audit => "Audit",
            Self::Enrollment => "Enrollment",
            Self::DesiredStateApplied => "DesiredStateApplied",
            Self::DesiredStateRejected => "DesiredStateRejected",
            Self::ObservedStateReported => "ObservedStateReported",
            Self::StateTransition => "StateTransition",
            Self::OperationStarted => "OperationStarted",
            Self::OperationCompleted => "OperationCompleted",
            Self::OperationFailed => "OperationFailed",
            Self::Maintenance => "Maintenance",
            Self::Health => "Health",
        }
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseEventTypeError;

impl fmt::Display for ParseEventTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown event type")
    }
}

impl Error for ParseEventTypeError {}

impl FromStr for EventType {
    type Err = ParseEventTypeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "Audit" => Ok(Self::Audit),
            "Enrollment" => Ok(Self::Enrollment),
            "DesiredStateApplied" => Ok(Self::DesiredStateApplied),
            "DesiredStateRejected" => Ok(Self::DesiredStateRejected),
            "ObservedStateReported" => Ok(Self::ObservedStateReported),
            "StateTransition" => Ok(Self::StateTransition),
            "OperationStarted" => Ok(Self::OperationStarted),
            "OperationCompleted" => Ok(Self::OperationCompleted),
            "OperationFailed" => Ok(Self::OperationFailed),
            "Maintenance" => Ok(Self::Maintenance),
            "Health" => Ok(Self::Health),
            _ => Err(ParseEventTypeError),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesiredState<T> {
    pub version: Generation,
    pub value: T,
    pub recorded_at: SystemTime,
    pub recorded_by: Option<ActorId>,
}

impl<T> DesiredState<T> {
    pub fn new(
        version: Generation,
        value: T,
        recorded_at: SystemTime,
        recorded_by: Option<ActorId>,
    ) -> Self {
        Self {
            version,
            value,
            recorded_at,
            recorded_by,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedState<T> {
    pub version: Generation,
    pub value: T,
    pub observed_at: SystemTime,
    pub observed_by: Option<ActorId>,
}

impl<T> ObservedState<T> {
    pub fn new(
        version: Generation,
        value: T,
        observed_at: SystemTime,
        observed_by: Option<ActorId>,
    ) -> Self {
        Self {
            version,
            value,
            observed_at,
            observed_by,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventRecord {
    pub operation_id: Option<OperationId>,
    pub resource: Option<ResourceRef>,
    pub severity: EventSeverity,
    pub event_type: EventType,
    pub message: String,
    pub occurred_at: SystemTime,
    pub actor: Option<ActorId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationRecord {
    pub operation_id: OperationId,
    pub resource: Option<ResourceRef>,
    pub status: OperationStatus,
    pub requested_by: Option<ActorId>,
    pub summary: Option<String>,
    pub requested_at: SystemTime,
    pub updated_at: SystemTime,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_rejects_non_numeric_input() {
        assert!(Generation::from_str("").is_err());
        assert!(Generation::from_str("abc").is_err());
        assert!(Generation::from_str("12x").is_err());
    }

    #[test]
    fn resource_kind_round_trips() {
        let kind = ResourceKind::from_str("vm").unwrap();
        assert_eq!(kind, ResourceKind::Vm);
        assert_eq!(kind.to_string(), "vm");
    }

    #[test]
    fn node_state_schedulability_tracks_the_tenant_ready_state() {
        assert!(!NodeState::Bootstrapping.is_schedulable());
        assert!(NodeState::TenantReady.is_schedulable());
    }

    #[test]
    fn operation_status_identifies_terminal_states() {
        assert!(!OperationStatus::Accepted.is_terminal());
        assert!(OperationStatus::Succeeded.is_terminal());
        assert!(OperationStatus::Conflict.is_terminal());
    }
}
