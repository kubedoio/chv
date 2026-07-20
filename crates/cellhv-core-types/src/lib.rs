//! Platform-neutral domain model for the CellHV Core authority.
//!
//! These types intentionally contain no control-plane, cloud-platform, provider,
//! protocol, or Cloud Hypervisor implementation details.

use chv_errors::ChvError;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Deterministically normalizes a JSON value for durable request identity.
pub fn canonical_json(value: &serde_json::Value) -> Result<String, serde_json::Error> {
    fn normalize(value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(map) => serde_json::Value::Object(
                map.iter()
                    .map(|(key, value)| (key.clone(), normalize(value)))
                    .collect(),
            ),
            serde_json::Value::Array(items) => {
                serde_json::Value::Array(items.iter().map(normalize).collect())
            }
            other => other.clone(),
        }
    }
    serde_json::to_string(&normalize(value))
}

/// SHA-256 identity of the canonical JSON representation.
pub fn canonical_request_fingerprint(
    value: &serde_json::Value,
) -> Result<String, serde_json::Error> {
    Ok(format!("{:x}", Sha256::digest(canonical_json(value)?)))
}

fn require_non_empty(field: &str, value: &str) -> Result<(), ChvError> {
    if value.trim().is_empty() {
        return Err(ChvError::InvalidArgument {
            field: field.to_string(),
            reason: "must not be empty".to_string(),
        });
    }
    Ok(())
}

macro_rules! identifier {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ChvError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(ChvError::InvalidArgument {
                        field: $field.to_string(),
                        reason: "must not be empty".to_string(),
                    });
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl TryFrom<String> for $name {
            type Error = ChvError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

identifier!(HostId, "host_id");
identifier!(VmId, "vm_id");
identifier!(OperationId, "operation_id");
identifier!(EventId, "event_id");
identifier!(IdempotencyKey, "idempotency_key");

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct ResourceVersion(u64);

impl ResourceVersion {
    pub fn new(value: u64) -> Result<Self, ChvError> {
        if value == 0 {
            return Err(ChvError::InvalidArgument {
                field: "resource_version".to_string(),
                reason: "must be greater than zero".to_string(),
            });
        }
        Ok(Self(value))
    }

    pub fn get(self) -> u64 {
        self.0
    }

    pub fn next(self) -> Result<Self, ChvError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or_else(|| ChvError::Conflict {
                resource: "resource_version".to_string(),
                id: self.0.to_string(),
            })
    }
}

impl<'de> Deserialize<'de> for ResourceVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u64::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestedPowerState {
    Running,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservedPowerState {
    Unknown,
    Created,
    Running,
    Stopped,
    Paused,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    CreateVm,
    UpdateVm,
    DeleteVm,
    StartVm,
    StopVm,
    RebootVm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationStatus {
    Accepted,
    Running,
    Succeeded,
    Failed,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnershipClass {
    CellHv,
    External,
    Unclaimed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryClass {
    OwnedRecoverable,
    OwnedInconsistent,
    Foreign,
    Ambiguous,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostIdentity {
    pub id: HostId,
    pub resource_version: ResourceVersion,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HostCapabilities {
    pub vm_definitions: bool,
    pub power_start: bool,
    pub power_stop: bool,
    pub power_reboot: bool,
    pub live_update_vcpus: bool,
    pub live_update_memory: bool,
    pub event_watch: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "RawBootSpec")]
pub struct BootSpec {
    pub kernel: String,
    pub firmware: Option<String>,
    pub initial_disk: Option<String>,
}

impl BootSpec {
    pub fn new(kernel: impl Into<String>) -> Result<Self, ChvError> {
        let kernel = kernel.into();
        require_non_empty("boot.kernel", &kernel)?;
        Ok(Self {
            kernel,
            firmware: None,
            initial_disk: None,
        })
    }

    pub fn validate(&self) -> Result<(), ChvError> {
        require_non_empty("boot.kernel", &self.kernel)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBootSpec {
    kernel: String,
    firmware: Option<String>,
    initial_disk: Option<String>,
}

impl TryFrom<RawBootSpec> for BootSpec {
    type Error = ChvError;

    fn try_from(raw: RawBootSpec) -> Result<Self, Self::Error> {
        let value = Self {
            kernel: raw.kernel,
            firmware: raw.firmware,
            initial_disk: raw.initial_disk,
        };
        value.validate()?;
        Ok(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "RawComputeSpec")]
pub struct ComputeSpec {
    pub vcpus: u32,
    pub memory_bytes: u64,
}

impl ComputeSpec {
    pub fn new(vcpus: u32, memory_bytes: u64) -> Result<Self, ChvError> {
        let value = Self {
            vcpus,
            memory_bytes,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), ChvError> {
        if self.vcpus == 0 {
            return Err(ChvError::InvalidArgument {
                field: "compute.vcpus".to_string(),
                reason: "must be greater than zero".to_string(),
            });
        }
        if self.memory_bytes == 0 {
            return Err(ChvError::InvalidArgument {
                field: "compute.memory_bytes".to_string(),
                reason: "must be greater than zero".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawComputeSpec {
    vcpus: u32,
    memory_bytes: u64,
}

impl TryFrom<RawComputeSpec> for ComputeSpec {
    type Error = ChvError;

    fn try_from(raw: RawComputeSpec) -> Result<Self, Self::Error> {
        Self::new(raw.vcpus, raw.memory_bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "RawStorageAttachmentRef")]
pub struct StorageAttachmentRef {
    pub attachment_id: String,
    pub storage_ref: String,
    pub read_only: bool,
}

impl StorageAttachmentRef {
    pub fn validate(&self) -> Result<(), ChvError> {
        require_non_empty("storage.attachment_id", &self.attachment_id)?;
        require_non_empty("storage.storage_ref", &self.storage_ref)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStorageAttachmentRef {
    attachment_id: String,
    storage_ref: String,
    read_only: bool,
}

impl TryFrom<RawStorageAttachmentRef> for StorageAttachmentRef {
    type Error = ChvError;

    fn try_from(raw: RawStorageAttachmentRef) -> Result<Self, Self::Error> {
        let value = Self {
            attachment_id: raw.attachment_id,
            storage_ref: raw.storage_ref,
            read_only: raw.read_only,
        };
        value.validate()?;
        Ok(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "RawNetworkAttachmentRef")]
pub struct NetworkAttachmentRef {
    pub attachment_id: String,
    pub network_ref: String,
    pub mac_address: Option<String>,
}

impl NetworkAttachmentRef {
    pub fn validate(&self) -> Result<(), ChvError> {
        require_non_empty("network.attachment_id", &self.attachment_id)?;
        require_non_empty("network.network_ref", &self.network_ref)?;
        if let Some(mac_address) = &self.mac_address {
            require_non_empty("network.mac_address", mac_address)?;
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNetworkAttachmentRef {
    attachment_id: String,
    network_ref: String,
    mac_address: Option<String>,
}

impl TryFrom<RawNetworkAttachmentRef> for NetworkAttachmentRef {
    type Error = ChvError;

    fn try_from(raw: RawNetworkAttachmentRef) -> Result<Self, Self::Error> {
        let value = Self {
            attachment_id: raw.attachment_id,
            network_ref: raw.network_ref,
            mac_address: raw.mac_address,
        };
        value.validate()?;
        Ok(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "RawVmDefinition")]
pub struct VmDefinition {
    pub id: VmId,
    pub name: String,
    pub boot: BootSpec,
    pub compute: ComputeSpec,
    pub storage: Vec<StorageAttachmentRef>,
    pub networks: Vec<NetworkAttachmentRef>,
    pub requested_power_state: RequestedPowerState,
    pub observed_power_state: ObservedPowerState,
    pub resource_version: ResourceVersion,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVmDefinition {
    id: VmId,
    name: String,
    boot: BootSpec,
    compute: ComputeSpec,
    storage: Vec<StorageAttachmentRef>,
    networks: Vec<NetworkAttachmentRef>,
    requested_power_state: RequestedPowerState,
    observed_power_state: ObservedPowerState,
    resource_version: ResourceVersion,
}

impl TryFrom<RawVmDefinition> for VmDefinition {
    type Error = ChvError;

    fn try_from(raw: RawVmDefinition) -> Result<Self, Self::Error> {
        let value = Self {
            id: raw.id,
            name: raw.name,
            boot: raw.boot,
            compute: raw.compute,
            storage: raw.storage,
            networks: raw.networks,
            requested_power_state: raw.requested_power_state,
            observed_power_state: raw.observed_power_state,
            resource_version: raw.resource_version,
        };
        value.validate()?;
        Ok(value)
    }
}

impl VmDefinition {
    pub fn validate(&self) -> Result<(), ChvError> {
        require_non_empty("vm.name", &self.name)?;
        self.boot.validate()?;
        self.compute.validate()?;
        for attachment in &self.storage {
            attachment.validate()?;
        }
        for attachment in &self.networks {
            attachment.validate()?;
        }
        let mut attachment_ids = std::collections::HashSet::new();
        for attachment_id in self
            .storage
            .iter()
            .map(|item| item.attachment_id.as_str())
            .chain(self.networks.iter().map(|item| item.attachment_id.as_str()))
        {
            if !attachment_ids.insert(attachment_id) {
                return Err(ChvError::InvalidArgument {
                    field: "vm.attachments".to_string(),
                    reason: format!("duplicate attachment_id {attachment_id}"),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "RawOperation")]
pub struct Operation {
    pub id: OperationId,
    pub kind: OperationKind,
    pub vm_id: VmId,
    pub status: OperationStatus,
    pub request_fingerprint: String,
    pub attempt_count: u32,
    pub max_attempts: u32,
}

impl Operation {
    pub fn validate(&self) -> Result<(), ChvError> {
        require_non_empty("operation.request_fingerprint", &self.request_fingerprint)?;
        if self.max_attempts == 0 {
            return Err(ChvError::InvalidArgument {
                field: "operation.max_attempts".to_string(),
                reason: "must be greater than zero".to_string(),
            });
        }
        if self.attempt_count > self.max_attempts {
            return Err(ChvError::InvalidArgument {
                field: "operation.attempt_count".to_string(),
                reason: "must not exceed max_attempts".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOperation {
    id: OperationId,
    kind: OperationKind,
    vm_id: VmId,
    status: OperationStatus,
    request_fingerprint: String,
    attempt_count: u32,
    max_attempts: u32,
}

impl TryFrom<RawOperation> for Operation {
    type Error = ChvError;

    fn try_from(raw: RawOperation) -> Result<Self, Self::Error> {
        let value = Self {
            id: raw.id,
            kind: raw.kind,
            vm_id: raw.vm_id,
            status: raw.status,
            request_fingerprint: raw.request_fingerprint,
            attempt_count: raw.attempt_count,
            max_attempts: raw.max_attempts,
        };
        value.validate()?;
        Ok(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "RawOperationStep")]
pub struct OperationStep {
    pub operation_id: OperationId,
    pub index: u32,
    pub name: String,
    pub status: StepStatus,
    pub attempt_count: u32,
    pub last_error: Option<String>,
}

impl OperationStep {
    pub fn validate(&self) -> Result<(), ChvError> {
        require_non_empty("operation_step.name", &self.name)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOperationStep {
    operation_id: OperationId,
    index: u32,
    name: String,
    status: StepStatus,
    attempt_count: u32,
    last_error: Option<String>,
}

impl TryFrom<RawOperationStep> for OperationStep {
    type Error = ChvError;

    fn try_from(raw: RawOperationStep) -> Result<Self, Self::Error> {
        let value = Self {
            operation_id: raw.operation_id,
            index: raw.index,
            name: raw.name,
            status: raw.status,
            attempt_count: raw.attempt_count,
            last_error: raw.last_error,
        };
        value.validate()?;
        Ok(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "RawOperationEvent")]
pub struct OperationEvent {
    pub id: EventId,
    pub sequence: u64,
    pub operation_id: Option<OperationId>,
    pub vm_id: Option<VmId>,
    pub kind: String,
    pub payload: serde_json::Value,
}

impl OperationEvent {
    pub fn validate(&self) -> Result<(), ChvError> {
        if self.sequence == 0 {
            return Err(ChvError::InvalidArgument {
                field: "event.sequence".to_string(),
                reason: "must be greater than zero".to_string(),
            });
        }
        require_non_empty("event.kind", &self.kind)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOperationEvent {
    id: EventId,
    sequence: u64,
    operation_id: Option<OperationId>,
    vm_id: Option<VmId>,
    kind: String,
    payload: serde_json::Value,
}

impl TryFrom<RawOperationEvent> for OperationEvent {
    type Error = ChvError;

    fn try_from(raw: RawOperationEvent) -> Result<Self, Self::Error> {
        let value = Self {
            id: raw.id,
            sequence: raw.sequence,
            operation_id: raw.operation_id,
            vm_id: raw.vm_id,
            kind: raw.kind,
            payload: raw.payload,
        };
        value.validate()?;
        Ok(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "RawOwnershipMarker")]
pub struct OwnershipMarker {
    pub vm_id: VmId,
    pub owner_id: HostId,
    pub ownership: OwnershipClass,
    pub recovery: RecoveryClass,
    pub marker_version: u32,
}

impl OwnershipMarker {
    pub fn validate(&self) -> Result<(), ChvError> {
        if self.marker_version == 0 {
            return Err(ChvError::InvalidArgument {
                field: "ownership.marker_version".to_string(),
                reason: "must be greater than zero".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOwnershipMarker {
    vm_id: VmId,
    owner_id: HostId,
    ownership: OwnershipClass,
    recovery: RecoveryClass,
    marker_version: u32,
}

impl TryFrom<RawOwnershipMarker> for OwnershipMarker {
    type Error = ChvError;

    fn try_from(raw: RawOwnershipMarker) -> Result<Self, Self::Error> {
        let value = Self {
            vm_id: raw.vm_id,
            owner_id: raw.owner_id,
            ownership: raw.ownership,
            recovery: raw.recovery,
            marker_version: raw.marker_version,
        };
        value.validate()?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_reject_empty_and_whitespace_values() {
        assert!(VmId::new("").is_err());
        assert!(HostId::new("  ").is_err());
        assert!(serde_json::from_str::<OperationId>(r#""""#).is_err());
    }

    #[test]
    fn resource_versions_start_at_one_and_increment_safely() {
        assert!(ResourceVersion::new(0).is_err());
        assert!(serde_json::from_str::<ResourceVersion>("0").is_err());
        let version = ResourceVersion::new(1).unwrap();
        assert_eq!(version.next().unwrap().get(), 2);
        assert!(ResourceVersion::new(u64::MAX).unwrap().next().is_err());
    }

    #[test]
    fn compute_and_boot_validation_reject_unusable_values() {
        assert!(ComputeSpec::new(0, 1024).is_err());
        assert!(ComputeSpec::new(1, 0).is_err());
        assert!(BootSpec::new(" ").is_err());
    }

    #[test]
    fn capabilities_default_to_false() {
        let capabilities = HostCapabilities::default();
        assert!(!capabilities.vm_definitions);
        assert!(!capabilities.power_start);
        assert!(!capabilities.power_stop);
        assert!(!capabilities.power_reboot);
        assert!(!capabilities.live_update_vcpus);
        assert!(!capabilities.live_update_memory);
        assert!(!capabilities.event_watch);
    }

    #[test]
    fn domain_contract_rejects_cloud_and_unknown_fields() {
        let input = r#"{
            "id":"vm-1",
            "name":"test",
            "boot":{"kernel":"kernel-ref","firmware":null,"initial_disk":null},
            "compute":{"vcpus":2,"memory_bytes":1073741824},
            "storage":[],
            "networks":[],
            "requested_power_state":"stopped",
            "observed_power_state":"unknown",
            "resource_version":1,
            "project_id":"must-not-enter-core"
        }"#;
        assert!(serde_json::from_str::<VmDefinition>(input).is_err());
    }

    #[test]
    fn vm_definition_round_trips() {
        let definition = VmDefinition {
            id: VmId::new("vm-1").unwrap(),
            name: "test".to_string(),
            boot: BootSpec::new("kernel-ref").unwrap(),
            compute: ComputeSpec::new(2, 1_073_741_824).unwrap(),
            storage: vec![StorageAttachmentRef {
                attachment_id: "disk-0".to_string(),
                storage_ref: "volume-1".to_string(),
                read_only: false,
            }],
            networks: vec![],
            requested_power_state: RequestedPowerState::Stopped,
            observed_power_state: ObservedPowerState::Unknown,
            resource_version: ResourceVersion::new(1).unwrap(),
        };
        let encoded = serde_json::to_string(&definition).unwrap();
        let decoded: VmDefinition = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, definition);
    }

    #[test]
    fn vm_definition_deserialization_rejects_invalid_nested_values() {
        let base = serde_json::json!({
            "id": "vm-1",
            "name": "test",
            "boot": {"kernel": "kernel-ref", "firmware": null, "initial_disk": null},
            "compute": {"vcpus": 2, "memory_bytes": 1024},
            "storage": [{"attachment_id": "disk-0", "storage_ref": "volume-1", "read_only": false}],
            "networks": [{"attachment_id": "nic-0", "network_ref": "network-1", "mac_address": null}],
            "requested_power_state": "stopped",
            "observed_power_state": "unknown",
            "resource_version": 1
        });
        for (pointer, invalid) in [
            ("/name", serde_json::json!(" ")),
            ("/boot/kernel", serde_json::json!("")),
            ("/compute/vcpus", serde_json::json!(0)),
            ("/compute/memory_bytes", serde_json::json!(0)),
            ("/storage/0/attachment_id", serde_json::json!("")),
            ("/storage/0/storage_ref", serde_json::json!(" ")),
            ("/networks/0/attachment_id", serde_json::json!("")),
            ("/networks/0/network_ref", serde_json::json!("")),
        ] {
            let mut candidate = base.clone();
            *candidate.pointer_mut(pointer).unwrap() = invalid;
            assert!(
                serde_json::from_value::<VmDefinition>(candidate).is_err(),
                "accepted invalid value at {pointer}"
            );
        }
    }

    #[test]
    fn vm_definition_rejects_attachment_ids_duplicated_across_kinds() {
        let mut definition = serde_json::from_value::<VmDefinition>(serde_json::json!({
            "id":"vm-1", "name":"test",
            "boot":{"kernel":"kernel","firmware":null,"initial_disk":null},
            "compute":{"vcpus":1,"memory_bytes":1},
            "storage":[{"attachment_id":"device-0","storage_ref":"volume","read_only":false}],
            "networks":[], "requested_power_state":"stopped",
            "observed_power_state":"unknown", "resource_version":1
        }))
        .unwrap();
        definition.networks.push(NetworkAttachmentRef {
            attachment_id: "device-0".to_owned(),
            network_ref: "network".to_owned(),
            mac_address: None,
        });
        assert!(definition.validate().is_err());
        assert!(
            serde_json::from_value::<VmDefinition>(serde_json::to_value(definition).unwrap())
                .is_err()
        );
    }

    #[test]
    fn operation_deserialization_enforces_fingerprint_and_attempt_bounds() {
        let base = serde_json::json!({
            "id": "op-1", "kind": "start_vm", "vm_id": "vm-1",
            "status": "accepted", "request_fingerprint": "sha256:value",
            "attempt_count": 0, "max_attempts": 3
        });
        for (pointer, invalid) in [
            ("/request_fingerprint", serde_json::json!("")),
            ("/max_attempts", serde_json::json!(0)),
            ("/attempt_count", serde_json::json!(4)),
        ] {
            let mut candidate = base.clone();
            *candidate.pointer_mut(pointer).unwrap() = invalid;
            assert!(serde_json::from_value::<Operation>(candidate).is_err());
        }
    }

    #[test]
    fn event_and_ownership_deserialization_reject_zero_versions() {
        let event = serde_json::json!({
            "id": "event-1", "sequence": 0, "operation_id": null,
            "vm_id": null, "kind": "accepted", "payload": {}
        });
        assert!(serde_json::from_value::<OperationEvent>(event).is_err());

        let marker = serde_json::json!({
            "vm_id": "vm-1", "owner_id": "host-1", "ownership": "cell_hv",
            "recovery": "owned_recoverable", "marker_version": 0
        });
        assert!(serde_json::from_value::<OwnershipMarker>(marker).is_err());
    }
}
