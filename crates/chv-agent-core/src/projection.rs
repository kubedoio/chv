//! NodeCache compatibility projection after Core execution (M2.2b).
//!
//! `ProjectingCoreRuntime` wraps the single Core effector (M2.2a) and, after a
//! Succeeded (`Ok(None)`) outcome, projects the authoritative desired state
//! into NodeCache so the legacy compatibility surface stays readable **without**
//! the Reconciler acting as a second authority (in core-managed mode the
//! Reconciler is constructed observe-only — `Reconciler::new_observe_only`,
//! M2.3 — so it cannot mutate NodeCache or the provider at all).
//!
//! Projection is strictly downstream of Core execution and must never change an
//! executor outcome:
//! - a request that is not the canonical envelope is warned and skipped;
//! - a runtime error or non-`None` result is returned unmodified;
//! - a projection or cache-save failure is warned and skipped;
//! - the returned `Result` is always exactly what `inner.execute` returned.

use crate::cache::NodeCache;
use cellhv_core_executor::{CoreVmRuntime, RuntimeFailure};
use cellhv_core_operations::{CanonicalRequest, MutationCommand, OperationJournalEntry};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::warn;

/// Wraps a single Core effector and projects Succeeded outcomes into NodeCache.
///
/// The inner runtime performs every real side effect (M2.2a); this wrapper only
/// adds the compatibility projection + cache persistence that follow a
/// Succeeded terminal outcome. Multiple [`ProjectingCoreRuntime`] instances
/// sharing one `cache`/`cache_path` serialize projection+save through the cache
/// mutex.
pub struct ProjectingCoreRuntime {
    inner: Arc<dyn CoreVmRuntime>,
    cache: Arc<Mutex<NodeCache>>,
    cache_path: PathBuf,
}

impl ProjectingCoreRuntime {
    pub fn new(
        inner: Arc<dyn CoreVmRuntime>,
        cache: Arc<Mutex<NodeCache>>,
        cache_path: PathBuf,
    ) -> Self {
        Self {
            inner,
            cache,
            cache_path,
        }
    }

    /// Parse the operation request into the canonical envelope, or `None` (with
    /// a warning) when it is not one. A malformed request never fails an
    /// already-executed op — it only means "no projection".
    fn surface_request(entry: &OperationJournalEntry) -> Option<CanonicalRequest> {
        match CanonicalRequest::try_from_value(&entry.request) {
            Ok(Some(request)) => Some(request),
            Ok(None) | Err(_) => {
                warn!(
                    operation_id = %entry.operation.id,
                    kind = ?entry.operation.kind,
                    "projection: operation request is not a canonical envelope; skipping compatibility projection"
                );
                // Defensive only: for a genuinely executed operation this branch
                // is unreachable — the inner runtime re-parses the same envelope
                // and fails closed as `InvalidRequest` before any side effect.
                // It guards against a future inner that does not re-parse.
                None
            }
        }
    }

    /// Project one Succeeded terminal outcome into the VM axis of NodeCache.
    ///
    /// This is best-effort by construction: every step is infallible for the
    /// executor (no error is returned to the scheduler), so the only failure
    /// surface left is the cache save, which the caller handles with warn+skip.
    ///
    /// Returns `true` when a cache mutation was applied (the caller then
    /// persists); `false` when nothing changed — a no-op arm (Update/Attach/
    /// Detach), a power-op on a VM with no projected fragment, or an envelope
    /// whose VM id disagrees with the durable operation id — in which case no
    /// save happens and the returned `Result` is still unchanged.
    async fn project(&self, entry: &OperationJournalEntry, request: &CanonicalRequest) -> bool {
        // The durable journal's operation VM id is authoritative for which VM the
        // effector acted on (the executor keys every side effect on it). The
        // envelope command must agree; if it does not, the request row is
        // internally inconsistent and projecting to either id could hit the
        // wrong VM — so warn and skip rather than write a wrong-axis entry.
        let envelope_id = request.command.vm_id();
        let authoritative_id = &entry.operation.vm_id;
        if envelope_id != authoritative_id {
            warn!(
                operation_id = %entry.operation.id,
                envelope_vm_id = ?envelope_id,
                operation_vm_id = ?authoritative_id,
                "projection: canonical envelope VM id disagrees with the durable operation; skipping projection"
            );
            return false;
        }
        let vm_id = authoritative_id.as_str();
        let (updated_at, updated_by) = attribution(entry);
        match &request.command {
            MutationCommand::CreateVm { definition } => {
                let mut cache = self.cache.lock().await;
                cache.project_vm(definition, updated_at, updated_by);
                true
            }
            MutationCommand::DeleteVm { .. } => {
                let mut cache = self.cache.lock().await;
                cache.remove_vm_state(vm_id);
                true
            }
            MutationCommand::StartVm { .. } | MutationCommand::RebootVm { .. } => {
                let mut cache = self.cache.lock().await;
                if !cache.get_fragment("vm", vm_id).is_some() {
                    warn!(
                        vm_id,
                        kind = ?request.command.kind(),
                        "projection: no VM fragment to update after start/reboot — was the startup rebuild skipped?"
                    );
                    false
                } else {
                    cache.update_vm_desired_state(vm_id, "Running");
                    true
                }
            }
            MutationCommand::StopVm { .. } => {
                let mut cache = self.cache.lock().await;
                if !cache.get_fragment("vm", vm_id).is_some() {
                    warn!(
                        vm_id,
                        kind = ?request.command.kind(),
                        "projection: no VM fragment to update after stop — was the startup rebuild skipped?"
                    );
                    false
                } else {
                    cache.update_vm_desired_state(vm_id, "Stopped");
                    true
                }
            }
            // Out-of-RC-lifecycle in M2.2a: these fail closed as Unsupported and
            // never reach a Succeeded outcome; if one somehow does, the
            // projection is a no-op (nothing to converge) rather than a failure.
            MutationCommand::UpdateVm { .. }
            | MutationCommand::AttachVolume { .. }
            | MutationCommand::DetachVolume { .. }
            | MutationCommand::AttachNetwork { .. }
            | MutationCommand::DetachNetwork { .. } => false,
        }
    }
}

/// Derived `updated_at`/`updated_by` attribution for a projection, taken from
/// the operation's durable request metadata when present.
fn attribution(entry: &OperationJournalEntry) -> (String, String) {
    match entry.request_metadata.as_ref() {
        Some(metadata) => {
            let requested_by = if metadata.requested_by.trim().is_empty() {
                "core".to_string()
            } else {
                metadata.requested_by.clone()
            };
            (metadata.request_unix_ms.to_string(), requested_by)
        }
        None => (chv_common::now_unix_ms().to_string(), "core".to_string()),
    }
}

#[async_trait::async_trait]
impl CoreVmRuntime for ProjectingCoreRuntime {
    async fn execute(
        &self,
        entry: OperationJournalEntry,
    ) -> std::result::Result<Option<serde_json::Value>, RuntimeFailure> {
        let request = Self::surface_request(&entry);
        let outcome = self.inner.execute(entry.clone()).await?;
        // Projection runs ONLY on the executor's Succeeded path (`Ok(None)`).
        // An `Ok(Some(_))` result has no desired-state projection here, and an
        // `Err(_)` returns early above; neither is ever rewritten.
        if outcome.is_none() {
            if let Some(request) = request {
                // Persist only when the projection actually mutated the cache
                // (no-op arms and malformed/skipped projections leave no trace;
                // saving them would just write a redundant snapshot). A save
                // failure must not change the already-validated outcome.
                if self.project(&entry, &request).await {
                    let cache = self.cache.lock().await;
                    if let Err(error) = cache.save(&self.cache_path).await {
                        warn!(
                            operation_id = %entry.operation.id,
                            error = %error,
                            "NodeCache projection save failed; cache stays stale in-memory"
                        );
                    }
                }
            }
        }
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cellhv_core_types::{
        BootSpec, ComputeSpec, IdempotencyKey, ObservedPowerState, Operation, OperationId,
        OperationKind, OperationRequestMetadata, OperationStatus, RequestedPowerState,
        ResourceVersion, VmDefinition, VmId,
    };
    use std::collections::VecDeque;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Programmable test double for the inner runtime.
    struct StubRuntime {
        outcomes: std::sync::Mutex<
            VecDeque<std::result::Result<Option<serde_json::Value>, RuntimeFailure>>,
        >,
        calls: AtomicUsize,
    }

    impl StubRuntime {
        fn new(
            outcomes: Vec<std::result::Result<Option<serde_json::Value>, RuntimeFailure>>,
        ) -> Self {
            Self {
                outcomes: std::sync::Mutex::new(outcomes.into()),
                calls: AtomicUsize::new(0),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl CoreVmRuntime for StubRuntime {
        async fn execute(
            &self,
            _operation: OperationJournalEntry,
        ) -> std::result::Result<Option<serde_json::Value>, RuntimeFailure> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.outcomes
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Ok(None))
        }
    }

    fn definition(vm_id: &str) -> VmDefinition {
        VmDefinition {
            id: VmId::new(vm_id).unwrap(),
            name: format!("{vm_id}-guest"),
            boot: BootSpec::new("/kernel").unwrap(),
            compute: ComputeSpec::new(2, 1024).unwrap(),
            storage: vec![],
            networks: vec![],
            requested_power_state: RequestedPowerState::Stopped,
            observed_power_state: ObservedPowerState::Unknown,
            resource_version: ResourceVersion::new(1).unwrap(),
            cloud_init_userdata: None,
            hypervisor_tuning: None,
        }
    }

    fn entry(
        kind: OperationKind,
        vm_id: &str,
        request: serde_json::Value,
    ) -> OperationJournalEntry {
        OperationJournalEntry {
            operation: Operation {
                id: OperationId::new("op-1").unwrap(),
                kind,
                vm_id: VmId::new(vm_id).unwrap(),
                status: OperationStatus::Succeeded,
                request_fingerprint: "fingerprint".to_string(),
                attempt_count: 1,
                max_attempts: 3,
            },
            request,
            result: None,
            error: None,
            request_metadata: Some(OperationRequestMetadata {
                requested_by: "requester-1".to_string(),
                external_operation_id: "external-1".to_string(),
                request_unix_ms: 1_700_000_000_000,
                legacy_generation: None,
            }),
            recovery_assessment: None,
        }
    }

    fn envelope(command: MutationCommand) -> serde_json::Value {
        serde_json::json!({
            "command": command,
            "expected_vm_version": 1,
        })
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        cache: Arc<tokio::sync::Mutex<NodeCache>>,
        cache_path: std::path::PathBuf,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        // Normalize the temp dir to 0700 regardless of the host umask: the
        // authority-lock and Core fresh-parent checks reject group/other-
        // writable parents, and `tempfile::tempdir()` honors the umask.
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let cache_path = dir.path().join("cache.json");
        Fixture {
            _dir: dir,
            cache: Arc::new(tokio::sync::Mutex::new(NodeCache::new("node-1"))),
            cache_path,
        }
    }

    #[tokio::test]
    async fn projection_succeeded_projects_create_into_cache() {
        let f = fixture();
        let stub = Arc::new(StubRuntime::new(vec![Ok(None)]));
        let wrapper = Arc::new(ProjectingCoreRuntime::new(
            stub.clone(),
            f.cache.clone(),
            f.cache_path.clone(),
        ));
        let outcome = wrapper
            .execute(entry(
                OperationKind::CreateVm,
                "vm-a",
                envelope(MutationCommand::CreateVm {
                    definition: definition("vm-a"),
                }),
            ))
            .await
            .unwrap();
        assert!(outcome.is_none());
        assert_eq!(stub.calls(), 1);
        let cache = f.cache.lock().await;
        assert!(cache.get_fragment("vm", "vm-a").is_some());
        assert_eq!(cache.get_generation("vm", "vm-a"), Some(&"1".to_string()));
        assert!(cache.vm_attachment_state("vm-a").is_some());
        let persisted = NodeCache::load(&f.cache_path).await.unwrap();
        assert!(persisted.get_fragment("vm", "vm-a").is_some());
        drop(cache);
    }

    #[tokio::test]
    async fn projection_runtime_errors_and_non_none_results_are_returned_unchanged() {
        // A runtime failure must propagate with no projection.
        let f = fixture();
        let stub = Arc::new(StubRuntime::new(vec![Err(
            RuntimeFailure::RuntimeUnavailable,
        )]));
        let wrapper =
            ProjectingCoreRuntime::new(stub.clone(), f.cache.clone(), f.cache_path.clone());
        let outcome = wrapper
            .execute(entry(
                OperationKind::CreateVm,
                "vm-a",
                envelope(MutationCommand::CreateVm {
                    definition: definition("vm-a"),
                }),
            ))
            .await;
        assert!(matches!(outcome, Err(RuntimeFailure::RuntimeUnavailable)));
        assert!(!f.cache.lock().await.get_fragment("vm", "vm-a").is_some());

        // An Ok(Some(_)) result (a non-projection terminal result) must also
        // pass through untouched — no projection, no save.
        let result = serde_json::json!({"observed": "state"});
        let stub = Arc::new(StubRuntime::new(vec![Ok(Some(result.clone()))]));
        let wrapper = ProjectingCoreRuntime::new(stub, f.cache.clone(), f.cache_path.clone());
        let outcome = wrapper
            .execute(entry(
                OperationKind::CreateVm,
                "vm-a",
                envelope(MutationCommand::CreateVm {
                    definition: definition("vm-a"),
                }),
            ))
            .await
            .unwrap();
        assert_eq!(outcome, Some(result));
        assert!(!f.cache.lock().await.get_fragment("vm", "vm-a").is_some());
        assert!(!f.cache_path.exists());
    }

    #[tokio::test]
    async fn projection_malformed_request_is_skipped_but_runtime_still_executes() {
        let f = fixture();
        let stub = Arc::new(StubRuntime::new(vec![Ok(None)]));
        let wrapper = Arc::new(ProjectingCoreRuntime::new(
            stub.clone(),
            f.cache.clone(),
            f.cache_path.clone(),
        ));
        let outcome = wrapper
            .execute(entry(
                OperationKind::CreateVm,
                "vm-a",
                serde_json::json!({"not": "an envelope"}),
            ))
            .await
            .unwrap();
        // Succeeded outcome is preserved; the malformed request just skips the
        // projection (and its cache save).
        assert!(outcome.is_none());
        assert_eq!(stub.calls(), 1);
        assert!(!f.cache.lock().await.get_fragment("vm", "vm-a").is_some());
        assert!(!f.cache_path.exists());
    }

    #[tokio::test]
    async fn projection_lifecycle_mutations_update_desired_state() {
        let f = fixture();
        // Start with a projected VM so Start/Stop/Reboot have state to update.
        {
            let mut cache = f.cache.lock().await;
            cache.project_vm(&definition("vm-a"), "0".to_string(), "core".to_string());
        }
        for (kind, command, expected) in [
            (
                OperationKind::StartVm,
                MutationCommand::StartVm {
                    vm_id: VmId::new("vm-a").unwrap(),
                },
                "Running",
            ),
            (
                OperationKind::StopVm,
                MutationCommand::StopVm {
                    vm_id: VmId::new("vm-a").unwrap(),
                },
                "Stopped",
            ),
            (
                OperationKind::RebootVm,
                MutationCommand::RebootVm {
                    vm_id: VmId::new("vm-a").unwrap(),
                },
                "Running",
            ),
        ] {
            let stub = Arc::new(StubRuntime::new(vec![Ok(None)]));
            let wrapper = Arc::new(ProjectingCoreRuntime::new(
                stub.clone(),
                f.cache.clone(),
                f.cache_path.clone(),
            ));
            wrapper
                .execute(entry(kind, "vm-a", envelope(command)))
                .await
                .unwrap();
            let cache = f.cache.lock().await;
            let frag = cache.get_fragment("vm", "vm-a").unwrap();
            let spec =
                crate::spec::VmSpec::from_json(std::str::from_utf8(&frag.spec_json).unwrap())
                    .unwrap();
            assert_eq!(spec.desired_state, expected);
            drop(cache);
        }
    }

    #[tokio::test]
    async fn projection_delete_removes_vm_axis_state() {
        let f = fixture();
        {
            let mut cache = f.cache.lock().await;
            cache.project_vm(&definition("vm-a"), "0".to_string(), "core".to_string());
        }
        let stub = Arc::new(StubRuntime::new(vec![Ok(None)]));
        let wrapper = Arc::new(ProjectingCoreRuntime::new(
            stub.clone(),
            f.cache.clone(),
            f.cache_path.clone(),
        ));
        wrapper
            .execute(entry(
                OperationKind::DeleteVm,
                "vm-a",
                envelope(MutationCommand::DeleteVm {
                    vm_id: VmId::new("vm-a").unwrap(),
                }),
            ))
            .await
            .unwrap();
        let cache = f.cache.lock().await;
        assert!(!cache.get_fragment("vm", "vm-a").is_some());
        assert!(!cache.vm_attachment_state("vm-a").is_some());
        assert!(!cache.get_generation("vm", "vm-a").is_some());
    }

    // ---------------------------------------------------------------------
    // End-to-end: a real JournalExecutor+scheduler drives the wrapper through a
    // real store, and the Succeeded terminal outcome lands in NodeCache.
    // ---------------------------------------------------------------------
    #[tokio::test]
    async fn projection_is_driven_by_a_real_journal_execution() {
        use cellhv_core_operations::{AuthorityActor, OperationService, SubmitMutation};
        use cellhv_core_types::HostIdentity;

        let f = fixture();
        let dir = tempfile::tempdir().unwrap();
        // Same umask normalization as `fixture()`: the Core store's fresh-parent
        // integrity check requires an euid-owned 0700/0750 parent.
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let submissions = SubmitMutation {
            operation_id: OperationId::new("end-to-end").unwrap(),
            idempotency_scope: "test".to_string(),
            idempotency_key: IdempotencyKey::new("end-to-end").unwrap(),
            expected_vm_version: ResourceVersion::new(1).unwrap(),
            metadata: OperationRequestMetadata {
                requested_by: "integration-requester".to_string(),
                external_operation_id: "integration-external".to_string(),
                request_unix_ms: 1_700_000_000_000,
                legacy_generation: None,
            },
            command: MutationCommand::CreateVm {
                definition: {
                    let mut def = definition("vm-e2e");
                    def.storage = vec![cellhv_core_types::StorageAttachmentRef {
                        attachment_id: "vol-0".to_string(),
                        storage_ref: "vol-0".to_string(),
                        read_only: false,
                        size_bytes: None,
                        seed_from: None,
                        backend_class: None,
                    }];
                    def.networks = vec![cellhv_core_types::NetworkAttachmentRef {
                        attachment_id: "nic-0".to_string(),
                        network_ref: "net-0".to_string(),
                        mac_address: None,
                        addressing: None,
                        firewall_policy_json: None,
                    }];
                    def
                },
            },
        };
        let mut service = OperationService::create_new(
            &dir.path().join("core.db"),
            &HostIdentity {
                id: cellhv_core_types::HostId::new("node-1").unwrap(),
                resource_version: ResourceVersion::new(1).unwrap(),
            },
        )
        .unwrap();
        service.submit(submissions).unwrap();

        let (authority, join) = AuthorityActor::spawn(service, 32).unwrap();
        let execution = authority.execution_handle();
        let stub = Arc::new(StubRuntime::new(vec![Ok(None)]));
        let wrapper = Arc::new(ProjectingCoreRuntime::new(
            stub,
            f.cache.clone(),
            f.cache_path.clone(),
        ));
        let executor =
            cellhv_core_executor::JournalExecutor::start(execution, wrapper, 1, 2).unwrap();
        let scan = executor.scan_ready().await.unwrap();
        assert_eq!(scan.scheduled.len(), 1);
        let report = executor.shutdown().await.unwrap();
        assert_eq!(report.completed, 1);
        authority.shutdown().await.unwrap();
        join.join().await.unwrap();

        // The Succeeded CreateVm was projected into the compatibility cache and
        // persisted.
        let cache = f.cache.lock().await;
        let frag = cache.get_fragment("vm", "vm-e2e").unwrap();
        let spec =
            crate::spec::VmSpec::from_json(std::str::from_utf8(&frag.spec_json).unwrap()).unwrap();
        assert_eq!(spec.disks[0].volume_id, "vol-0");
        assert_eq!(spec.nics[0].network_id, "net-0");
        assert_eq!(frag.updated_by, "integration-requester");
        assert_eq!(frag.updated_at, "1700000000000");
        let persisted = NodeCache::load(&f.cache_path).await.unwrap();
        assert!(persisted.get_fragment("vm", "vm-e2e").is_some());
        drop(cache);
    }

    #[tokio::test]
    async fn projection_noop_arms_leave_cache_untouched() {
        use cellhv_core_types::StorageAttachmentRef;
        let f = fixture();
        let commands = [
            (
                OperationKind::UpdateVm,
                MutationCommand::UpdateVm {
                    definition: definition("vm-b"),
                },
            ),
            (
                OperationKind::AttachVolume,
                MutationCommand::AttachVolume {
                    vm_id: VmId::new("vm-b").unwrap(),
                    attachment: StorageAttachmentRef {
                        attachment_id: "vol-0".to_string(),
                        storage_ref: "vol-0".to_string(),
                        read_only: false,
                        size_bytes: None,
                        seed_from: None,
                        backend_class: None,
                    },
                },
            ),
            (
                OperationKind::DetachVolume,
                MutationCommand::DetachVolume {
                    vm_id: VmId::new("vm-b").unwrap(),
                    attachment_id: "vol-0".to_string(),
                },
            ),
            (
                OperationKind::AttachNetwork,
                MutationCommand::AttachNetwork {
                    vm_id: VmId::new("vm-b").unwrap(),
                    attachment: cellhv_core_types::NetworkAttachmentRef {
                        attachment_id: "nic-0".to_string(),
                        network_ref: "net-0".to_string(),
                        mac_address: None,
                        addressing: None,
                        firewall_policy_json: None,
                    },
                },
            ),
            (
                OperationKind::DetachNetwork,
                MutationCommand::DetachNetwork {
                    vm_id: VmId::new("vm-b").unwrap(),
                    attachment_id: "nic-0".to_string(),
                },
            ),
        ];
        for (kind, command) in commands {
            let stub = Arc::new(StubRuntime::new(vec![Ok(None)]));
            let wrapper = Arc::new(ProjectingCoreRuntime::new(
                stub.clone(),
                f.cache.clone(),
                f.cache_path.clone(),
            ));
            let outcome = wrapper
                .execute(entry(kind, "vm-b", envelope(command)))
                .await
                .unwrap();
            assert!(
                outcome.is_none(),
                "no-op arm must preserve the Succeeded outcome"
            );
        }
        // None of the out-of-RC-lifecycle commands may project or persist.
        let cache = f.cache.lock().await;
        assert!(!cache.get_fragment("vm", "vm-b").is_some());
        assert!(!cache.get_generation("vm", "vm-b").is_some());
        assert!(!cache.vm_attachment_state("vm-b").is_some());
        drop(cache);
        assert!(!f.cache_path.exists());
    }

    #[tokio::test]
    async fn projection_persist_failure_never_changes_the_outcome() {
        // Point the cache path at a directory that does not exist so save()
        // fails. The wrapper must still return the Succeeded outcome unchanged
        // and keep the projection in memory (the startup rebuild repairs the
        // on-disk staleness on the next restart).
        let f = fixture();
        let bad_path = f._dir.path().join("missing").join("cache.json");
        let stub = Arc::new(StubRuntime::new(vec![Ok(None)]));
        let wrapper = Arc::new(ProjectingCoreRuntime::new(
            stub.clone(),
            f.cache.clone(),
            bad_path,
        ));
        let outcome = wrapper
            .execute(entry(
                OperationKind::CreateVm,
                "vm-a",
                envelope(MutationCommand::CreateVm {
                    definition: definition("vm-a"),
                }),
            ))
            .await
            .unwrap();
        assert!(outcome.is_none());
        // Projection ran in-memory despite the persist failure; the returned
        // Result is untouched.
        assert!(f.cache.lock().await.get_fragment("vm", "vm-a").is_some());
    }

    /// M2.4 fault point 4 (after the provider effect, before the
    /// compatibility projection): simulated process death at the window
    /// leaves the effect done, the operation `running`, and the NodeCache
    /// un-projected. A restart rebuilds the cache from the Core store's
    /// desired state (repairing the projection gap), never re-executes the
    /// effect, and the operator resolution records the outcome the effect
    /// already had.
    #[tokio::test]
    async fn fault_after_effect_before_projection_is_repaired_by_rebuild() {
        use cellhv_core_executor::{FaultPoint, FaultRuntime};
        use cellhv_core_operations::{AuthorityActor, OperationService, SubmitMutation};
        use cellhv_core_types::HostIdentity;

        let f = fixture();
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let submissions = SubmitMutation {
            operation_id: OperationId::new("fault-p4").unwrap(),
            idempotency_scope: "test".to_string(),
            idempotency_key: IdempotencyKey::new("fault-p4").unwrap(),
            expected_vm_version: ResourceVersion::new(1).unwrap(),
            metadata: OperationRequestMetadata {
                requested_by: "integration-requester".to_string(),
                external_operation_id: "integration-external".to_string(),
                request_unix_ms: 1_700_000_000_000,
                legacy_generation: None,
            },
            command: MutationCommand::CreateVm {
                definition: definition("vm-p4"),
            },
        };
        let db_path = dir.path().join("core.db");
        let mut service = OperationService::create_new(
            &db_path,
            &HostIdentity {
                id: cellhv_core_types::HostId::new("node-1").unwrap(),
                resource_version: ResourceVersion::new(1).unwrap(),
            },
        )
        .unwrap();
        service.submit(submissions).unwrap();

        let (authority, join) = AuthorityActor::spawn(service, 32).unwrap();
        let execution = authority.execution_handle();
        // The fault runtime sits INSIDE the projecting wrapper: the stub
        // (provider effect) completes, then the task parks before the
        // projection can run.
        let stub = Arc::new(StubRuntime::new(vec![Ok(None), Ok(None)]));
        let fault = FaultRuntime::park_at(FaultPoint::AfterEffect, stub.clone());
        let wrapper = Arc::new(ProjectingCoreRuntime::new(
            fault.clone(),
            f.cache.clone(),
            f.cache_path.clone(),
        ));
        let executor =
            cellhv_core_executor::JournalExecutor::start(execution, wrapper, 1, 2).unwrap();
        executor.scan_ready().await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), fault.reached.notified())
            .await
            .expect("fault point must be reached");
        assert_eq!(
            stub.calls(),
            1,
            "the effect completed before the fault point"
        );
        // Process death at the fault point.
        executor.abort().await.unwrap();
        authority.shutdown().await.unwrap();
        join.join().await.unwrap();

        // The projection never ran: the cache has no fragment, and the
        // operation is still running with its claim fence.
        {
            let cache = f.cache.lock().await;
            assert!(cache.get_fragment("vm", "vm-p4").is_none());
        }

        // Restart: the production composition rebuilds the NodeCache from
        // the Core store's desired state before the executor starts.
        let service = OperationService::open_existing(&db_path).unwrap();
        let rebuild_vms = service.vms().unwrap();
        {
            let mut cache = f.cache.lock().await;
            cache.rebuild_from_core(&rebuild_vms);
        }
        {
            let cache = f.cache.lock().await;
            assert!(
                cache.get_fragment("vm", "vm-p4").is_some(),
                "the rebuild repairs the projection gap from desired state"
            );
        }
        let (authority, join) = AuthorityActor::spawn(service, 32).unwrap();
        let execution = authority.execution_handle();
        execution.classify_restart_interrupted().await.unwrap();
        let restart = execution.restart_operations().await.unwrap();
        assert_eq!(restart.len(), 1);
        assert_eq!(
            restart[0].disposition,
            cellhv_core_operations::RestartDisposition::InspectRequired
        );

        // The restarted executor never re-executes the effect.
        let executor = cellhv_core_executor::JournalExecutor::start(
            execution,
            Arc::new(ProjectingCoreRuntime::new(
                stub.clone(),
                f.cache.clone(),
                f.cache_path.clone(),
            )),
            1,
            2,
        )
        .unwrap();
        executor.scan_ready().await.unwrap();
        assert_eq!(stub.calls(), 1, "no second effect after restart");

        // Operator resolution: the effect did happen.
        authority
            .resolve_inspect_required(
                OperationId::new("fault-p4").unwrap(),
                true,
                "effect completed before fault point 4".to_string(),
            )
            .await
            .unwrap();
        let resolved = authority
            .operation(OperationId::new("fault-p4").unwrap())
            .await
            .unwrap();
        assert_eq!(
            resolved.operation.status,
            cellhv_core_types::OperationStatus::Succeeded
        );
        executor.shutdown().await.unwrap();
        authority.shutdown().await.unwrap();
        join.join().await.unwrap();
    }
}
