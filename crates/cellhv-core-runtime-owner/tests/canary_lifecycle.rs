//! Executor↔effector canary lifecycle test (M2.2a).
//!
//! Composition-level: drives the REAL `JournalExecutor` (through the
//! `CoreRuntimeOwner` harness) with the production `CloudHypervisorCoreRuntime`
//! effector backed by `MockHostResourceController` + `MockCloudHypervisorAdapter`.
//! This pins the executor↔runtime wiring end-to-end with real per-operation
//! side effects, without any KVM/stord/nwd daemon.

use cellhv_core_operations::{MutationCommand, SubmitMutation};
use cellhv_core_runtime_owner::{CoreRuntimeOwner, JournalPollerConfig};
use cellhv_core_startup::{StartupPaths, StartupTransaction};
use cellhv_core_types::{
    BootSpec, ComputeSpec, IdempotencyKey, ObservedPowerState, OperationId,
    OperationRequestMetadata, OperationStatus, RequestedPowerState, ResourceVersion,
    StorageAttachmentRef, VmDefinition, VmId,
};
use chv_agent_runtime_ch::core_runtime::CloudHypervisorCoreRuntime;
use chv_agent_runtime_ch::{MockCloudHypervisorAdapter, MockHostResourceController};
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn paths(directory: &tempfile::TempDir) -> StartupPaths {
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    StartupPaths {
        node_cache: directory.path().join("node-cache.json"),
        core_database: directory.path().join("core.db"),
        node_cache_archive: directory.path().join("node-cache.archive"),
    }
}

fn version(value: u64) -> ResourceVersion {
    ResourceVersion::new(value).unwrap()
}

fn base_submission(vm_id: &str, op_id: &str, command: MutationCommand) -> SubmitMutation {
    SubmitMutation {
        operation_id: OperationId::new(op_id).unwrap(),
        idempotency_scope: format!("canary/{vm_id}"),
        idempotency_key: IdempotencyKey::new(op_id).unwrap(),
        expected_vm_version: version(1),
        metadata: OperationRequestMetadata {
            requested_by: "canary-requester".to_owned(),
            external_operation_id: "external-canary".to_owned(),
            request_unix_ms: 1_700_000_000_000,
            legacy_generation: None,
        },
        command,
    }
}

fn create_submission(vm_id: &str, op_id: &str) -> SubmitMutation {
    let definition = VmDefinition {
        id: VmId::new(vm_id).unwrap(),
        name: format!("{vm_id}-guest"),
        boot: BootSpec::new("/kernel").unwrap(),
        compute: ComputeSpec::new(2, 1024).unwrap(),
        storage: vec![StorageAttachmentRef {
            attachment_id: "vol-0".to_string(),
            storage_ref: "vol-0".to_string(),
            read_only: false,
        }],
        networks: vec![],
        requested_power_state: RequestedPowerState::Stopped,
        observed_power_state: ObservedPowerState::Unknown,
        resource_version: version(1),
    };
    base_submission(vm_id, op_id, MutationCommand::CreateVm { definition })
}

fn delete_submission(vm_id: &str, op_id: &str) -> SubmitMutation {
    base_submission(
        vm_id,
        op_id,
        MutationCommand::DeleteVm {
            vm_id: VmId::new(vm_id).unwrap(),
        },
    )
}

async fn poll_terminal(owner: &CoreRuntimeOwner, op_id: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let entries = owner.authority().operations().await.unwrap();
        if entries.iter().any(|entry| {
            entry.operation.id.as_str() == op_id
                && entry.operation.status == OperationStatus::Succeeded
        }) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "operation {op_id} never reached Succeeded in the composition"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// True when `needle` appears in `haystack` in order.
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn executor_canary_runs_full_create_and_delete_side_effects() {
    let directory = tempfile::tempdir().unwrap();
    let paths = paths(&directory);
    let socket = directory.path().join("core.sock");
    let runtime_dir = directory.path().join("runtime");

    let adapter = Arc::new(MockCloudHypervisorAdapter::default());
    let controller = Arc::new(MockHostResourceController::new());
    let runtime = Arc::new(CloudHypervisorCoreRuntime::new(
        adapter.clone(),
        controller.clone(),
        runtime_dir.clone(),
    ));

    let owner = CoreRuntimeOwner::start(
        runtime,
        StartupTransaction::begin(&paths)
            .unwrap()
            .activate(Some("canary-host".to_owned()), None)
            .unwrap(),
        &socket,
        16,
        Duration::from_secs(1),
        JournalPollerConfig {
            scan_interval: Duration::from_millis(40),
            scan_timeout: Duration::from_secs(1),
            drain_budget: Duration::from_secs(1),
        },
    )
    .await
    .unwrap();

    // CreateVm: the composition-internal poller claims the accepted operation,
    // executes the full side-effect effector, and finishes it Succeeded.
    owner
        .authority()
        .submit(create_submission("vm-canary", "op-create"))
        .await
        .unwrap();
    poll_terminal(&owner, "op-create").await;

    let log = controller.calls.lock().unwrap().clone();
    assert!(
        is_subsequence(&log, &["open:vol-0", "attach:vol-0"]),
        "create must open+attach the volume: {log:?}"
    );
    assert!(
        adapter.vms.lock().unwrap().contains_key("vm-canary"),
        "adapter must contain the created VM"
    );
    // The real executor must have presented the canonical envelope to the
    // effector (otherwise the create would have failed as InvalidRequest).
    let vm_dir = runtime_dir.join("vms").join("vm-canary");
    assert!(vm_dir.exists(), "VM runtime dir must exist: {vm_dir:?}");

    // DeleteVm: adapter delete + drain of the recorded side effects.
    owner
        .authority()
        .submit(delete_submission("vm-canary", "op-del"))
        .await
        .unwrap();
    poll_terminal(&owner, "op-del").await;

    assert!(
        !adapter.vms.lock().unwrap().contains_key("vm-canary"),
        "adapter VM must be gone after delete"
    );
    let log = controller.calls.lock().unwrap().clone();
    assert!(
        is_subsequence(&log, &["detach:vol-0", "close:vol-0"]),
        "delete must drain detach+close of the volume: {log:?}"
    );

    owner.shutdown().await.unwrap();
}
