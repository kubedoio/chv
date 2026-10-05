//! Lossless translation from the legacy lifecycle RPC vocabulary to Core mutations.
//!
//! This adapter is wired in core-managed authority mode: the five legacy
//! lifecycle handlers call [`crate::AgentServer`]'s core-managed mutation path,
//! which routes through [`adapt_legacy_vm_mutation`]. The caller's audit
//! metadata (requester, external operation ID, request timestamp, legacy
//! desired generation) is carried into the `SubmitMutation` envelope and
//! durably journaled by `OperationService::submit`, never kept memory-only.

use crate::VmSpec;
use cellhv_core_operations::{MutationCommand, SubmitMutation};
use cellhv_core_types::{
    BootSpec, ComputeSpec, HypervisorTuning, IdempotencyKey, NetworkAttachmentRef, NicAddressing,
    ObservedPowerState, OperationId, OperationRequestMetadata, RequestedPowerState,
    ResourceVersion, StorageAttachmentRef, VmDefinition, VmId, LEGACY_OPERATION_ID_PREFIX,
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
        operation_id: legacy_operation_id(node_id, &vm_id, &meta.operation_id)?,
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
        metadata: OperationRequestMetadata {
            requested_by: meta.requested_by.clone(),
            external_operation_id: meta.operation_id.clone(),
            request_unix_ms: meta.request_unix_ms,
            legacy_generation: Some(desired_generation),
        },
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

/// Derives the Core operation id for a legacy control-plane task. This is
/// the durable task identity shared by every boundary that journals or
/// looks up the task's Core operation: the direct lifecycle handlers, the
/// desired-state dispatch shim, and (via [`legacy_requeue_operation_id`])
/// its re-drive twin. Length-prefixed segments keep the id unambiguous
/// for any safe-id content.
pub(crate) fn legacy_operation_id(
    node_id: &str,
    vm_id: &VmId,
    operation_id: &str,
) -> Result<OperationId, ChvError> {
    OperationId::new(format!(
        "{LEGACY_OPERATION_ID_PREFIX}{SCOPE_PREFIX}:node:{}:{node_id}:vm:{}:{vm_id}:operation:{}:{}",
        node_id.len(),
        vm_id.as_str().len(),
        operation_id.len(),
        operation_id
    ))
}

/// #368 C1: the re-drive twin of [`legacy_operation_id`] — the same
/// derivation with a trailing `:requeue` discriminator, so a re-drive
/// task's journaled operation can never collide with (or be mistaken
/// for) the original create task's operation under the same
/// control-plane operation id. The dispatch shim routes verbatim
/// retries on exactly this distinction: a retry of the original create
/// task replays the create, a retry of a re-drive task replays the
/// requeue, and neither is a fresh submission.
pub(crate) fn legacy_requeue_operation_id(
    node_id: &str,
    vm_id: &VmId,
    operation_id: &str,
) -> Result<OperationId, ChvError> {
    OperationId::new(format!(
        "{LEGACY_OPERATION_ID_PREFIX}{SCOPE_PREFIX}:node:{}:{node_id}:vm:{}:{vm_id}:operation:{}:{}:requeue",
        node_id.len(),
        vm_id.as_str().len(),
        operation_id.len(),
        operation_id
    ))
}

/// Translate a legacy generation-1 create task whose Core journal
/// already holds a terminally failed create for the SAME vm id into a
/// requeue submission (see `CoreStore::requeue_failed_create`).
///
/// Identity derivation mirrors [`adapt_legacy_vm_mutation`] — same scope
/// and metadata, from the caller's operation id and generation — with two
/// deliberate differences so a re-drive can never alias the original
/// create's journal identity: the operation id carries a `:requeue`
/// discriminator ([`legacy_requeue_operation_id`]), and the idempotency
/// key carries a `requeue` discriminator so it can NEVER resolve to the
/// original create's mapping (which would replay-return the failed
/// original instead of inserting the re-drive). Consequences:
///
/// - a dispatcher retry of the SAME re-drive task (same control-plane
///   operation id) converges on the one requeued Core operation;
/// - every NEW re-drive task (fresh control-plane operation id, as the
///   orchestrator's re-drive pass issues) derives a fresh operation id and
///   key, so each re-drive is a new journaled operation.
///
/// The create request envelope is NOT carried: the store re-derives it
/// from the live journal row inside the requeue transaction, so the
/// re-drive re-executes exactly the journaled spec (a generation-1 task is
/// fixed at accept and retried verbatim — a different spec would be a
/// different, refused generation).
pub fn adapt_legacy_create_redrive(
    meta: &LegacyRequestMeta,
    node_id: &str,
    vm_id: &str,
    expected_core_version: ResourceVersion,
) -> Result<cellhv_core_operations::RequeueCreateSubmission, ChvError> {
    require_non_empty("node_id", node_id)?;
    require_non_empty("target_node_id", &meta.target_node_id)?;
    if meta.target_node_id != node_id {
        return invalid("target_node_id", "must match request node_id");
    }
    require_non_empty("operation_id", &meta.operation_id)?;
    let desired_generation = parse_generation(&meta.desired_state_version)?;
    let vm_id = VmId::new(vm_id)?;
    Ok(cellhv_core_operations::RequeueCreateSubmission {
        vm_id: vm_id.clone(),
        expected_vm_version: expected_core_version,
        operation_id: legacy_requeue_operation_id(node_id, &vm_id, &meta.operation_id)?,
        idempotency_scope: format!(
            "{SCOPE_PREFIX}/node/{}:{node_id}/vm/{}:{vm_id}",
            node_id.len(),
            vm_id.as_str().len()
        ),
        idempotency_key: IdempotencyKey::new(format!(
            "operation/{}:{}/generation/{}:{}/requeue",
            meta.operation_id.len(),
            meta.operation_id,
            meta.desired_state_version.len(),
            meta.desired_state_version
        ))?,
        metadata: OperationRequestMetadata {
            requested_by: meta.requested_by.clone(),
            external_operation_id: meta.operation_id.clone(),
            request_unix_ms: meta.request_unix_ms,
            legacy_generation: Some(desired_generation),
        },
    })
}

/// Translate the legacy hypervisor override surface into the Core mirror,
/// field for field. Field parity with `chv_common::HypervisorOverrides` is
/// pinned by `hypervisor_tuning_parity_with_legacy_surface` below: adding a
/// field to either side without mirroring it fails that test (the fixture
/// is a fully populated struct literal, so the compiler forces it).
fn tuning_from_legacy(overrides: chv_common::hypervisor::HypervisorOverrides) -> HypervisorTuning {
    HypervisorTuning {
        cpu_nested: overrides.cpu_nested,
        cpu_amx: overrides.cpu_amx,
        cpu_kvm_hyperv: overrides.cpu_kvm_hyperv,
        memory_mergeable: overrides.memory_mergeable,
        memory_hugepages: overrides.memory_hugepages,
        memory_shared: overrides.memory_shared,
        memory_prefault: overrides.memory_prefault,
        iommu: overrides.iommu,
        rng_src: overrides.rng_src,
        watchdog: overrides.watchdog,
        landlock_enable: overrides.landlock_enable,
        serial_mode: overrides.serial_mode,
        console_mode: overrides.console_mode,
        pvpanic: overrides.pvpanic,
        tpm_type: overrides.tpm_type,
        tpm_socket_path: overrides.tpm_socket_path,
    }
}

fn convert_create_spec(vm_id: &str, spec: VmSpec) -> Result<VmDefinition, ChvError> {
    spec.validate()?;
    // Legacy fields Core does not model are still rejected explicitly —
    // never silently dropped. Cloud-init userdata, the hypervisor override
    // surface, disk sizing/seed, and control-plane NIC addressing are all
    // modeled in VmDefinition now and translate below. Tap configuration
    // remains runtime-owned and unsupported here.
    if spec.nics.iter().any(|nic| !nic.tap_name.is_empty()) {
        return unsupported("nics.tap_name");
    }

    let requested_power_state = match spec.desired_state.as_str() {
        "Running" => RequestedPowerState::Running,
        "Stopped" => RequestedPowerState::Stopped,
        _ => return invalid("desired_state", "must be Running or Stopped"),
    };
    let hypervisor_tuning = spec.hypervisor_overrides.map(tuning_from_legacy);
    // An empty (or whitespace) seed path is semantically absent — the
    // executor treats it that way, and normalizing here keeps the durable
    // definition free of Some("") sentinel values that older validators
    // would reject.
    let seed_path = spec
        .disk_seed_path
        .as_deref()
        .filter(|path| !path.trim().is_empty());
    let definition = VmDefinition {
        id: VmId::new(vm_id)?,
        name: spec.name,
        boot: BootSpec {
            kernel: spec.kernel_path,
            firmware: spec.firmware_path,
            initial_disk: seed_path.map(str::to_string),
        },
        compute: ComputeSpec::new(spec.cpus, spec.memory_bytes)?,
        storage: spec
            .disks
            .into_iter()
            .enumerate()
            .map(|(index, disk)| StorageAttachmentRef {
                attachment_id: legacy_storage_attachment_id(&disk.volume_id),
                storage_ref: disk.volume_id,
                read_only: disk.read_only,
                size_bytes: disk.size_bytes,
                // The legacy seed path is per-VM and applies to the boot
                // disk only — a deliberate divergence from the legacy
                // reconcile loop, which would seed every absent disk from
                // the same image; existing (already-provisioned) volumes
                // skip seeding in stord either way.
                seed_from: if index == 0 {
                    seed_path.map(str::to_string)
                } else {
                    None
                },
            })
            .collect(),
        networks: spec
            .nics
            .into_iter()
            .map(|nic| {
                let addressing =
                    if nic.ip_address.is_empty() && nic.cidr.is_empty() && nic.gateway.is_empty() {
                        None
                    } else {
                        Some(NicAddressing {
                            ip_address: nic.ip_address,
                            cidr: nic.cidr,
                            gateway: nic.gateway,
                        })
                    };
                NetworkAttachmentRef {
                    attachment_id: legacy_network_attachment_id(vm_id, &nic.network_id),
                    network_ref: nic.network_id,
                    mac_address: Some(nic.mac_address),
                    addressing,
                    // Normalize a blank snapshot to None, matching the
                    // serde TryFrom path — the durable definition never
                    // carries Some("") sentinels. (Empty ARRAYS ride
                    // through; the executor's emptiness gate is the
                    // safety net for those.)
                    firewall_policy_json: nic.firewall_policy_json.filter(|p| !p.trim().is_empty()),
                }
            })
            .collect(),
        requested_power_state,
        observed_power_state: ObservedPowerState::Unknown,
        resource_version: ResourceVersion::new(1).expect("one is a valid resource version"),
        cloud_init_userdata: spec.cloud_init_userdata,
        hypervisor_tuning,
    };
    definition.validate()?;
    Ok(definition)
}

/// Parses a legacy desired-state generation string. Shared by the direct
/// mutation adapter and the desired-state dispatch shim so both boundaries
/// enforce one canonical form: a positive decimal integer with no leading
/// zeros, sign, or other non-canonical decoration.
pub(crate) fn parse_generation(raw: &str) -> Result<u64, ChvError> {
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
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use cellhv_core_api::router;
    use cellhv_core_operations::{Acceptance, AuthorityActor, OperationService};
    use cellhv_core_types::{HostId, HostIdentity};
    use http_body_util::BodyExt;
    use std::os::unix::fs::PermissionsExt;
    use tower::ServiceExt;

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
        assert_eq!(
            first.submission.metadata,
            OperationRequestMetadata {
                requested_by: "controller-a".to_owned(),
                external_operation_id: "op-42".to_owned(),
                request_unix_ms: 1_700_000_000_000,
                legacy_generation: Some(7),
            }
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
        // Tap configuration is runtime-owned: still rejected explicitly
        // rather than silently dropped.
        let mut create_meta = meta();
        create_meta.desired_state_version = "1".into();
        let mut spec = minimal_spec();
        spec.nics.push(NicSpec {
            network_id: "network-a".into(),
            mac_address: "02:00:00:00:00:01".into(),
            ip_address: String::new(),
            tap_name: "tap-leftover".into(),
            cidr: String::new(),
            gateway: String::new(),
            firewall_policy_json: None,
        });
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
    fn create_translates_the_full_legacy_spec_shape() {
        // The BFF's build_agent_vm_spec always emits disk sizes, control-plane
        // NIC addressing, and merged hypervisor overrides; all of it is
        // modeled in VmDefinition now and must translate losslessly.
        // (#355: the network's firewall policy snapshot rides the same
        // path — verify it below.)
        let mut create_meta = meta();
        create_meta.desired_state_version = "1".into();
        let mut spec = minimal_spec();
        spec.disk_seed_path = Some("/var/lib/chv/images/ubuntu.img".into());
        spec.disks.push(DiskSpec {
            volume_id: "volume-a".into(),
            read_only: false,
            size_bytes: Some(10_737_418_240),
            backend_class: None,
        });
        spec.nics.push(NicSpec {
            network_id: "network-a".into(),
            mac_address: "02:00:00:00:00:01".into(),
            ip_address: "10.200.0.47".into(),
            tap_name: String::new(),
            cidr: "10.200.0.0/24".into(),
            gateway: "10.200.0.1".into(),
            firewall_policy_json: Some(
                r#"[{"direction":"inbound","action":"accept","protocol":"icmp"}]"#.into(),
            ),
        });
        spec.cloud_init_userdata = Some("#cloud-config".into());
        spec.hypervisor_overrides = Some(chv_common::hypervisor::HypervisorOverrides {
            cpu_nested: Some(true),
            rng_src: Some("/dev/hwrng".into()),
            ..Default::default()
        });
        let result = adapt_legacy_vm_mutation(
            &create_meta,
            "node-a",
            LegacyVmMutation::Create {
                vm_id: "vm-a".into(),
                spec: Box::new(spec),
            },
            version(1),
        )
        .unwrap();
        let MutationCommand::CreateVm { definition } = result.submission.command else {
            panic!("expected create command")
        };
        assert_eq!(
            definition.boot.initial_disk.as_deref(),
            Some("/var/lib/chv/images/ubuntu.img")
        );
        assert_eq!(definition.storage[0].size_bytes, Some(10_737_418_240));
        assert_eq!(
            definition.storage[0].seed_from.as_deref(),
            Some("/var/lib/chv/images/ubuntu.img")
        );
        let addressing = definition.networks[0]
            .addressing
            .as_ref()
            .expect("addressing must be carried");
        assert_eq!(addressing.ip_address, "10.200.0.47");
        assert_eq!(addressing.cidr, "10.200.0.0/24");
        assert_eq!(addressing.gateway, "10.200.0.1");
        assert_eq!(
            definition.networks[0].firewall_policy_json.as_deref(),
            Some(r#"[{"direction":"inbound","action":"accept","protocol":"icmp"}]"#),
            "the firewall policy snapshot must reach the Core definition (#355)"
        );
        assert_eq!(
            definition.cloud_init_userdata.as_deref(),
            Some("#cloud-config")
        );
        let tuning = definition
            .hypervisor_tuning
            .as_ref()
            .expect("tuning must be carried");
        assert_eq!(tuning.cpu_nested, Some(true));
        assert_eq!(tuning.rng_src.as_deref(), Some("/dev/hwrng"));
    }

    #[test]
    fn create_normalizes_an_empty_seed_path_to_absent() {
        // An empty (or whitespace) seed path is semantically absent — the
        // executor treats it that way, and the durable definition must not
        // carry Some("") sentinel values.
        let mut create_meta = meta();
        create_meta.desired_state_version = "1".into();
        let mut spec = minimal_spec();
        spec.disk_seed_path = Some("   ".to_owned());
        spec.disks.push(DiskSpec {
            volume_id: "volume-a".into(),
            read_only: false,
            size_bytes: None,
            backend_class: None,
        });
        let result = adapt_legacy_vm_mutation(
            &create_meta,
            "node-a",
            LegacyVmMutation::Create {
                vm_id: "vm-a".into(),
                spec: Box::new(spec),
            },
            version(1),
        )
        .unwrap();
        let MutationCommand::CreateVm { definition } = result.submission.command else {
            panic!("expected create command")
        };
        assert_eq!(definition.boot.initial_disk, None);
        assert_eq!(definition.storage[0].seed_from, None);
    }

    #[test]
    fn hypervisor_tuning_parity_with_legacy_surface() {
        // Fully populated on BOTH sides so the compiler forces this fixture
        // to grow whenever either struct gains a field. The total check is
        // the JSON-equality assertion below: both structs share field names
        // and 1:1 types, so a faithful translation reproduces the input
        // object exactly — any dropped, added, or swapped field breaks it.
        // The alternating/distinctive values additionally make the first
        // diverging field identifiable in the assertion diff.
        let overrides = chv_common::hypervisor::HypervisorOverrides {
            cpu_nested: Some(true),
            cpu_amx: Some(false),
            cpu_kvm_hyperv: Some(true),
            memory_mergeable: Some(false),
            memory_hugepages: Some(true),
            memory_shared: Some(false),
            memory_prefault: Some(true),
            iommu: Some(false),
            rng_src: Some("/dev/hwrng".to_string()),
            watchdog: Some(true),
            landlock_enable: Some(false),
            serial_mode: Some("Null".to_string()),
            console_mode: Some("Pty".to_string()),
            pvpanic: Some(true),
            tpm_type: Some("swtpm".to_string()),
            tpm_socket_path: Some("/run/tpm.sock".to_string()),
        };
        let tuning = tuning_from_legacy(overrides.clone());
        assert_eq!(
            serde_json::to_value(&tuning).unwrap(),
            serde_json::to_value(&overrides).unwrap(),
            "tuning_from_legacy must reproduce the legacy object field for field"
        );
        // And the empty surface stays empty.
        assert_eq!(
            tuning_from_legacy(chv_common::hypervisor::HypervisorOverrides::default()),
            HypervisorTuning::default()
        );
        // Spot fields keep their readable failure messages.
        assert_eq!(tuning.cpu_nested, Some(true));
        assert_eq!(tuning.cpu_amx, Some(false));
        assert_eq!(tuning.rng_src.as_deref(), Some("/dev/hwrng"));
        assert_eq!(tuning.serial_mode.as_deref(), Some("Null"));
        assert_eq!(tuning.console_mode.as_deref(), Some("Pty"));
        assert_eq!(tuning.tpm_socket_path.as_deref(), Some("/run/tpm.sock"));
    }

    #[test]
    fn core_tuning_validation_matches_the_legacy_boundary_rules() {
        // Rule parity: every hypervisor-override value the legacy spec
        // boundary accepts or rejects must get the same verdict from the
        // Core mirror's validation (VmDefinition::validate calls it at every
        // Core boundary — direct submissions, journal replays, migrations).
        // chv-agent-core can see both rule sets; cellhv-core-types cannot
        // (it must stay independent of the legacy surface), so the parity
        // is pinned HERE.
        let cases = [
            // Invalid on both sides.
            chv_common::hypervisor::HypervisorOverrides {
                rng_src: Some(String::new()),
                ..Default::default()
            },
            chv_common::hypervisor::HypervisorOverrides {
                rng_src: Some("relative/path".to_string()),
                ..Default::default()
            },
            chv_common::hypervisor::HypervisorOverrides {
                serial_mode: Some("Hero".to_string()),
                ..Default::default()
            },
            chv_common::hypervisor::HypervisorOverrides {
                console_mode: Some("Hero".to_string()),
                ..Default::default()
            },
            chv_common::hypervisor::HypervisorOverrides {
                tpm_type: Some("tpm2".to_string()),
                ..Default::default()
            },
            chv_common::hypervisor::HypervisorOverrides {
                tpm_socket_path: Some("/run/tpm.sock".to_string()),
                ..Default::default()
            },
            // Valid on both sides.
            chv_common::hypervisor::HypervisorOverrides::default(),
            chv_common::hypervisor::HypervisorOverrides {
                rng_src: Some("/dev/hwrng".to_string()),
                serial_mode: Some("Null".to_string()),
                console_mode: Some("Pty".to_string()),
                tpm_type: Some("swtpm".to_string()),
                tpm_socket_path: Some("/run/tpm.sock".to_string()),
                cpu_nested: Some(true),
                ..Default::default()
            },
        ];
        for (index, case) in cases.into_iter().enumerate() {
            let mut spec = minimal_spec();
            spec.hypervisor_overrides = Some(case.clone());
            let legacy_verdict = spec.validate().is_ok();
            let core_verdict = tuning_from_legacy(case).validate().is_ok();
            assert_eq!(
                legacy_verdict, core_verdict,
                "case {index}: legacy boundary says {legacy_verdict}, Core mirror says {core_verdict}"
            );
        }
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
            backend_class: None,
        });
        spec.nics.push(NicSpec {
            network_id: "network-a".into(),
            mac_address: "02:00:00:00:00:01".into(),
            ip_address: String::new(),
            tap_name: String::new(),
            cidr: String::new(),
            gateway: String::new(),
            firewall_policy_json: None,
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

    #[tokio::test]
    async fn translated_legacy_mutation_can_enter_shared_unwired_actor() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let service = OperationService::create_new(
            &directory.path().join("core.db"),
            &HostIdentity {
                id: HostId::new("node-a").unwrap(),
                resource_version: version(1),
            },
        )
        .unwrap();
        let (authority, join) = AuthorityActor::spawn(service, 1).unwrap();
        let mut create_meta = meta();
        create_meta.desired_state_version = "1".into();
        let intent = adapt_legacy_vm_mutation(
            &create_meta,
            "node-a",
            LegacyVmMutation::Create {
                vm_id: "vm-a".into(),
                spec: Box::new(minimal_spec()),
            },
            version(1),
        )
        .unwrap();
        // The intent's own audit fields must agree with the submission's
        // durable metadata by construction.
        assert_eq!(
            intent.submission.metadata,
            OperationRequestMetadata {
                requested_by: intent.requested_by.clone(),
                external_operation_id: intent.external_operation_id.clone(),
                request_unix_ms: intent.request_unix_ms,
                legacy_generation: Some(intent.version.desired_generation),
            }
        );

        let expected_metadata = intent.submission.metadata.clone();
        let accepted = authority.submit(intent.submission).await.unwrap();
        assert_eq!(accepted.disposition, Acceptance::Accepted);
        assert_eq!(authority.operations().await.unwrap().len(), 1);
        assert_eq!(authority.vms().await.unwrap().len(), 1);
        let journaled = authority
            .operation(
                OperationId::new(
                    "legacy:control-plane-node.v1:node:6:node-a:vm:4:vm-a:operation:5:op-42",
                )
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(journaled.request_metadata, Some(expected_metadata));
        authority.shutdown().await.unwrap();
        join.join().await.unwrap();
    }

    #[tokio::test]
    async fn native_and_legacy_adapter_share_one_operation_journal() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let service = OperationService::create_new(
            &directory.path().join("core.db"),
            &HostIdentity {
                id: HostId::new("node-a").unwrap(),
                resource_version: version(1),
            },
        )
        .unwrap();
        let (authority, owner) = AuthorityActor::spawn(service, 16).unwrap();
        let app = router(authority.clone());
        let native_create = serde_json::json!({
            "request_id": "native-create",
            "definition": {
                "id": "vm-a",
                "name": "guest",
                "boot": {"kernel": "/kernel", "firmware": null, "initial_disk": null},
                "compute": {"vcpus": 1, "memory_bytes": 1048576},
                "storage": [],
                "networks": [],
                "requested_power_state": "stopped",
                "observed_power_state": "unknown",
                "resource_version": 1
            }
        });
        let response = app
            .clone()
            .oneshot(
                Request::post("/v1/vms")
                    .header("content-type", "application/json")
                    .header("idempotency-key", "native-create")
                    .body(Body::from(serde_json::to_vec(&native_create).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);

        let mut legacy_meta = meta();
        legacy_meta.operation_id = "legacy-start".to_owned();
        legacy_meta.desired_state_version = "2".to_owned();
        let legacy = adapt_legacy_vm_mutation(
            &legacy_meta,
            "node-a",
            LegacyVmMutation::Start {
                vm_id: "vm-a".to_owned(),
            },
            version(1),
        )
        .unwrap();
        authority.submit(legacy.submission).await.unwrap();

        let response = app
            .clone()
            .oneshot(Request::get("/v1/operations").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let operations: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(operations.as_array().unwrap().len(), 2);
        assert_eq!(operations[0]["operation"]["id"], "native:v1:native-create");
        assert!(operations[1]["operation"]["id"]
            .as_str()
            .unwrap()
            .starts_with(&format!(
                "{LEGACY_OPERATION_ID_PREFIX}control-plane-node.v1:"
            )));

        drop(app);
        authority.shutdown().await.unwrap();
        owner.join().await.unwrap();
    }
}
