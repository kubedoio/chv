use super::*;
use cellhv_core_operations::{AuthorityActor, MutationCommand, OperationService, SubmitMutation};
use cellhv_core_store::CoreStore;
use cellhv_core_types::{
    BootSpec, ComputeSpec, IdempotencyKey, ObservedPowerState, RequestedPowerState,
    ResourceVersion, VmDefinition,
};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture {
    _dir: tempfile::TempDir,
    authority: cellhv_core_operations::AuthorityHandle,
    execution: ExecutionHandle,
    join: cellhv_core_operations::AuthorityActorJoin,
}
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let store = CoreStore::create_new(&dir.path().join("core.db")).unwrap();
    let (authority, execution, join) =
        AuthorityActor::spawn_with_execution(OperationService::new(store), 32).unwrap();
    Fixture {
        _dir: dir,
        authority,
        execution,
        join,
    }
}
fn submit(vm: &str, op: &str) -> SubmitMutation {
    SubmitMutation {
        operation_id: OperationId::new(op).unwrap(),
        idempotency_scope: "test".into(),
        idempotency_key: IdempotencyKey::new(op).unwrap(),
        expected_vm_version: ResourceVersion::new(1).unwrap(),
        command: MutationCommand::CreateVm {
            definition: VmDefinition {
                id: VmId::new(vm).unwrap(),
                name: vm.into(),
                boot: BootSpec::new("/kernel").unwrap(),
                compute: ComputeSpec::new(1, 128).unwrap(),
                storage: vec![],
                networks: vec![],
                requested_power_state: RequestedPowerState::Stopped,
                observed_power_state: ObservedPowerState::Unknown,
                resource_version: ResourceVersion::new(1).unwrap(),
            },
        },
    }
}
async fn stop(f: Fixture) {
    f.authority.shutdown().await.unwrap();
    f.join.join().await.unwrap();
}
struct Counting {
    calls: AtomicUsize,
    result: Option<serde_json::Value>,
}
#[async_trait]
impl CoreVmRuntime for Counting {
    async fn execute(
        &self,
        _: OperationJournalEntry,
    ) -> std::result::Result<Option<serde_json::Value>, RuntimeFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.result.clone())
    }
}
fn fixed(value: &str) -> TokenFactory {
    let token = AttemptToken::new(value).unwrap();
    Arc::new(move || token.clone())
}

#[tokio::test]
async fn replay_quarantines_later_same_vm_work() {
    let f = fixture();
    f.authority.submit(submit("a", "one")).await.unwrap();
    f.authority
        .submit(SubmitMutation {
            operation_id: OperationId::new("two").unwrap(),
            idempotency_scope: "test".into(),
            idempotency_key: IdempotencyKey::new("two").unwrap(),
            expected_vm_version: ResourceVersion::new(1).unwrap(),
            command: MutationCommand::StartVm {
                vm_id: VmId::new("a").unwrap(),
            },
        })
        .await
        .unwrap();
    f.execution
        .claim_attempt(
            OperationId::new("one").unwrap(),
            AttemptToken::new("token").unwrap(),
        )
        .await
        .unwrap();
    let runtime = Arc::new(Counting {
        calls: AtomicUsize::new(0),
        result: None,
    });
    let executor = JournalExecutor::start_with_token_factory(
        f.execution.clone(),
        runtime.clone(),
        1,
        2,
        fixed("token"),
    )
    .unwrap();
    let scan = executor.scan_ready().await.unwrap();
    assert_eq!(
        scan.inspect_required,
        vec![OperationId::new("one").unwrap()]
    );
    assert_eq!(scan.quarantined, vec![OperationId::new("two").unwrap()]);
    executor.shutdown().await.unwrap();
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);
    stop(f).await;
}

#[tokio::test]
async fn claim_ambiguity_quarantines_queued_successor() {
    let f = fixture();
    f.authority.submit(submit("a", "one")).await.unwrap();
    f.authority
        .submit(SubmitMutation {
            operation_id: OperationId::new("two").unwrap(),
            idempotency_scope: "test".into(),
            idempotency_key: IdempotencyKey::new("two").unwrap(),
            expected_vm_version: ResourceVersion::new(1).unwrap(),
            command: MutationCommand::StartVm {
                vm_id: VmId::new("a").unwrap(),
            },
        })
        .await
        .unwrap();
    f.execution
        .claim_attempt(
            OperationId::new("one").unwrap(),
            AttemptToken::new("other").unwrap(),
        )
        .await
        .unwrap();
    let runtime = Arc::new(Counting {
        calls: AtomicUsize::new(0),
        result: None,
    });
    let executor = JournalExecutor::start_with_token_factory(
        f.execution.clone(),
        runtime.clone(),
        1,
        2,
        fixed("token"),
    )
    .unwrap();
    let scan = executor.scan_ready().await.unwrap();
    assert_eq!(scan.inspect_required.len(), 1);
    assert_eq!(scan.quarantined.len(), 1);
    executor.shutdown().await.unwrap();
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);
    stop(f).await;
}

#[tokio::test]
async fn invalid_result_is_not_finished_and_quarantines_vm() {
    let f = fixture();
    f.authority.submit(submit("a", "one")).await.unwrap();
    let runtime = Arc::new(Counting {
        calls: AtomicUsize::new(0),
        result: Some(serde_json::Value::String("not-object".into())),
    });
    let executor = JournalExecutor::start(f.execution.clone(), runtime, 1, 1).unwrap();
    executor.scan_ready().await.unwrap();
    let report = executor.shutdown().await.unwrap();
    assert_eq!(report.failures[0].code, ExecutionFailureCode::ResultInvalid);
    assert_eq!(
        f.execution.restart_operations().await.unwrap()[0].disposition,
        RestartDisposition::InspectRequired
    );
    stop(f).await;
}

#[test]
fn results_and_runtime_errors_are_publicly_bounded() {
    assert!(valid_result(&Some(serde_json::json!({"ok": true}))));
    assert!(!valid_result(&Some(serde_json::json!([1, 2]))));
    assert_eq!(
        RuntimeFailure::Internal.into_json(),
        serde_json::json!({"code":"INTERNAL"})
    );
}

struct Blocking {
    entered: tokio::sync::Notify,
    release: tokio::sync::Semaphore,
    calls: AtomicUsize,
}
#[async_trait]
impl CoreVmRuntime for Blocking {
    async fn execute(
        &self,
        _: OperationJournalEntry,
    ) -> std::result::Result<Option<serde_json::Value>, RuntimeFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        self.release.acquire().await.unwrap().forget();
        Ok(Some(serde_json::json!({"ok":true})))
    }
}

#[tokio::test]
async fn success_claim_is_durable_before_effect_and_terminal_afterward() {
    let f = fixture();
    f.authority.submit(submit("a", "one")).await.unwrap();
    struct Inspect {
        authority: cellhv_core_operations::AuthorityHandle,
    }
    #[async_trait]
    impl CoreVmRuntime for Inspect {
        async fn execute(
            &self,
            op: OperationJournalEntry,
        ) -> std::result::Result<Option<serde_json::Value>, RuntimeFailure> {
            assert_eq!(
                self.authority
                    .operation(op.operation.id)
                    .await
                    .unwrap()
                    .operation
                    .status,
                cellhv_core_types::OperationStatus::Running
            );
            Ok(Some(serde_json::json!({"ok":true})))
        }
    }
    let executor = JournalExecutor::start(
        f.execution.clone(),
        Arc::new(Inspect {
            authority: f.authority.clone(),
        }),
        1,
        1,
    )
    .unwrap();
    executor.scan_ready().await.unwrap();
    assert_eq!(executor.shutdown().await.unwrap().completed, 1);
    assert_eq!(
        f.authority
            .operation(OperationId::new("one").unwrap())
            .await
            .unwrap()
            .operation
            .status,
        cellhv_core_types::OperationStatus::Succeeded
    );
    stop(f).await;
}

#[tokio::test]
async fn capacity_is_exact_total_admission_not_channel_plus_pending() {
    let f = fixture();
    for n in 0..3 {
        f.authority
            .submit(submit(&format!("v{n}"), &format!("o{n}")))
            .await
            .unwrap();
    }
    let runtime = Arc::new(Blocking {
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Semaphore::new(0),
        calls: AtomicUsize::new(0),
    });
    let executor = JournalExecutor::start(f.execution.clone(), runtime.clone(), 1, 2).unwrap();
    let scan = executor.scan_ready().await.unwrap();
    assert_eq!(scan.scheduled.len(), 2);
    assert!(scan.capacity_reached);
    runtime.entered.notified().await;
    runtime.release.add_permits(2);
    assert_eq!(executor.shutdown().await.unwrap().completed, 2);
    assert_eq!(
        f.authority
            .operation(OperationId::new("o2").unwrap())
            .await
            .unwrap()
            .operation
            .status,
        cellhv_core_types::OperationStatus::Accepted
    );
    stop(f).await;
}

#[tokio::test]
async fn abort_leaves_claimed_inspect_required_and_queued_accepted() {
    let f = fixture();
    f.authority.submit(submit("a", "one")).await.unwrap();
    f.authority.submit(submit("b", "two")).await.unwrap();
    let runtime = Arc::new(Blocking {
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Semaphore::new(0),
        calls: AtomicUsize::new(0),
    });
    let executor = JournalExecutor::start(f.execution.clone(), runtime.clone(), 1, 2).unwrap();
    executor.scan_ready().await.unwrap();
    runtime.entered.notified().await;
    executor.abort().await.unwrap();
    let restart = f.execution.restart_operations().await.unwrap();
    assert_eq!(
        restart
            .iter()
            .filter(|x| x.disposition == RestartDisposition::InspectRequired)
            .count(),
        1
    );
    assert_eq!(
        restart
            .iter()
            .filter(|x| x.disposition == RestartDisposition::Ready)
            .count(),
        1
    );
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 1);
    stop(f).await;
}

#[tokio::test]
async fn finish_ambiguity_does_not_launch_same_vm_successor() {
    let f = fixture();
    f.authority.submit(submit("a", "one")).await.unwrap();
    f.authority
        .submit(SubmitMutation {
            operation_id: OperationId::new("two").unwrap(),
            idempotency_scope: "test".into(),
            idempotency_key: IdempotencyKey::new("two").unwrap(),
            expected_vm_version: ResourceVersion::new(1).unwrap(),
            command: MutationCommand::StartVm {
                vm_id: VmId::new("a").unwrap(),
            },
        })
        .await
        .unwrap();
    let runtime = Arc::new(Blocking {
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Semaphore::new(0),
        calls: AtomicUsize::new(0),
    });
    let executor = JournalExecutor::start(f.execution.clone(), runtime.clone(), 1, 2).unwrap();
    executor.scan_ready().await.unwrap();
    runtime.entered.notified().await;
    f.authority.shutdown().await.unwrap();
    runtime.release.add_permits(1);
    let report = executor.shutdown().await.unwrap();
    assert_eq!(
        report.failures[0].code,
        ExecutionFailureCode::FinishAmbiguous
    );
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 1);
    f.join.join().await.unwrap();
}

#[tokio::test]
async fn durable_scan_order_is_stable_and_duplicate_scans_do_not_reorder() {
    let f = fixture();
    f.authority.submit(submit("a", "z-first")).await.unwrap();
    f.authority.submit(submit("b", "a-second")).await.unwrap();
    let expected: Vec<_> = f
        .execution
        .restart_operations()
        .await
        .unwrap()
        .into_iter()
        .map(|x| x.entry.operation.id)
        .collect();
    let runtime = Arc::new(Counting {
        calls: AtomicUsize::new(0),
        result: Some(serde_json::json!({"ok":true})),
    });
    let executor = JournalExecutor::start(f.execution.clone(), runtime, 1, 2).unwrap();
    let first = executor.scan_ready().await.unwrap();
    let second = executor.scan_ready().await.unwrap();
    assert_eq!(first.scheduled, expected);
    assert!(second.scheduled.is_empty());
    executor.shutdown().await.unwrap();
    stop(f).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn panic_aborts_active_peer_and_never_launches_pending_work() {
    struct Mixed {
        barrier: tokio::sync::Barrier,
        hold: tokio::sync::Notify,
        calls: AtomicUsize,
    }
    #[async_trait]
    impl CoreVmRuntime for Mixed {
        async fn execute(
            &self,
            op: OperationJournalEntry,
        ) -> std::result::Result<Option<serde_json::Value>, RuntimeFailure> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.barrier.wait().await;
            if op.operation.id.as_str() == "one" {
                panic!("private panic payload");
            }
            self.hold.notified().await;
            Ok(None)
        }
    }
    let f = fixture();
    f.authority.submit(submit("a", "one")).await.unwrap();
    f.authority.submit(submit("b", "two")).await.unwrap();
    f.authority.submit(submit("c", "three")).await.unwrap();
    let runtime = Arc::new(Mixed {
        barrier: tokio::sync::Barrier::new(2),
        hold: tokio::sync::Notify::new(),
        calls: AtomicUsize::new(0),
    });
    let executor = JournalExecutor::start(f.execution.clone(), runtime.clone(), 2, 3).unwrap();
    executor.scan_ready().await.unwrap();
    let report = tokio::time::timeout(std::time::Duration::from_secs(1), executor.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert!(report
        .failures
        .iter()
        .any(|x| x.code == ExecutionFailureCode::TaskPanicked));
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 2);
    let restart = f.execution.restart_operations().await.unwrap();
    assert_eq!(
        restart
            .iter()
            .filter(|x| x.disposition == RestartDisposition::InspectRequired)
            .count(),
        2
    );
    assert_eq!(
        restart
            .iter()
            .filter(|x| x.disposition == RestartDisposition::Ready)
            .count(),
        1
    );
    stop(f).await;
}

#[tokio::test]
async fn complete_snapshot_quarantines_before_capacity_admission() {
    let f = fixture();
    f.authority.submit(submit("a", "ready-a")).await.unwrap();
    f.authority.submit(submit("b", "ready-b")).await.unwrap();
    f.authority
        .submit(SubmitMutation {
            operation_id: OperationId::new("running-a").unwrap(),
            idempotency_scope: "test".into(),
            idempotency_key: IdempotencyKey::new("running-a").unwrap(),
            expected_vm_version: ResourceVersion::new(1).unwrap(),
            command: MutationCommand::StartVm {
                vm_id: VmId::new("a").unwrap(),
            },
        })
        .await
        .unwrap();
    f.execution
        .claim_attempt(
            OperationId::new("running-a").unwrap(),
            AttemptToken::new("prior").unwrap(),
        )
        .await
        .unwrap();
    let runtime = Arc::new(Counting {
        calls: AtomicUsize::new(0),
        result: Some(serde_json::json!({"ok":true})),
    });
    let executor = JournalExecutor::start(f.execution.clone(), runtime.clone(), 1, 1).unwrap();
    let scan = executor.scan_ready().await.unwrap();
    assert_eq!(scan.scheduled, vec![OperationId::new("ready-b").unwrap()]);
    assert_eq!(scan.quarantined, vec![OperationId::new("ready-a").unwrap()]);
    executor.shutdown().await.unwrap();
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 1);
    stop(f).await;
}

#[tokio::test]
async fn concurrent_scans_admit_one_copy_without_false_quarantine() {
    let f = fixture();
    f.authority.submit(submit("a", "one")).await.unwrap();
    let runtime = Arc::new(Counting {
        calls: AtomicUsize::new(0),
        result: Some(serde_json::json!({"ok":true})),
    });
    let executor =
        Arc::new(JournalExecutor::start(f.execution.clone(), runtime.clone(), 1, 1).unwrap());
    let (a, b) = tokio::join!(executor.scan_ready(), executor.scan_ready());
    assert_eq!(a.unwrap().scheduled.len() + b.unwrap().scheduled.len(), 1);
    Arc::try_unwrap(executor)
        .ok()
        .unwrap()
        .shutdown()
        .await
        .unwrap();
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 1);
    stop(f).await;
}

#[tokio::test]
async fn unsupported_runtime_outcome_is_persisted_as_unsupported() {
    struct Unsupported;
    #[async_trait]
    impl CoreVmRuntime for Unsupported {
        async fn execute(
            &self,
            _: OperationJournalEntry,
        ) -> std::result::Result<Option<serde_json::Value>, RuntimeFailure> {
            Err(RuntimeFailure::Unsupported)
        }
    }
    let f = fixture();
    f.authority.submit(submit("a", "one")).await.unwrap();
    let executor =
        JournalExecutor::start(f.execution.clone(), Arc::new(Unsupported), 1, 1).unwrap();
    executor.scan_ready().await.unwrap();
    executor.shutdown().await.unwrap();
    assert_eq!(
        f.authority
            .operation(OperationId::new("one").unwrap())
            .await
            .unwrap()
            .operation
            .status,
        cellhv_core_types::OperationStatus::Unsupported
    );
    stop(f).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn same_vm_is_ordered_while_different_vms_overlap() {
    struct Concurrent {
        active: Mutex<std::collections::HashMap<VmId, usize>>,
        entered: Mutex<Vec<String>>,
        count: AtomicUsize,
        two: tokio::sync::Notify,
        three: tokio::sync::Notify,
        release: tokio::sync::Semaphore,
        active_total: AtomicUsize,
        max_total: AtomicUsize,
        max_same: AtomicUsize,
    }
    #[async_trait]
    impl CoreVmRuntime for Concurrent {
        async fn execute(
            &self,
            op: OperationJournalEntry,
        ) -> std::result::Result<Option<serde_json::Value>, RuntimeFailure> {
            let total = self.active_total.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_total.fetch_max(total, Ordering::SeqCst);
            {
                let mut active = self.active.lock().unwrap();
                let current = active.entry(op.operation.vm_id.clone()).or_default();
                *current += 1;
                self.max_same.fetch_max(*current, Ordering::SeqCst);
            }
            self.entered
                .lock()
                .unwrap()
                .push(op.operation.id.to_string());
            match self.count.fetch_add(1, Ordering::SeqCst) + 1 {
                2 => self.two.notify_one(),
                3 => self.three.notify_one(),
                _ => {}
            }
            self.release.acquire().await.unwrap().forget();
            {
                let mut active = self.active.lock().unwrap();
                *active.get_mut(&op.operation.vm_id).unwrap() -= 1;
            }
            self.active_total.fetch_sub(1, Ordering::SeqCst);
            Ok(Some(serde_json::json!({"ok":true})))
        }
    }
    let f = fixture();
    f.authority.submit(submit("a", "a1")).await.unwrap();
    f.authority
        .submit(SubmitMutation {
            operation_id: OperationId::new("a2").unwrap(),
            idempotency_scope: "test".into(),
            idempotency_key: IdempotencyKey::new("a2").unwrap(),
            expected_vm_version: ResourceVersion::new(1).unwrap(),
            command: MutationCommand::StartVm {
                vm_id: VmId::new("a").unwrap(),
            },
        })
        .await
        .unwrap();
    f.authority.submit(submit("b", "b1")).await.unwrap();
    let runtime = Arc::new(Concurrent {
        active: Mutex::new(std::collections::HashMap::new()),
        entered: Mutex::new(Vec::new()),
        count: AtomicUsize::new(0),
        two: tokio::sync::Notify::new(),
        three: tokio::sync::Notify::new(),
        release: tokio::sync::Semaphore::new(0),
        active_total: AtomicUsize::new(0),
        max_total: AtomicUsize::new(0),
        max_same: AtomicUsize::new(0),
    });
    let executor = JournalExecutor::start(f.execution.clone(), runtime.clone(), 2, 3).unwrap();
    executor.scan_ready().await.unwrap();
    runtime.two.notified().await;
    let first = runtime.entered.lock().unwrap().clone();
    assert_eq!(first.len(), 2);
    assert!(first.contains(&"a1".into()));
    assert!(first.contains(&"b1".into()));
    assert!(!first.contains(&"a2".into()));
    runtime.release.add_permits(2);
    runtime.three.notified().await;
    assert_eq!(
        runtime
            .entered
            .lock()
            .unwrap()
            .iter()
            .filter(|id| id.starts_with('a'))
            .cloned()
            .collect::<Vec<_>>(),
        vec!["a1", "a2"]
    );
    runtime.release.add_permits(1);
    executor.shutdown().await.unwrap();
    assert_eq!(runtime.max_same.load(Ordering::SeqCst), 1);
    assert_eq!(runtime.max_total.load(Ordering::SeqCst), 2);
    stop(f).await;
}
