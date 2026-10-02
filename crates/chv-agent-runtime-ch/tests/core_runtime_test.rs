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
use chv_hypervisor_api::resources::{vm_api_socket, vm_config_file, vm_pid_file};
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
            size_bytes: None,
            seed_from: None,
        })
        .collect();
    let networks = (0..nnics)
        .map(|i| NetworkAttachmentRef {
            attachment_id: format!("{vm_id}-net{i}"),
            network_ref: format!("net-{i}"),
            mac_address: Some(format!("02:00:00:00:00:{i:02x}")),
            addressing: None,
            firewall_policy_json: None,
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
        cloud_init_userdata: None,
        hypervisor_tuning: None,
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
    // Without control-plane addressing the NIC stays unassigned and the
    // gateway empty (topology defaults), and no tuning/userdata is invented.
    assert_eq!(config.nics[0].ip_address, "");
    assert_eq!(config.nics[0].gateway, "");
    assert_eq!(config.cloud_init_userdata, None);
    assert_eq!(config.hypervisor_overrides, None);
    drop(vms);
    // No provisioning hints on the definition: both volumes open bare.
    let options = h.controller.open_options.lock().expect("options lock");
    assert_eq!(options.len(), 2, "two opens");
    assert!(options[0].1.is_empty(), "no hints on vol-0: {options:?}");
    assert!(options[1].1.is_empty(), "no hints on vol-1: {options:?}");
    drop(options);

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
async fn create_vm_carries_provisioning_hints_addressing_and_tuning() {
    let h = harness(None);
    let vm_id = "vm-tuned";
    let mut def = definition(vm_id, 2, 1);
    // Boot disk: sized + seeded; second disk: pre-provisioned (no hints).
    def.storage[0].size_bytes = Some(10_737_418_240);
    def.storage[0].seed_from = Some("/var/lib/chv/images/ubuntu.img".to_string());
    def.networks[0].addressing = Some(cellhv_core_types::NicAddressing {
        ip_address: "10.200.0.47".to_string(),
        cidr: "10.200.0.0/24".to_string(),
        gateway: "10.200.0.1".to_string(),
    });
    def.cloud_init_userdata = Some("#cloud-config".to_string());
    def.hypervisor_tuning = Some(cellhv_core_types::HypervisorTuning {
        cpu_nested: Some(true),
        rng_src: Some("/dev/hwrng".to_string()),
        ..Default::default()
    });
    let command = MutationCommand::CreateVm { definition: def };
    let result = h
        .runtime
        .execute(entry(
            OperationKind::CreateVm,
            vm_id,
            "op-create-tuned",
            envelope(command),
        ))
        .await;
    assert!(result.is_ok(), "create must succeed: {result:?}");

    // Provisioning hints reach the storage layer exactly once, on the boot
    // disk; the pre-provisioned second disk opens without options.
    let options = h
        .controller
        .open_options
        .lock()
        .expect("options lock")
        .clone();
    assert_eq!(options.len(), 2, "two opens: {options:?}");
    assert_eq!(options[0].0, "vol-0");
    assert_eq!(
        options[0].1.get("size_bytes").map(String::as_str),
        Some("10737418240")
    );
    assert_eq!(
        options[0].1.get("seed_from").map(String::as_str),
        Some("/var/lib/chv/images/ubuntu.img")
    );
    assert_eq!(options[1].0, "vol-1");
    assert!(options[1].1.is_empty(), "no hints on vol-1: {options:?}");

    // Addressing and tuning reach the hypervisor config verbatim.
    let vms = h.adapter.vms.lock().expect("vms lock");
    let config = vms
        .get(vm_id)
        .expect("vm must be present in the adapter map");
    assert_eq!(config.nics[0].ip_address, "10.200.0.47");
    assert_eq!(config.nics[0].cidr, "10.200.0.0/24");
    assert_eq!(config.nics[0].gateway, "10.200.0.1");
    assert_eq!(config.cloud_init_userdata.as_deref(), Some("#cloud-config"));
    let overrides = config
        .hypervisor_overrides
        .as_ref()
        .expect("tuning must reach the hypervisor config");
    assert_eq!(overrides.cpu_nested, Some(true));
    assert_eq!(overrides.rng_src.as_deref(), Some("/dev/hwrng"));
}

#[tokio::test]
async fn create_vm_applies_attach_time_firewall_policy() {
    // #355: a policy snapshot carried on the network attachment is
    // applied via nwd AFTER the topology is ensured (nwd scopes the
    // policy to the ensured topology's CHV-owned interfaces and fails
    // closed without them) and BEFORE the NIC attaches (no guest traffic
    // outside the CHV boundary). The recorded policy is the snapshot
    // verbatim.
    let h = harness(None);
    let vm_id = "vm-policy";
    let policy = r#"[{"direction":"inbound","action":"accept","protocol":"icmp"}]"#;
    let mut def = definition(vm_id, 1, 1);
    def.networks[0].firewall_policy_json = Some(policy.to_string());
    let command = MutationCommand::CreateVm { definition: def };
    let result = h
        .runtime
        .execute(entry(
            OperationKind::CreateVm,
            vm_id,
            "op-create-policy",
            envelope(command),
        ))
        .await;
    assert!(result.is_ok(), "create must succeed: {result:?}");

    let calls = h.controller.calls.lock().expect("calls lock").clone();
    let ensure = calls
        .iter()
        .position(|c| c.starts_with("ensure:net-0"))
        .expect("topology ensure must be recorded");
    let policy_at = calls
        .iter()
        .position(|c| c.starts_with("policy:net-0:") && c.ends_with(policy))
        .expect("policy application must be recorded with the snapshot verbatim");
    let attach = calls
        .iter()
        .position(|c| c.starts_with("attach_nic:"))
        .expect("nic attach must be recorded");
    assert!(
        ensure < policy_at && policy_at < attach,
        "policy must apply after ensure and before the nic attaches: {calls:?}"
    );
}

#[tokio::test]
async fn policy_version_is_content_derived_and_stable_across_nics_and_retries() {
    // The policy_version passed to nwd is bookkeeping for later
    // re-scoping, but it must be CONTENT-derived so that retries (and
    // multiple NICs on the same network within one create) are
    // idempotent — the same snapshot always yields the same version.
    // One set_firewall_policy is issued per NIC (network-scoped RPC,
    // cheap and idempotent).
    let h = harness(None);
    let policy = r#"[{"direction":"inbound","action":"accept","protocol":"tcp"}]"#;

    // Two NICs on DIFFERENT networks (net-0, net-1) carrying the SAME
    // snapshot, in one create.
    let vm_id = "vm-two-nics";
    let mut def = definition(vm_id, 1, 2);
    def.networks[0].firewall_policy_json = Some(policy.to_string());
    def.networks[1].firewall_policy_json = Some(policy.to_string());
    let result = h
        .runtime
        .execute(entry(
            OperationKind::CreateVm,
            vm_id,
            "op-two-nics",
            envelope(MutationCommand::CreateVm { definition: def }),
        ))
        .await;
    assert!(result.is_ok(), "create must succeed: {result:?}");

    // A retry-equivalent: a second VM with the same snapshot.
    let vm_id = "vm-two-nics-2";
    let mut def = definition(vm_id, 1, 1);
    def.networks[0].firewall_policy_json = Some(policy.to_string());
    let result = h
        .runtime
        .execute(entry(
            OperationKind::CreateVm,
            vm_id,
            "op-two-nics-2",
            envelope(MutationCommand::CreateVm { definition: def }),
        ))
        .await;
    assert!(result.is_ok(), "create must succeed: {result:?}");

    let calls = h.controller.calls.lock().expect("calls lock").clone();
    let policy_calls: Vec<&String> = calls
        .iter()
        .filter(|c| c.starts_with("policy:") && c.ends_with(policy))
        .collect();
    assert_eq!(
        policy_calls.len(),
        3,
        "one policy application per NIC (2 nics + 1 on the second VM): {calls:?}"
    );
    // Records are "policy:<network_id>:<version>:<json>"; the version is
    // the field between the network id and the known json suffix.
    let suffix = format!(":{policy}");
    let versions: std::collections::HashSet<&str> = policy_calls
        .iter()
        .map(|c| {
            let net_start = "policy:".len();
            let net_end = net_start + c[net_start..].find(':').expect("network delimiter") + 1;
            &c[net_end..c.len() - suffix.len()]
        })
        .collect();
    assert_eq!(
        versions.len(),
        1,
        "the same snapshot must always yield the same content-derived version: {calls:?}"
    );
    assert!(
        versions.iter().all(|v| v.starts_with("attach-")),
        "versions must carry the attach- prefix: {calls:?}"
    );
}

#[tokio::test]
async fn create_vm_without_policy_snapshot_skips_policy_application() {
    // The safety half of #355: no snapshot (or a SEMANTICALLY empty one —
    // blank string, whitespace, or an empty JSON array) → NO policy
    // call. nwd's engine engages default-deny even for an empty
    // ruleset; applying one to a rule-less network would cut its guests
    // off entirely (including DHCP). Rule-less networks keep the
    // bare-table behavior.
    let h = harness(None);

    for (vm_id, snapshot) in [
        ("vm-nopolicy", None),
        ("vm-emptylist", Some("[]")),
        ("vm-blank", Some("")),
        ("vm-whitespace", Some("   ")),
    ] {
        let mut def = definition(vm_id, 1, 1);
        def.networks[0].firewall_policy_json = snapshot.map(str::to_string);
        let result = h
            .runtime
            .execute(entry(
                OperationKind::CreateVm,
                vm_id,
                &format!("op-create-{vm_id}"),
                envelope(MutationCommand::CreateVm { definition: def }),
            ))
            .await;
        assert!(result.is_ok(), "create must succeed: {result:?}");
    }

    let calls = h.controller.calls.lock().expect("calls lock").clone();
    assert!(
        !calls.iter().any(|c| c.starts_with("policy:")),
        "no policy application without a non-empty snapshot: {calls:?}"
    );
    assert_eq!(
        calls
            .iter()
            .filter(|c| c.starts_with("ensure:net-0"))
            .count(),
        4,
        "topology is still ensured for every VM: {calls:?}"
    );
}

#[tokio::test]
async fn create_vm_defaults_empty_nic_cidr_like_the_legacy_path() {
    // A network row without a CIDR makes the BFF emit an empty cidr next to
    // an assigned ip (build_agent_vm_spec uses unwrap_or_default); the Core
    // executor must default it to the topology default exactly like the
    // legacy reconcile path instead of routing an empty CIDR to nwd.
    let h = harness(None);
    let vm_id = "vm-nocidr";
    let mut def = definition(vm_id, 1, 1);
    def.networks[0].addressing = Some(cellhv_core_types::NicAddressing {
        ip_address: "10.200.0.47".to_string(),
        cidr: String::new(),
        gateway: String::new(),
    });
    let command = MutationCommand::CreateVm { definition: def };
    let result = h
        .runtime
        .execute(entry(
            OperationKind::CreateVm,
            vm_id,
            "op-create-nocidr",
            envelope(command),
        ))
        .await;
    assert!(result.is_ok(), "create must succeed: {result:?}");
    let vms = h.adapter.vms.lock().expect("vms lock");
    let config = vms
        .get(vm_id)
        .expect("vm must be present in the adapter map");
    assert_eq!(config.nics[0].cidr, "10.0.0.0/24");
    assert_eq!(config.nics[0].ip_address, "10.200.0.47");
    assert_eq!(config.nics[0].gateway, "");
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
async fn delete_after_force_stop_is_idempotent() {
    // Run 8d of the M2.5 qualification: the force-stop path removes the
    // adapter's map entry (pre-existing force-stop semantics), so a delete
    // issued afterwards finds no runtime entry and the adapter reports
    // NotFound. The VM exists at the authority and its runtime dir is on
    // disk — the delete must complete the adapter-owned artifact cleanup
    // itself and succeed, not fail NOT_FOUND on every retry.
    let h = harness(None);
    let vm_id = "vm-force";

    // The force-stopped state: no adapter entry (the force path removed it),
    // but the runtime dir with the adapter's artifacts and the
    // storage-layer volume backing.
    let vm_dir = vm_dir_path(&h, vm_id);
    std::fs::create_dir_all(&vm_dir).expect("vm dir");
    std::fs::write(vm_pid_file(&vm_dir), "12345").expect("pid file");
    std::fs::write(vm_config_file(&vm_dir), "{}").expect("persisted config");
    std::fs::write(vm_api_socket(&vm_dir), b"").expect("api socket file");
    std::fs::write(vm_dir.join("vol-0.img"), b"disk").expect("volume backing");

    let delete = MutationCommand::DeleteVm {
        vm_id: VmId::new(vm_id).expect("vm id"),
    };
    let result = h
        .runtime
        .execute(entry(
            OperationKind::DeleteVm,
            vm_id,
            "op-del-force",
            envelope(delete),
        ))
        .await;
    assert!(
        result.is_ok(),
        "delete after force stop must succeed: {result:?}"
    );
    assert!(!vm_pid_file(&vm_dir).exists(), "pid file must be removed");
    assert!(
        !vm_config_file(&vm_dir).exists(),
        "persisted config must be removed"
    );
    assert!(
        !vm_api_socket(&vm_dir).exists(),
        "api socket must be removed"
    );
    assert!(
        vm_dir.join("vol-0.img").exists(),
        "storage-layer files must survive the runtime delete"
    );
}

#[tokio::test]
async fn delete_without_runtime_dir_stays_not_found() {
    // No adapter entry AND no runtime dir: the VM never ran on this node.
    // NotFound must surface — a delete misrouted to the wrong node must
    // not be silently swallowed as an idempotent success.
    let h = harness(None);
    let vm_id = "vm-never";

    let delete = MutationCommand::DeleteVm {
        vm_id: VmId::new(vm_id).expect("vm id"),
    };
    let result = h
        .runtime
        .execute(entry(
            OperationKind::DeleteVm,
            vm_id,
            "op-del-never",
            envelope(delete),
        ))
        .await;
    assert!(
        matches!(result, Err(RuntimeFailure::NotFound)),
        "delete without any runtime footprint must stay NotFound, got {result:?}"
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
                    size_bytes: None,
                    seed_from: None,
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
                    addressing: None,
                    firewall_policy_json: None,
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
        matches!(result, Err(RuntimeFailure::Internal)),
        "failed hypervisor delete must surface the effect failure (mapped from ChvError::Internal via the fail_delete knob), got {result:?}"
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
