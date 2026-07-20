//! Lossless translation from the legacy lifecycle RPC vocabulary to Core mutations.
//!
//! This module is deliberately not called by [`crate::AgentServer`]. Until the
//! Core journal and the legacy [`crate::NodeCache`] can be updated atomically,
//! routing production requests through it would create two partially committed
//! views of desired state.

use crate::VmSpec;
use cellhv_core_operations::{MutationCommand, SubmitMutation};
use cellhv_core_types::{
    BootSpec, ComputeSpec, IdempotencyKey, NetworkAttachmentRef, ObservedPowerState, OperationId,
    RequestedPowerState, ResourceVersion, StorageAttachmentRef, VmDefinition, VmId,
};
use cellhv_nodecache_migration::{legacy_network_attachment_id, legacy_storage_attachment_id};
use chv_errors::ChvError;

const SCOPE_PREFIX: &str = "control-plane-node.v1";

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyRequestMeta {
    pub operation_id: String,
    pub requested_by: String,
    pub target_node_id: String,
    pub desired_state_version: String,
    pub request_unix_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyVersionContext {
    pub desired_generation: u64,
    pub expected_core_version: ResourceVersion,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyMutationIntent {
    pub external_operation_id: String,
    pub requested_by: String,
    pub request_unix_ms: i64,
    pub version: LegacyVersionContext,
    pub submission: SubmitMutation,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LegacyVmMutation {
    Create { vm_id: String, spec: Box<VmSpec> },
    Start { vm_id: String },
    Stop { vm_id: String, force: bool },
    Reboot { vm_id: String, force: bool },
    Delete { vm_id: String, force: bool },
}

/// Translate a legacy request without executing or persisting it.
///
/// The scope includes the target node and VM, while the key includes both the
/// caller's operation ID and generation. Replays are therefore stable and the
/// same operation ID cannot accidentally alias a different desired generation.
pub fn adapt_legacy_vm_mutation(
    meta: &LegacyRequestMeta,
    node_id: &str,
    mutation: LegacyVmMutation,
    expected_core_version: ResourceVersion,
) -> Result<LegacyMutationIntent, ChvError> {
    require_non_empty("node_id", node_id)?;
    require_non_empty("target_node_id", &meta.target_node_id)?;
    if meta.target_node_id != node_id {
        return invalid("target_node_id", "must match request node_id");
    }

    require_non_empty("operation_id", &meta.operation_id)?;
    let desired_generation = parse_generation(&meta.desired_state_version)?;
    let (vm_id, command) = match mutation {
        LegacyVmMutation::Create { vm_id, spec } => {
            if expected_core_version.get() != 1 {
                return invalid("expected_core_version", "create requires Core version 1");
            }
            let definition = convert_create_spec(&vm_id, *spec)?;
            (
                definition.id.clone(),
                MutationCommand::CreateVm { definition },
            )
        }
        LegacyVmMutation::Start { vm_id } => {
            let vm_id = VmId::new(vm_id)?;
            (vm_id.clone(), MutationCommand::StartVm { vm_id })
        }
        LegacyVmMutation::Stop { vm_id, force } => {
            reject_force(force, "stop")?;
            let vm_id = VmId::new(vm_id)?;
            (vm_id.clone(), MutationCommand::StopVm { vm_id })
        }
        LegacyVmMutation::Reboot { vm_id, force } => {
            reject_force(force, "reboot")?;
            let vm_id = VmId::new(vm_id)?;
            (vm_id.clone(), MutationCommand::RebootVm { vm_id })
        }
        LegacyVmMutation::Delete { vm_id, force } => {
            reject_force(force, "delete")?;
            let vm_id = VmId::new(vm_id)?;
            (vm_id.clone(), MutationCommand::DeleteVm { vm_id })
        }
    };

    let submission = SubmitMutation {
        operation_id: OperationId::new(format!(
            "legacy:{SCOPE_PREFIX}:node:{}:{node_id}:vm:{}:{vm_id}:operation:{}:{}",
            node_id.len(),
            vm_id.as_str().len(),
            meta.operation_id.len(),
            meta.operation_id
        ))?,
        idempotency_scope: format!(
            "{SCOPE_PREFIX}/node/{}:{node_id}/vm/{}:{vm_id}",
            node_id.len(),
            vm_id.as_str().len()
        ),
        idempotency_key: IdempotencyKey::new(format!(
            "operation/{}:{}/generation/{}:{}",
            meta.operation_id.len(),
            meta.operation_id,
            meta.desired_state_version.len(),
            meta.desired_state_version
        ))?,
        expected_vm_version: expected_core_version,
        command,
    };
    Ok(LegacyMutationIntent {
        external_operation_id: meta.operation_id.clone(),
        requested_by: meta.requested_by.clone(),
        request_unix_ms: meta.request_unix_ms,
        version: LegacyVersionContext {
            desired_generation,
            expected_core_version,
        },
        submission,
    })
}

fn convert_create_spec(vm_id: &str, spec: VmSpec) -> Result<VmDefinition, ChvError> {
    spec.validate()?;
    if spec.cloud_init_userdata.is_some() {
        return unsupported("cloud_init_userdata");
    }
    if spec.hypervisor_overrides.is_some() {
        return unsupported("hypervisor_overrides");
    }
    if spec.disks.iter().any(|disk| disk.size_bytes.is_some()) {
        return unsupported("disks.size_bytes");
    }
    if spec.nics.iter().any(|nic| {
        !nic.ip_address.is_empty()
            || !nic.tap_name.is_empty()
            || !nic.cidr.is_empty()
            || !nic.gateway.is_empty()
    }) {
        return unsupported("NIC addressing or tap configuration");
    }

    let requested_power_state = match spec.desired_state.as_str() {
        "Running" => RequestedPowerState::Running,
        "Stopped" => RequestedPowerState::Stopped,
        _ => return invalid("desired_state", "must be Running or Stopped"),
    };
    let definition = VmDefinition {
        id: VmId::new(vm_id)?,
        name: spec.name,
        boot: BootSpec {
            kernel: spec.kernel_path,
            firmware: spec.firmware_path,
            initial_disk: spec.disk_seed_path,
        },
        compute: ComputeSpec::new(spec.cpus, spec.memory_bytes)?,
        storage: spec
            .disks
            .into_iter()
            .map(|disk| StorageAttachmentRef {
                attachment_id: legacy_storage_attachment_id(&disk.volume_id),
                storage_ref: disk.volume_id,
                read_only: disk.read_only,
            })
            .collect(),
        networks: spec
            .nics
            .into_iter()
            .map(|nic| NetworkAttachmentRef {
                attachment_id: legacy_network_attachment_id(vm_id, &nic.network_id),
                network_ref: nic.network_id,
                mac_address: Some(nic.mac_address),
            })
            .collect(),
        requested_power_state,
        observed_power_state: ObservedPowerState::Unknown,
        resource_version: ResourceVersion::new(1).expect("one is a valid resource version"),
    };
    definition.validate()?;
    Ok(definition)
}

fn parse_generation(raw: &str) -> Result<u64, ChvError> {
    let value = raw.parse::<u64>().map_err(|_| ChvError::InvalidArgument {
        field: "desired_state_version".to_owned(),
        reason: "must be a canonical positive decimal integer".to_owned(),
    })?;
    if value == 0 || value.to_string() != raw {
        return invalid(
            "desired_state_version",
            "must be a canonical positive decimal integer",
        );
    }
    Ok(value)
}

fn reject_force(force: bool, operation: &str) -> Result<(), ChvError> {
    if force {
        return unsupported(&format!("forced {operation}"));
    }
    Ok(())
}

fn require_non_empty(field: &str, value: &str) -> Result<(), ChvError> {
    if value.trim().is_empty() {
        return invalid(field, "must not be empty");
    }
    Ok(())
}

fn invalid<T>(field: &str, reason: &str) -> Result<T, ChvError> {
    Err(ChvError::InvalidArgument {
        field: field.to_owned(),
        reason: reason.to_owned(),
    })
}

fn unsupported<T>(feature: &str) -> Result<T, ChvError> {
    Err(ChvError::InvalidArgument {
        field: "legacy_vm_mutation".to_owned(),
        reason: format!("cannot losslessly map unsupported field: {feature}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DiskSpec, NicSpec};

    fn meta() -> LegacyRequestMeta {
        LegacyRequestMeta {
            operation_id: "op-42".to_owned(),
            requested_by: "controller-a".to_owned(),
            target_node_id: "node-a".to_owned(),
            desired_state_version: "7".to_owned(),
            request_unix_ms: 1_700_000_000_000,
        }
    }

    fn version(value: u64) -> ResourceVersion {
        ResourceVersion::new(value).unwrap()
    }

    fn minimal_spec() -> VmSpec {
        VmSpec {
            name: "guest".to_owned(),
            cpus: 2,
            memory_bytes: 1024,
            kernel_path: "/kernel".to_owned(),
            firmware_path: Some("/firmware".to_owned()),
            disk_seed_path: None,
            disks: vec![],
            nics: vec![],
            desired_state: "Stopped".to_owned(),
            cloud_init_userdata: None,
            hypervisor_overrides: None,
        }
    }

    #[test]
    fn lifecycle_mapping_has_deterministic_scope_and_key() {
        let first = adapt_legacy_vm_mutation(
            &meta(),
            "node-a",
            LegacyVmMutation::Start {
                vm_id: "vm-a".into(),
            },
            version(3),
        )
        .unwrap();
        let second = adapt_legacy_vm_mutation(
            &meta(),
            "node-a",
            LegacyVmMutation::Start {
                vm_id: "vm-a".into(),
            },
            version(3),
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.submission.idempotency_scope,
            "control-plane-node.v1/node/6:node-a/vm/4:vm-a"
        );
        assert_eq!(
            first.submission.idempotency_key.as_str(),
            "operation/5:op-42/generation/1:7"
        );
    }

    #[test]
    fn create_maps_the_lossless_subset() {
        let result = adapt_legacy_vm_mutation(
            &meta(),
            "node-a",
            LegacyVmMutation::Create {
                vm_id: "vm-a".into(),
                spec: Box::new(minimal_spec()),
            },
            version(1),
        )
        .unwrap();
        let MutationCommand::CreateVm { definition } = result.submission.command else {
            panic!("expected create command")
        };
        assert_eq!(definition.boot.firmware.as_deref(), Some("/firmware"));
        assert_eq!(
            definition.requested_power_state,
            RequestedPowerState::Stopped
        );
        assert_eq!(definition.observed_power_state, ObservedPowerState::Unknown);
        assert_eq!(definition.resource_version, version(1));
        assert_eq!(result.version.desired_generation, 7);
        assert_eq!(result.requested_by, "controller-a");
        assert_eq!(result.external_operation_id, "op-42");
    }

    #[test]
    fn rejects_non_numeric_generation_and_target_mismatch() {
        let mut invalid_meta = meta();
        invalid_meta.desired_state_version = "latest".into();
        assert!(adapt_legacy_vm_mutation(
            &invalid_meta,
            "node-a",
            LegacyVmMutation::Start {
                vm_id: "vm-a".into()
            },
            version(3)
        )
        .is_err());
        assert!(adapt_legacy_vm_mutation(
            &meta(),
            "node-b",
            LegacyVmMutation::Start {
                vm_id: "vm-a".into()
            },
            version(3)
        )
        .is_err());
    }

    #[test]
    fn rejects_fields_that_core_cannot_preserve() {
        for mutation in [
            LegacyVmMutation::Stop {
                vm_id: "vm-a".into(),
                force: true,
            },
            LegacyVmMutation::Delete {
                vm_id: "vm-a".into(),
                force: true,
            },
        ] {
            assert!(adapt_legacy_vm_mutation(&meta(), "node-a", mutation, version(3)).is_err());
        }
        let mut create_meta = meta();
        create_meta.desired_state_version = "1".into();
        let mut spec = minimal_spec();
        spec.cloud_init_userdata = Some("secret".into());
        assert!(adapt_legacy_vm_mutation(
            &create_meta,
            "node-a",
            LegacyVmMutation::Create {
                vm_id: "vm-a".into(),
                spec: Box::new(spec)
            },
            version(1)
        )
        .is_err());
    }

    #[test]
    fn rejects_noncanonical_generation_and_wrong_create_core_version() {
        let mut leading_zero = meta();
        leading_zero.desired_state_version = "07".into();
        assert!(adapt_legacy_vm_mutation(
            &leading_zero,
            "node-a",
            LegacyVmMutation::Start {
                vm_id: "vm-a".into()
            },
            version(3),
        )
        .is_err());
        assert!(adapt_legacy_vm_mutation(
            &meta(),
            "node-a",
            LegacyVmMutation::Create {
                vm_id: "vm-a".into(),
                spec: Box::new(minimal_spec()),
            },
            version(2),
        )
        .is_err());
    }

    #[test]
    fn create_attachment_ids_equal_nodecache_migration_projection() {
        let mut spec = minimal_spec();
        spec.disks.push(DiskSpec {
            volume_id: "volume-a".into(),
            read_only: true,
            size_bytes: None,
        });
        spec.nics.push(NicSpec {
            network_id: "network-a".into(),
            mac_address: "02:00:00:00:00:01".into(),
            ip_address: String::new(),
            tap_name: String::new(),
            cidr: String::new(),
            gateway: String::new(),
        });
        let intent = adapt_legacy_vm_mutation(
            &meta(),
            "node-a",
            LegacyVmMutation::Create {
                vm_id: "vm-a".into(),
                spec: Box::new(spec),
            },
            version(1),
        )
        .unwrap();
        let MutationCommand::CreateVm { definition } = intent.submission.command else {
            panic!("expected create command")
        };
        assert_eq!(
            definition.storage[0].attachment_id,
            legacy_storage_attachment_id("volume-a")
        );
        assert_eq!(
            definition.networks[0].attachment_id,
            legacy_network_attachment_id("vm-a", "network-a")
        );
    }
}
