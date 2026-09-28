//! Integration tests for the single full-side-effect Core runtime (M2.2a).
//!
//! These drive `CloudHypervisorCoreRuntime::execute` with real journal entries
//! whose `request` is the canonical envelope (as the executor would produce),
//! using `MockHostResourceController` + `MockCloudHypervisorAdapter` so no
//! real stord/nwd/KVM is needed. They also smoke the public mock exports for
//! cross-crate use.

use cellhv_core_executor::{CoreVmRuntime, RuntimeFailure};
use cellhv_core_operations::{MutationCommand, OperationJournalEntry};
use cellhv_core_types::{
    BootSpec, ComputeSpec, NetworkAttachmentRef, ObservedPowerState, Operation, OperationId,
    OperationKind, OperationStatus, RequestedPowerState, ResourceVersion, StorageAttachmentRef,
    VmDefinition, VmId,
};
use chv_agent_runtime_ch::core_runtime::CloudHypervisorCoreRuntime;
use chv_agent_runtime_ch::mock::{MockCloudHypervisorAdapter, MockHostResourceController};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

fn definition(vm_id: &str, nvols: usize, nnics: usize) -> VmDefinition {
    let storage = (0..nvols)
        .map(|i| StorageAttachmentRef {
            attachment_id: format!("vol-{i}"),
            storage_ref: format!("vol-{i}"),
            read_only: false,
        })
        .collect();
    let networks = (0..nnics)
        .map(|i| NetworkAttachmentRef {
            attachment_id: format!("{vm_id}-net{i}"),
            network_ref: format!("net-{i}"),
            mac_address: Some(format!("02:00:00:00:00:{i:02x}")),
        })
        .collect();
    VmDefinition {
        id: VmId::new(vm_id).expect("vm id"),
        name: format!("{vm_id}-guest"),
        boot: BootSpec::new("/kernel").expect("boot"),
        compute: ComputeSpec::new(2, 1024).expect("compute"),
        storage,
        networks,
        requested_power_state: RequestedPowerState::Stopped,
        observed_power_state: ObservedPowerState::Unknown,
        resource_version: ResourceVersion::new(1).expect("version"),
    }
}

/// Build the canonical envelope JSON exactly as `canonical_request` does:
/// `{"command": <internally-tagged command>, "expected_vm_version": N}`.
fn envelope(command: MutationCommand) -> serde_json::Value {
    serde_json::json!({
        "command": command,
        "expected_vm_version": 1,
    })
}

/// Build a journal entry carrying the given request value.
fn entry(
    kind: OperationKind,
    vm_id: &str,
    op_id: &str,
    request: serde_json::Value,
) -> OperationJournalEntry {
    OperationJournalEntry {
        operation: Operation {
            id: OperationId::new(op_id).expect("op id"),
            kind,
            vm_id: VmId::new(vm_id).expect("vm id"),
            status: OperationStatus::Running,
            request_fingerprint: "fingerprint".to_string(),
            attempt_count: 1,
            max_attempts: 3,
        },
        request,
        result: None,
        error: None,
        request_metadata: None,
        recovery_assessment: None,
    }
}

struct Harness {
    _dir: TempDir,
    runtime_dir: PathBuf,
    adapter: Arc<MockCloudHypervisorAdapter>,
    controller: Arc<MockHostResourceController>,
    runtime: Arc<CloudHypervisorCoreRuntime>,
}

fn harness(fail_on: Option<&str>) -> Harness {
    let dir = tempfile::tempdir().expect("tempdir");
    let runtime_dir = dir.path().join("runtime");
    let adapter = Arc::new(MockCloudHypervisorAdapter::default());
    let controller = Arc::new(match fail_on {
        Some(step) => MockHostResourceController::new_with_fail(step),
        None => MockHostResourceController::new(),
    });
    let runtime = Arc::new(CloudHypervisorCoreRuntime::new(
        adapter.clone(),
        controller.clone(),
        runtime_dir.clone(),
    ));
    Harness {
        _dir: dir,
        runtime_dir,
        adapter,
        controller,
        runtime,
    }
}

fn calls(controller: &MockHostResourceController) -> Vec<String> {
    controller.calls.lock().expect("calls lock").clone()
}

fn is_subsequence(haystack: &[String], needle: &[&str]) -> bool {
    let mut hay = haystack.iter();
    for want in needle {
        loop {
            match hay.next() {
                Some(got) if got == want => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

fn vm_dir_path(h: &Harness, vm_id: &str) -> PathBuf {
    h.runtime_dir.join("vms").join(vm_id)
}

#[tokio::test]
async fn create_vm_performs_full_side_effects() {
    let h = harness(None);
    let vm_id = "vm-a";
    let command = MutationCommand::CreateVm {
        definition: definition(vm_id, 2, 1),
    };
    let result = h
        .runtime
        .execute(entry(
            OperationKind::CreateVm,
            vm_id,
            "op-create",
            envelope(command),
        ))
        .await;
    assert!(result.is_ok(), "create must succeed: {result:?}");

    // Full side-effect sequencing: open+attach per volume in order, then
    // ensure+attach_nic per network.
    let log = calls(&h.controller);
    assert!(
        is_subsequence(
            &log,
            &["open:vol-0", "attach:vol-0", "open:vol-1", "attach:vol-1"]
        ),
        "volume open/attach ordering: {log:?}"
    );
    assert!(
        is_subsequence(&log, &["ensure:net-0", "attach_nic:vm-a-net-0"]),
        "network ensure/attach ordering: {log:?}"
    );

    // Adapter received the exact config derived from the Core definition.
    let vms = h.adapter.vms.lock().expect("vms lock");
    let config = vms
        .get(vm_id)
        .expect("vm must be present in the adapter map");
    assert_eq!(
        config.api_socket_path,
        h.runtime_dir.join("vms").join(vm_id).join("vm.sock")
    );
    assert_eq!(config.cpus, 2);
    assert_eq!(config.memory_bytes, 1024);
    assert_eq!(config.kernel_path, PathBuf::from("/kernel"));
    assert_eq!(config.disks.len(), 2, "two disks");
    assert_eq!(config.disks[0].path, PathBuf::from("/dev/mock/vol-0"));
    assert_eq!(config.disks[0].id.as_deref(), Some("vol-0"));
    assert_eq!(config.disks[1].path, PathBuf::from("/dev/mock/vol-1"));
    assert_eq!(config.disks[1].id.as_deref(), Some("vol-1"));
    assert_eq!(config.nics.len(), 1, "one nic");
    assert_eq!(config.nics[0].tap_name, "tap-vm-a-net-0");
    assert_eq!(config.nics[0].mac_address, "02:00:00:00:00:00");
    assert_eq!(config.nics[0].cidr, "10.0.0.0/24");
    drop(vms);

    // VM runtime dir exists with mode 0o775.
    let vm_dir = vm_dir_path(&h, vm_id);
    let metadata = std::fs::metadata(&vm_dir).expect("vm dir exists");
    assert_eq!(
        metadata.permissions().mode() & 0o777,
        0o775,
        "vm dir must be 0o775"
    );
}

#[tokio::test]
async fn create_vm_failure_midway_unwinds() {
    // Fail when opening the 2nd volume: volume 1 is opened+attached and must be
    // detached+closed, volume 2's open fails so nothing to clean, and the VM
    // dir is removed.
    let h = harness(Some("open:2"));
    let vm_id = "vm-b";
    let command = MutationCommand::CreateVm {
        definition: definition(vm_id, 2, 1),
    };
    let result = h
        .runtime
        .execute(entry(
            OperationKind::CreateVm,
            vm_id,
            "op-fail",
            envelope(command),
        ))
        .await;
    assert!(result.is_err(), "create must fail: {result:?}");

    // The adapter must never have been asked to create the VM.
    assert!(
        !h.adapter.vms.lock().expect("vms lock").contains_key(vm_id),
        "adapter must not contain the failed VM"
    );

    // No leaked handles: every opened volume has a matching close, every
    // attached nic a matching detach_nic.
    let log = calls(&h.controller);
    assert!(
        is_subsequence(&log, &["open:vol-0", "attach:vol-0", "detach:vol-0"]),
        "first volume must be closed: {log:?}"
    );
    let opened: Vec<&str> = log.iter().filter_map(|c| c.strip_prefix("open:")).collect();
    for volume in &opened {
        assert!(
            log.iter().any(|c| c == &format!("close:{volume}")),
            "no close for opened volume {volume}: {log:?}"
        );
    }
    let attached_nics: Vec<&str> = log
        .iter()
        .filter_map(|c| c.strip_prefix("attach_nic:"))
        .collect();
    for nic in &attached_nics {
        assert!(
            log.iter().any(|c| c == &format!("detach_nic:{nic}")),
            "no detach_nic for attached nic {nic}: {log:?}"
        );
    }
    // VM dir removed (best-effort teardown).
    assert!(
        !vm_dir_path(&h, vm_id).exists(),
        "vm dir should be removed after failed create"
    );
}

#[tokio::test]
async fn delete_vm_drains_side_effects() {
    let h = harness(None);
    let vm_id = "vm-c";
    let create = MutationCommand::CreateVm {
        definition: definition(vm_id, 2, 1),
    };
    h.runtime
        .execute(entry(
            OperationKind::CreateVm,
            vm_id,
            "op-create",
            envelope(create),
        ))
        .await
        .expect("create succeeds");

    let delete = MutationCommand::DeleteVm {
        vm_id: VmId::new(vm_id).expect("vm id"),
    };
    let result = h
        .runtime
        .execute(entry(
            OperationKind::DeleteVm,
            vm_id,
            "op-del",
            envelope(delete),
        ))
        .await;
    assert!(result.is_ok(), "delete must succeed: {result:?}");
    assert!(
        !h.adapter.vms.lock().expect("vms lock").contains_key(vm_id),
        "adapter must no longer contain the VM"
    );

    // Volumes drained in reverse (detach+close each), then NICs detached.
    let log = calls(&h.controller);
    assert!(
        is_subsequence(
            &log,
            &[
                "detach:vol-1",
                "close:vol-1",
                "detach:vol-0",
                "close:vol-0",
                "detach_nic:vm-c-net-0",
            ]
        ),
        "delete drain ordering: {log:?}"
    );
    let opened: Vec<&str> = log.iter().filter_map(|c| c.strip_prefix("open:")).collect();
    let closed: Vec<&str> = log
        .iter()
        .filter_map(|c| c.strip_prefix("close:"))
        .collect();
    assert_eq!(
        opened.len(),
        closed.len(),
        "every opened volume must be closed: {log:?}"
    );
    for volume in &opened {
        assert!(
            closed.contains(volume),
            "no close for opened volume {volume}: {log:?}"
        );
    }
}

#[tokio::test]
async fn create_then_restart_delete_without_state() {
    let h = harness(None);
    let vm_id = "vm-d";
    let create = MutationCommand::CreateVm {
        definition: definition(vm_id, 1, 1),
    };
    h.runtime
        .execute(entry(
            OperationKind::CreateVm,
            vm_id,
            "op-create",
            envelope(create),
        ))
        .await
        .expect("create succeeds");

    // Simulate an agent restart: a FRESH runtime with an empty in-memory map
    // (same adapter + controller — the daemons/CH are still up).
    let restarted = Arc::new(CloudHypervisorCoreRuntime::new(
        h.adapter.clone(),
        h.controller.clone(),
        h.runtime_dir.clone(),
    ));
    let delete = MutationCommand::DeleteVm {
        vm_id: VmId::new(vm_id).expect("vm id"),
    };
    let result = restarted
        .execute(entry(
            OperationKind::DeleteVm,
            vm_id,
            "op-del",
            envelope(delete),
        ))
        .await;
    // Delete still succeeds (the missing handle state is a logged crash
    // residual, not an op failure); the adapter VM is gone.
    assert!(
        result.is_ok(),
        "delete after restart must succeed: {result:?}"
    );
    assert!(
        !h.adapter.vms.lock().expect("vms lock").contains_key(vm_id),
        "adapter must no longer contain the VM"
    );
    // No drain was possible (fresh runtime has no handle state).
    let log = calls(&h.controller);
    assert!(
        !log.iter()
            .any(|c| c.starts_with("detach:") || c.starts_with("close:")),
        "no handle drain expected without in-memory state: {log:?}"
    );
}

#[tokio::test]
async fn start_stop_reboot_passthrough() {
    let h = harness(None);
    let vm_id = "vm-e";

    let start = MutationCommand::StartVm {
        vm_id: VmId::new(vm_id).expect("vm id"),
    };
    h.runtime
        .execute(entry(
            OperationKind::StartVm,
            vm_id,
            "op-start",
            envelope(start),
        ))
        .await
        .expect("start succeeds");

    let stop = MutationCommand::StopVm {
        vm_id: VmId::new(vm_id).expect("vm id"),
    };
    h.runtime
        .execute(entry(
            OperationKind::StopVm,
            vm_id,
            "op-stop",
            envelope(stop),
        ))
        .await
        .expect("stop succeeds");

    let reboot = MutationCommand::RebootVm {
        vm_id: VmId::new(vm_id).expect("vm id"),
    };
    h.runtime
        .execute(entry(
            OperationKind::RebootVm,
            vm_id,
            "op-reboot",
            envelope(reboot),
        ))
        .await
        .expect("reboot succeeds");
}

#[tokio::test]
async fn update_attach_detach_are_unsupported() {
    let h = harness(None);
    let vm_id = "vm-f";
    let vm = VmId::new(vm_id).expect("vm id");
    let cases: Vec<(OperationKind, MutationCommand)> = vec![
        (
            OperationKind::UpdateVm,
            MutationCommand::UpdateVm {
                definition: definition(vm_id, 0, 0),
            },
        ),
        (
            OperationKind::AttachVolume,
            MutationCommand::AttachVolume {
                vm_id: vm.clone(),
                attachment: StorageAttachmentRef {
                    attachment_id: "vol-x".to_string(),
                    storage_ref: "vol-x".to_string(),
                    read_only: false,
                },
            },
        ),
        (
            OperationKind::DetachVolume,
            MutationCommand::DetachVolume {
                vm_id: vm.clone(),
                attachment_id: "vol-x".to_string(),
            },
        ),
        (
            OperationKind::AttachNetwork,
            MutationCommand::AttachNetwork {
                vm_id: vm.clone(),
                attachment: NetworkAttachmentRef {
                    attachment_id: format!("{vm_id}-netx"),
                    network_ref: "net-x".to_string(),
                    mac_address: None,
                },
            },
        ),
        (
            OperationKind::DetachNetwork,
            MutationCommand::DetachNetwork {
                vm_id: vm.clone(),
                attachment_id: format!("{vm_id}-netx"),
            },
        ),
    ];
    for (op_index, (kind, command)) in cases.into_iter().enumerate() {
        let result = h
            .runtime
            .execute(entry(
                kind,
                vm_id,
                &format!("op-{}-{}", kind_name(kind), op_index),
                envelope(command),
            ))
            .await;
        assert!(
            matches!(result, Err(RuntimeFailure::Unsupported)),
            "{kind:?} must be Unsupported, got {result:?}"
        );
    }
}

fn kind_name(kind: OperationKind) -> &'static str {
    match kind {
        OperationKind::UpdateVm => "update",
        OperationKind::AttachVolume => "attach-volume",
        OperationKind::DetachVolume => "detach-volume",
        OperationKind::AttachNetwork => "attach-network",
        OperationKind::DetachNetwork => "detach-network",
        _ => "other",
    }
}

#[tokio::test]
async fn malformed_envelope_is_invalid_request() {
    let h = harness(None);
    let request = serde_json::json!({"not_a_command": true, "unexpected": "shape"});
    let result = h
        .runtime
        .execute(entry(
            OperationKind::CreateVm,
            "vm-g",
            "op-malformed",
            request,
        ))
        .await;
    assert!(
        matches!(result, Err(RuntimeFailure::InvalidRequest)),
        "malformed request must be InvalidRequest, got {result:?}"
    );
}

#[tokio::test]
async fn create_vm_kind_with_non_create_command_is_invalid() {
    let h = harness(None);
    // A CreateVm-kind operation whose envelope carries a DeleteVm command is a
    // journal-integrity failure, not a user mistake.
    let command = MutationCommand::DeleteVm {
        vm_id: VmId::new("vm-x").expect("vm id"),
    };
    let result = h
        .runtime
        .execute(entry(
            OperationKind::CreateVm,
            "vm-kind",
            "op-kind",
            envelope(command),
        ))
        .await;
    assert!(
        matches!(result, Err(RuntimeFailure::InvalidRequest)),
        "CreateVm-kind with a non-create command must be InvalidRequest, got {result:?}"
    );
    assert!(
        calls(&h.controller).is_empty(),
        "no side effects may run for a mismatched command"
    );
}

#[tokio::test]
async fn create_unwind_never_attached_volume_is_close_only() {
    // Fail attaching the 2nd volume. Volume 1 is opened+attached and must be
    // detached+closed; volume 2 is opened but NEVER attached, so its unwind
    // must be close-only (HostResourceController contract: a never-attached
    // volume is closed without a detach).
    let h = harness(Some("attach:2"));
    let vm_id = "vm-closeonly";
    let command = MutationCommand::CreateVm {
        definition: definition(vm_id, 2, 0),
    };
    let result = h
        .runtime
        .execute(entry(
            OperationKind::CreateVm,
            vm_id,
            "op-closeonly",
            envelope(command),
        ))
        .await;
    assert!(result.is_err(), "create must fail: {result:?}");

    let log = calls(&h.controller);
    // First volume attached: detach+close.
    assert!(
        is_subsequence(
            &log,
            &["open:vol-0", "attach:vol-0", "detach:vol-0", "close:vol-0"]
        ),
        "attached volume must detach+close: {log:?}"
    );
    // Second volume's attach never happened -> close only, NEVER detach.
    assert!(
        log.iter().any(|c| c == "close:vol-1"),
        "never-attached volume must still be closed: {log:?}"
    );
    assert!(
        !log.iter().any(|c| c == "detach:vol-1"),
        "never-attached volume must NOT be detached (contract violation): {log:?}"
    );
    assert!(
        !log.iter().any(|c| c == "attach:vol-1"),
        "second attach must not have succeeded: {log:?}"
    );
    assert!(
        !h.adapter.vms.lock().expect("vms lock").contains_key(vm_id),
        "adapter must not contain the failed VM"
    );
    assert!(
        !vm_dir_path(&h, vm_id).exists(),
        "vm dir should be removed after failed create"
    );
}

#[tokio::test]
async fn delete_drains_side_effects_when_adapter_delete_fails() {
    let h = harness(None);
    let vm_id = "vm-drainfail";
    let create = MutationCommand::CreateVm {
        definition: definition(vm_id, 2, 1),
    };
    h.runtime
        .execute(entry(
            OperationKind::CreateVm,
            vm_id,
            "op-create",
            envelope(create),
        ))
        .await
        .expect("create succeeds");
    assert_eq!(
        h.runtime.debug_side_effects_len(),
        1,
        "create tracks one entry"
    );

    // Make the hypervisor delete fail deterministically.
    *h.adapter.fail_delete.lock().unwrap() = true;
    let delete = MutationCommand::DeleteVm {
        vm_id: VmId::new(vm_id).expect("vm id"),
    };
    let result = h
        .runtime
        .execute(entry(
            OperationKind::DeleteVm,
            vm_id,
            "op-del-fail",
            envelope(delete.clone()),
        ))
        .await;
    assert!(
        matches!(result, Err(RuntimeFailure::NotFound)),
        "failed hypervisor delete must surface NotFound (mapped from ChvError::NotFound), got {result:?}"
    );
    // The VM was NOT removed by the mock (delete failed).
    assert!(
        h.adapter.vms.lock().expect("vms lock").contains_key(vm_id),
        "adapter VM must survive a failed delete for retry"
    );

    // Drain ran DESPITE the delete failure: every volume detach+close, every
    // nic detached, and the tracked entry is kept for a later retry.
    let log = calls(&h.controller);
    assert!(
        is_subsequence(
            &log,
            &[
                "detach:vol-1",
                "close:vol-1",
                "detach:vol-0",
                "close:vol-0",
                "detach_nic:vm-drainfail-net-0",
            ]
        ),
        "failed delete must still drain side effects: {log:?}"
    );
    assert_eq!(
        h.runtime.debug_side_effects_len(),
        1,
        "tracked entry must survive a failed delete for retry"
    );

    // A SECOND delete (failure cleared) succeeds and finishes the drain.
    *h.adapter.fail_delete.lock().unwrap() = false;
    let result = h
        .runtime
        .execute(entry(
            OperationKind::DeleteVm,
            vm_id,
            "op-del-retry",
            envelope(delete),
        ))
        .await;
    assert!(result.is_ok(), "retry delete must succeed: {result:?}");
    assert!(
        !h.adapter.vms.lock().expect("vms lock").contains_key(vm_id),
        "adapter VM must be gone after the retry delete"
    );
    assert_eq!(
        h.runtime.debug_side_effects_len(),
        0,
        "no entry may remain after a successful delete"
    );
    let log = calls(&h.controller);
    let detaches: Vec<&str> = log
        .iter()
        .filter_map(|c| c.strip_prefix("detach:"))
        .collect();
    assert!(
        detaches.iter().filter(|v| v == &&"vol-0").count() >= 2
            && detaches.iter().filter(|v| v == &&"vol-1").count() >= 2,
        "retry delete must drain the retained entry again: {log:?}"
    );
}

#[tokio::test]
async fn delete_removes_side_effects_entry_after_success() {
    let h = harness(None);
    let vm_id = "vm-entries";
    let create = MutationCommand::CreateVm {
        definition: definition(vm_id, 1, 1),
    };
    h.runtime
        .execute(entry(
            OperationKind::CreateVm,
            vm_id,
            "op-create",
            envelope(create),
        ))
        .await
        .expect("create succeeds");
    assert_eq!(h.runtime.debug_side_effects_len(), 1);

    let delete = MutationCommand::DeleteVm {
        vm_id: VmId::new(vm_id).expect("vm id"),
    };
    let result = h
        .runtime
        .execute(entry(
            OperationKind::DeleteVm,
            vm_id,
            "op-del",
            envelope(delete),
        ))
        .await;
    assert!(result.is_ok(), "delete must succeed: {result:?}");
    assert_eq!(
        h.runtime.debug_side_effects_len(),
        0,
        "successful delete must remove the tracked entry"
    );
}

/// Build a CreateVm envelope whose `request` carries the given raw definition
/// JSON for public-path tests that verify end-to-end rejection of path-unsafe
/// ids (no side effects, no fs mutation). Layer A (`VmDefinition` serde
/// try_from → validate) already rejects these inputs at deserialization, so the
/// Layer-B `is_safe_resource_id`/`verify_vm_dir_within_base` guards are
/// defense-in-depth, covered only by the dedicated in-crate unit tests.
fn create_envelope_with_raw_definition(definition: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "command": {
            "command": "create_vm",
            "definition": definition,
        },
        "expected_vm_version": 1,
    })
}

fn raw_definition(id: &str, storage: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "name": "evil",
        "boot": {"kernel": "/kernel", "firmware": null, "initial_disk": null},
        "compute": {"vcpus": 2, "memory_bytes": 1024},
        "storage": storage,
        "networks": [],
        "requested_power_state": "stopped",
        "observed_power_state": "unknown",
        "resource_version": 1,
    })
}

#[tokio::test]
async fn create_rejects_path_unsafe_vm_id() {
    let h = harness(None);
    let request = create_envelope_with_raw_definition(raw_definition(
        "../../../etc/evil",
        serde_json::json!([]),
    ));
    let result = h
        .runtime
        .execute(entry(
            OperationKind::CreateVm,
            "vm-safe-op",
            "op-evil-id",
            request,
        ))
        .await;
    assert!(
        matches!(result, Err(RuntimeFailure::InvalidRequest)),
        "path-unsafe vm id must be InvalidRequest, got {result:?}"
    );
    assert!(
        calls(&h.controller).is_empty(),
        "no side effects may run for a path-unsafe vm id"
    );
    // No fs mutation: the runtime never created `{runtime_dir}/vms`.
    assert!(
        !h.runtime_dir.join("vms").exists(),
        "no vms tree may be created for a path-unsafe vm id"
    );
}

#[tokio::test]
async fn create_rejects_path_unsafe_storage_ref() {
    let h = harness(None);
    let request = create_envelope_with_raw_definition(raw_definition(
        "vm-safe",
        serde_json::json!([{
            "attachment_id": "disk-0",
            "storage_ref": "../vol-0",
            "read_only": false,
        }]),
    ));
    let result = h
        .runtime
        .execute(entry(
            OperationKind::CreateVm,
            "vm-safe",
            "op-evil-storage",
            request,
        ))
        .await;
    assert!(
        matches!(result, Err(RuntimeFailure::InvalidRequest)),
        "path-unsafe storage_ref must be InvalidRequest, got {result:?}"
    );
    assert!(
        calls(&h.controller).is_empty(),
        "no side effects may run for a path-unsafe storage_ref"
    );
    assert!(
        !h.runtime_dir.join("vms").exists(),
        "no vms tree may be created for a path-unsafe storage_ref"
    );
}
