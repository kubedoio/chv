//! Unwired native-only composition owner for one CellHV Core runtime.
//!
//! This slice owns database exclusion, exactly one serialization actor, and
//! exactly one native API listener. It deliberately has no VM runtime or
//! NodeCache compatibility-mode dependencies.

use cellhv_core_api::{CoreApiListener, ListenerError};
use cellhv_core_operations::{
    AuthorityActor, AuthorityActorError, AuthorityActorJoin, AuthorityHandle,
};
use cellhv_core_startup::{
    ActivatedStore, ActivationKind, ActivationProvenance, RuntimeAuthorityGuard,
};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeOwnerError {
    #[error("Core runtime composition is not native-only: {0}")]
    Ineligible(&'static str),
    #[error("Core runtime actor startup failed: {0}")]
    ActorStartup(#[from] AuthorityActorError),
    #[error("Core runtime executor startup failed: {primary}; cleanup failures: {cleanup:?}")]
    ExecutorStartup {
        primary: cellhv_core_executor::ExecutorError,
        cleanup: Vec<RuntimeStageFailure>,
    },
    #[error("Core runtime listener startup failed: {primary}; cleanup failures: {cleanup:?}")]
    ListenerStartup {
        primary: ListenerError,
        cleanup: Vec<RuntimeStageFailure>,
    },
    #[error("Core runtime recovery startup failed: {primary}; cleanup failures: {cleanup:?}")]
    RecoveryStartup {
        primary: String,
        cleanup: Vec<RuntimeStageFailure>,
    },
    #[error("Core runtime shutdown failures: {0:?}")]
    Shutdown(Vec<RuntimeStageFailure>),
    #[error("invalid journal poller configuration: {:?}", .0)]
    InvalidPollerConfig(JournalPollerConfig),
}

#[derive(Debug, Error)]
pub enum RuntimeStageFailure {
    #[error("listener: {0}")]
    Listener(ListenerError),
    #[error("executor: {0}")]
    Executor(cellhv_core_executor::ExecutorError),
    #[error("recovery: {0}")]
    Recovery(String),
    #[error("actor shutdown: {0}")]
    ActorShutdown(AuthorityActorError),
    #[error("actor join: {0}")]
    ActorJoin(AuthorityActorError),
    #[error("executor scheduler task failed: {0}")]
    PollerJoin(tokio::task::JoinError),
    #[error("core executor drain exceeded {budget:?}; in-flight tasks were cancelled")]
    ExecutorDrainTimedOut { budget: Duration },
}

pub type Result<T> = std::result::Result<T, RuntimeOwnerError>;

/// Monotonic journal-poller failure telemetry. The cumulative counter is for
/// diagnostics; the consecutive counter is the current health signal (zero
/// means the journal scanner is working right now).
#[derive(Debug, Default)]
struct JournalScanStats {
    consecutive_failures: AtomicU64,
    total_failures: AtomicU64,
}

/// How the journal poller exited, surfaced through [`CoreRuntimeOwner::shutdown`].
enum PollerExit {
    Drained(cellhv_core_executor::ExecutionReport),
    Failed(cellhv_core_executor::ExecutorError),
    DrainTimedOut { budget: Duration },
}

/// Bounded exponential backoff for the journal poller: the base interval is
/// doubled per consecutive failure and capped, so a persistently failing store
/// is not re-scanned at maximum rate (which would both hammer the store and
/// spam the log).
const MAX_JOURNAL_SCAN_BACKOFF: Duration = Duration::from_secs(2);

fn backoff_delay(base: Duration, consecutive_failures: u32) -> Duration {
    let shift = consecutive_failures.min(3);
    base.saturating_mul(1u32 << shift)
        .min(MAX_JOURNAL_SCAN_BACKOFF)
}

/// Record one failed journal scan (either a returned error or a per-scan
/// timeout) and back off. Keeping this in one place ensures every failure path
/// bumps the health counters identically — a wedge must never masquerade as a
/// healthy journal.
fn record_scan_failure(
    stats: &JournalScanStats,
    consecutive_failures: &mut u32,
    delay: Duration,
    scan_timeout: Duration,
    error: Option<&cellhv_core_executor::ExecutorError>,
) {
    *consecutive_failures = consecutive_failures.saturating_add(1);
    stats
        .consecutive_failures
        .store(*consecutive_failures as u64, Ordering::Relaxed);
    stats.total_failures.fetch_add(1, Ordering::Relaxed);
    match error {
        Some(error) => tracing::warn!(
            consecutive_failures = *consecutive_failures,
            backoff_ms = delay.as_millis(),
            %error,
            "core journal scan failed; retrying with backoff"
        ),
        None => tracing::warn!(
            consecutive_failures = *consecutive_failures,
            backoff_ms = delay.as_millis(),
            scan_timeout_ms = scan_timeout.as_millis(),
            "core journal scan exceeded its timeout; treating as failure and retrying with backoff"
        ),
    }
}

/// Timing knobs for the journal polling loop that drives the executor.
#[derive(Clone, Copy, Debug)]
pub struct JournalPollerConfig {
    /// Base interval between journal scans (doubled per consecutive failure,
    /// capped at [`MAX_JOURNAL_SCAN_BACKOFF`]).
    pub scan_interval: Duration,
    /// Upper bound on a single `scan_ready`. A wedged authority/store hanging
    /// the scan RPC becomes a counted failure with backoff instead of wedging
    /// the poller (and shutdown).
    pub scan_timeout: Duration,
    /// Graceful drain budget at shutdown; on expiry the executor is explicitly
    /// cancelled (fail-closed -> InspectRequired).
    pub drain_budget: Duration,
}

/// Sole owner of the bounded native-only Core runtime composition.
///
/// The journal executor lives inside a background poller task that drives
/// `JournalExecutor::scan_ready` — the only ingress into the scheduler — at a
/// bounded interval. Without it, accepted operations are durable but never
/// claimed, executed, or finished. The poller owns the executor's lifecycle,
/// drains it with a bounded budget before exit (falling back to explicit
/// cancellation of the executor *task*; any runtime effect already in flight is
/// reconciled through the operation's `InspectRequired` disposition on
/// restart), and thereby preserves the executor-before-authority shutdown
/// ordering contract.
pub struct CoreRuntimeOwner {
    listener: Option<CoreApiListener>,
    authority: Option<AuthorityHandle>,
    actor_join: Option<AuthorityActorJoin>,
    poller: Option<tokio::task::JoinHandle<PollerExit>>,
    stop_tx: Option<tokio::sync::watch::Sender<()>>,
    journal_scan: std::sync::Arc<JournalScanStats>,
    kind: ActivationKind,
    provenance: ActivationProvenance,
    runtime_guard: Option<RuntimeAuthorityGuard>,
}

impl CoreRuntimeOwner {
    pub async fn start(
        runtime: std::sync::Arc<dyn cellhv_core_executor::CoreVmRuntime>,
        activated: ActivatedStore,
        socket: &Path,
        queue_capacity: usize,
        drain_timeout: Duration,
        poller: JournalPollerConfig,
    ) -> Result<Self> {
        let (service, kind, runtime_guard, provenance) = activated.into_runtime_parts();
        // Fail-closed on nonsensical timings: in release builds a zero
        // scan_interval would spin at max rate, a zero scan_timeout would mark
        // the journal permanently unhealthy, and a zero drain budget would
        // force-abort every shutdown. None of the production defaults (or test
        // knobs) are zero, so this only fires on hard misconfiguration.
        if poller.scan_interval == Duration::ZERO
            || poller.scan_timeout == Duration::ZERO
            || poller.drain_budget == Duration::ZERO
        {
            return Err(RuntimeOwnerError::InvalidPollerConfig(poller));
        }
        validate_native_only(kind, &provenance)?;
        let (authority, actor_join) = AuthorityActor::spawn(service, queue_capacity)?;
        let execution = authority.execution_handle();

        let executor = match cellhv_core_executor::JournalExecutor::start(
            execution,
            runtime,
            16,
            queue_capacity,
        ) {
            Ok(e) => e,
            Err(executor_err) => {
                let mut cleanup = Vec::new();
                if let Err(error) = authority.shutdown().await {
                    cleanup.push(RuntimeStageFailure::ActorShutdown(error));
                }
                drop(authority);
                if let Err(error) = actor_join.join().await {
                    cleanup.push(RuntimeStageFailure::ActorJoin(error));
                }
                return Err(RuntimeOwnerError::ExecutorStartup {
                    primary: executor_err,
                    cleanup,
                });
            }
        };
        let listener = match CoreApiListener::start_authority_owned_with_drain_timeout(
            socket,
            authority.clone(),
            drain_timeout,
        )
        .await
        {
            Ok(listener) => listener,
            Err(error) => {
                let mut cleanup = Vec::new();
                if let Err(error) = authority.shutdown().await {
                    cleanup.push(RuntimeStageFailure::ActorShutdown(error));
                }
                drop(authority);
                if let Err(error) = actor_join.join().await {
                    cleanup.push(RuntimeStageFailure::ActorJoin(error));
                }
                return Err(RuntimeOwnerError::ListenerStartup {
                    primary: error,
                    cleanup,
                });
            }
        };
        // Drive the journal: `scan_ready` is the only ingress into the executor
        // scheduler, so without this poller accepted operations remain durable
        // but are never claimed, executed, or finished. The poller owns the
        // executor and uses a bounded graceful drain at shutdown so no executor
        // *task* survives the authority (single-effector invariant); any runtime
        // effect already in flight is reconciled through the operation's
        // `InspectRequired` disposition on restart. Each scan is itself bounded
        // by `scan_timeout` so a wedged authority/store cannot hang the loop or
        // masquerade as a healthy journal: it becomes a counted scan failure
        // with backoff, and shutdown stays bounded. Repeated failures back off
        // exponentially and are surfaced as a persistent health signal instead
        // of per-tick log spam.
        let (stop_tx, mut stop_rx) = tokio::sync::watch::channel(());
        let journal_scan = std::sync::Arc::new(JournalScanStats::default());
        let poller = tokio::spawn({
            let journal_scan = std::sync::Arc::clone(&journal_scan);
            async move {
                let JournalPollerConfig {
                    scan_interval,
                    scan_timeout,
                    drain_budget,
                } = poller;
                let mut consecutive_failures: u32 = 0;
                loop {
                    let delay = backoff_delay(scan_interval, consecutive_failures);
                    tokio::select! {
                        _ = stop_rx.changed() => break,
                        _ = tokio::time::sleep(delay) => {
                            match tokio::time::timeout(scan_timeout, executor.scan_ready()).await {
                                Ok(Ok(_report)) => {
                                    consecutive_failures = 0;
                                    journal_scan
                                        .consecutive_failures
                                        .store(0, Ordering::Relaxed);
                                }
                                Ok(Err(error)) => record_scan_failure(
                                    &journal_scan,
                                    &mut consecutive_failures,
                                    delay,
                                    scan_timeout,
                                    Some(&error),
                                ),
                                Err(_elapsed) => record_scan_failure(
                                    &journal_scan,
                                    &mut consecutive_failures,
                                    delay,
                                    scan_timeout,
                                    None,
                                ),
                            }
                        }
                    }
                }
                match executor.shutdown_bounded(drain_budget).await {
                    Ok(report) => PollerExit::Drained(report),
                    Err(cellhv_core_executor::ExecutorError::DrainTimedOut { budget }) => {
                        PollerExit::DrainTimedOut { budget }
                    }
                    Err(error) => PollerExit::Failed(error),
                }
            }
        });
        Ok(Self {
            listener: Some(listener),
            authority: Some(authority),
            actor_join: Some(actor_join),
            poller: Some(poller),
            stop_tx: Some(stop_tx),
            journal_scan,
            kind,
            provenance,
            runtime_guard: Some(runtime_guard),
        })
    }

    pub fn authority(&self) -> cellhv_core_operations::AuthorityHandle {
        self.authority
            .as_ref()
            .expect("authority is present before shutdown")
            .clone()
    }

    pub fn socket_path(&self) -> &Path {
        self.listener
            .as_ref()
            .expect("listener is present before shutdown")
            .socket_path()
    }

    pub fn activation_kind(&self) -> ActivationKind {
        self.kind
    }

    pub fn provenance(&self) -> &ActivationProvenance {
        &self.provenance
    }

    /// Cumulative number of failed journal scans since this owner started
    /// (diagnostics / observability).
    pub fn journal_scan_failures(&self) -> u64 {
        self.journal_scan.total_failures.load(Ordering::Relaxed)
    }

    /// Whether the journal scanner is currently healthy: the most recent scan
    /// did not fail. This is the health signal a production composition feeds
    /// into the agent's health aggregation so a silently wedged journal becomes
    /// visible instead of returning 200/202 forever.
    pub fn journal_scan_healthy(&self) -> bool {
        self.journal_scan
            .consecutive_failures
            .load(Ordering::Relaxed)
            == 0
    }

    /// Stops the listener first, then the actor, and releases the runtime lease
    /// only after the actor thread has joined.
    pub async fn shutdown(mut self) -> Result<()> {
        let mut failures = Vec::new();
        if let Err(error) = self
            .listener
            .take()
            .expect("listener is present before shutdown")
            .shutdown()
            .await
        {
            failures.push(RuntimeStageFailure::Listener(error));
        }

        // Stop the scan loop and join the poller, which drains the executor
        // (graceful shutdown) before we shut the actor down. This preserves the
        // executor-before-authority ordering contract.
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        let poller = self
            .poller
            .take()
            .expect("poller is present before shutdown");
        match poller.await {
            Ok(PollerExit::Drained(report)) => {
                tracing::debug!(completed = report.completed, "core executor drained")
            }
            Ok(PollerExit::Failed(error)) => failures.push(RuntimeStageFailure::Executor(error)),
            Ok(PollerExit::DrainTimedOut { budget }) => {
                tracing::warn!(
                    budget_ms = budget.as_millis(),
                    "core executor drain exceeded budget; in-flight tasks were cancelled (ops left Running reconcile as InspectRequired on restart)"
                );
                failures.push(RuntimeStageFailure::ExecutorDrainTimedOut { budget })
            }
            Err(join_error) => failures.push(RuntimeStageFailure::PollerJoin(join_error)),
        }

        let authority = self
            .authority
            .take()
            .expect("authority is present before shutdown");
        if let Err(error) = authority.shutdown().await {
            failures.push(RuntimeStageFailure::ActorShutdown(error));
        }
        drop(authority);
        if let Err(error) = self
            .actor_join
            .take()
            .expect("actor join is present before shutdown")
            .join()
            .await
        {
            failures.push(RuntimeStageFailure::ActorJoin(error));
        }
        drop(self.runtime_guard.take());
        if failures.is_empty() {
            Ok(())
        } else {
            Err(RuntimeOwnerError::Shutdown(failures))
        }
    }
}

impl Drop for CoreRuntimeOwner {
    fn drop(&mut self) {
        drop(self.listener.take());
        if let Some(poller) = self.poller.take() {
            // Emergency path: abort the poller. Dropping the owned executor
            // aborts its scheduler task, and the runtime lease is retained
            // until process exit via abandonment below (no split authority).
            poller.abort();
        }
        drop(self.stop_tx.take());
        drop(self.authority.take());
        drop(self.actor_join.take());
        if let Some(runtime_guard) = self.runtime_guard.take() {
            // Abandonment cannot observe the asynchronous actor reaper. Keep
            // the lease until process exit rather than permit split authority.
            std::mem::forget(runtime_guard);
        }
    }
}

fn validate_native_only(kind: ActivationKind, provenance: &ActivationProvenance) -> Result<()> {
    if provenance.source_checksum().is_some() {
        return Err(RuntimeOwnerError::Ineligible(
            "NodeCache migration provenance is present",
        ));
    }
    if provenance.live_cache_present() {
        return Err(RuntimeOwnerError::Ineligible(
            "a live NodeCache snapshot is present",
        ));
    }
    if provenance.has_any_migration_state() {
        return Err(RuntimeOwnerError::Ineligible(
            "durable migration state is present",
        ));
    }
    if kind == ActivationKind::ImportedNodeCache {
        return Err(RuntimeOwnerError::Ineligible(
            "the database was imported from NodeCache",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cellhv_core_operations::OperationService;
    use cellhv_core_startup::{StartupPaths, StartupTransaction};
    use cellhv_core_types::{HostId, HostIdentity, OperationStatus, ResourceVersion};
    use std::os::unix::fs::PermissionsExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn paths(directory: &tempfile::TempDir) -> StartupPaths {
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        StartupPaths {
            node_cache: directory.path().join("node-cache.json"),
            core_database: directory.path().join("core.db"),
            node_cache_archive: directory.path().join("node-cache.archive"),
        }
    }

    async fn request(socket: &Path, target: &str) -> String {
        let mut stream = tokio::net::UnixStream::connect(socket).await.unwrap();
        stream
            .write_all(
                format!("GET {target} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();
        response
    }

    fn fresh(paths: &StartupPaths, id: &str) -> ActivatedStore {
        StartupTransaction::begin(paths)
            .unwrap()
            .activate(Some(id.to_owned()), None)
            .unwrap()
    }

    struct DummyRuntime;

    #[async_trait::async_trait]
    impl cellhv_core_executor::CoreVmRuntime for DummyRuntime {
        async fn execute(
            &self,
            _operation: cellhv_core_operations::OperationJournalEntry,
        ) -> std::result::Result<Option<serde_json::Value>, cellhv_core_executor::RuntimeFailure>
        {
            Ok(None)
        }
    }

    #[tokio::test]
    async fn native_runtime_serves_identity_restarts_and_excludes_second_runtime() {
        let directory = tempfile::tempdir().unwrap();
        let paths = paths(&directory);
        let socket = directory.path().join("core.sock");
        let owner = CoreRuntimeOwner::start(
            std::sync::Arc::new(DummyRuntime),
            fresh(&paths, "native-host"),
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
        let response = request(&socket, "/v1/host").await;
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.contains("native-host"));
        assert!(StartupTransaction::begin(&paths).is_err());
        owner.shutdown().await.unwrap();

        let restarted = StartupTransaction::begin(&paths)
            .unwrap()
            .activate(Some("native-host".to_owned()), None)
            .unwrap();
        assert_eq!(restarted.kind(), ActivationKind::Existing);
        let owner = CoreRuntimeOwner::start(
            std::sync::Arc::new(DummyRuntime),
            restarted,
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
        assert!(request(&socket, "/v1/host").await.contains("native-host"));
        owner.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn actor_spawn_failure_releases_runtime_lease() {
        let directory = tempfile::tempdir().unwrap();
        let paths = paths(&directory);
        let socket = directory.path().join("core.sock");
        assert!(matches!(
            CoreRuntimeOwner::start(
                std::sync::Arc::new(DummyRuntime),
                fresh(&paths, "actor-failure"),
                &socket,
                0,
                Duration::from_secs(1),
                JournalPollerConfig {
                    scan_interval: Duration::from_millis(40),
                    scan_timeout: Duration::from_secs(1),
                    drain_budget: Duration::from_secs(1),
                },
            )
            .await,
            Err(RuntimeOwnerError::ActorStartup(
                AuthorityActorError::InvalidCapacity
            ))
        ));
        drop(StartupTransaction::begin(&paths).unwrap());
        assert!(!socket.exists());
    }

    #[tokio::test]
    async fn listener_bind_failure_stops_actor_and_releases_runtime_lease() {
        let directory = tempfile::tempdir().unwrap();
        let paths = paths(&directory);
        let socket = directory.path().join("core.sock");
        std::fs::write(&socket, b"foreign").unwrap();
        assert!(matches!(
            CoreRuntimeOwner::start(
                std::sync::Arc::new(DummyRuntime),
                fresh(&paths, "listener-failure"),
                &socket,
                16,
                Duration::from_secs(1),
                JournalPollerConfig {
                    scan_interval: Duration::from_millis(40),
                    scan_timeout: Duration::from_secs(1),
                    drain_budget: Duration::from_secs(1),
                },
            )
            .await,
            Err(RuntimeOwnerError::ListenerStartup { .. })
        ));
        assert_eq!(std::fs::read(&socket).unwrap(), b"foreign");
        drop(StartupTransaction::begin(&paths).unwrap());
    }

    #[tokio::test]
    async fn restart_recovers_socket_left_by_unclean_process_exit() {
        let directory = tempfile::tempdir().unwrap();
        let paths = paths(&directory);
        let socket = directory.path().join("core.sock");
        let stale = tokio::net::UnixListener::bind(&socket).unwrap();
        drop(stale);

        let owner = CoreRuntimeOwner::start(
            std::sync::Arc::new(DummyRuntime),
            fresh(&paths, "recovered-host"),
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
        assert!(request(&socket, "/v1/host")
            .await
            .contains("recovered-host"));
        owner.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn imported_nodecache_is_refused_before_actor_or_listener_creation() {
        let directory = tempfile::tempdir().unwrap();
        let paths = paths(&directory);
        let socket = directory.path().join("core.sock");
        let spec = serde_json::json!({
            "name":"legacy-vm", "cpus":1, "memory_bytes":1073741824_u64,
            "kernel_path":"/kernel", "disks":[], "nics":[], "desired_state":"Stopped"
        });
        let source = serde_json::json!({
            "cache_version":1, "node_id":"legacy-host", "observed_generation":"1",
            "node_state":"TenantReady", "enrollment_complete":true,
            "vm_generations":{"vm-1":"1"}, "volume_generations":{}, "network_generations":{},
            "vm_fragments":{"vm-1":{"id":"vm-1","kind":"vm","generation":"1",
                "spec_json":serde_json::to_vec(&spec).unwrap(),
                "policy_json":serde_json::to_vec(&serde_json::json!({})).unwrap(),
                "updated_at":"now","updated_by":"controller"}},
            "volume_fragments":{}, "network_fragments":{}, "vm_attachments":{},
            "volume_handles":{}, "pending_control_plane":[]
        });
        std::fs::write(&paths.node_cache, serde_json::to_vec(&source).unwrap()).unwrap();
        std::fs::set_permissions(&paths.node_cache, std::fs::Permissions::from_mode(0o600))
            .unwrap();
        let activated = StartupTransaction::begin(&paths)
            .unwrap()
            .activate(Some("legacy-host".to_owned()), None)
            .unwrap();
        assert!(matches!(
            CoreRuntimeOwner::start(
                std::sync::Arc::new(DummyRuntime),
                activated,
                &socket,
                16,
                Duration::from_secs(1),
                JournalPollerConfig {
                    scan_interval: Duration::from_millis(40),
                    scan_timeout: Duration::from_secs(1),
                    drain_budget: Duration::from_secs(1),
                },
            )
            .await,
            Err(RuntimeOwnerError::Ineligible(_))
        ));
        assert!(!socket.exists());
        drop(StartupTransaction::begin(&paths).unwrap());
    }

    #[tokio::test]
    async fn arbitrary_migration_source_is_not_misclassified_as_native() {
        let directory = tempfile::tempdir().unwrap();
        let paths = paths(&directory);
        let socket = directory.path().join("core.sock");
        let mut service = OperationService::create_migration_target(&paths.core_database).unwrap();
        let host = HostIdentity {
            id: HostId::new("foreign-import").unwrap(),
            resource_version: ResourceVersion::new(1).unwrap(),
        };
        service
            .import_legacy_snapshot("another-importer", "checksum", &host, &[])
            .unwrap();
        service
            .cutover_legacy_snapshot("another-importer", "checksum")
            .unwrap();
        drop(service);

        let activated = StartupTransaction::begin(&paths)
            .unwrap()
            .activate(Some("foreign-import".to_owned()), None)
            .unwrap();
        assert!(activated.provenance().has_any_migration_state());
        assert!(matches!(
            CoreRuntimeOwner::start(
                std::sync::Arc::new(DummyRuntime),
                activated,
                &socket,
                16,
                Duration::from_secs(1),
                JournalPollerConfig {
                    scan_interval: Duration::from_millis(40),
                    scan_timeout: Duration::from_secs(1),
                    drain_budget: Duration::from_secs(1),
                },
            )
            .await,
            Err(RuntimeOwnerError::Ineligible(
                "durable migration state is present"
            ))
        ));
        assert!(!socket.exists());
    }

    #[tokio::test]
    async fn implicit_drop_retains_runtime_lease_fail_closed() {
        let directory = tempfile::tempdir().unwrap();
        let paths = paths(&directory);
        let socket = directory.path().join("core.sock");
        let owner = CoreRuntimeOwner::start(
            std::sync::Arc::new(DummyRuntime),
            fresh(&paths, "abandoned-runtime"),
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
        drop(owner);
        assert!(StartupTransaction::begin(&paths).is_err());
    }

    #[test]
    fn shutdown_failure_container_preserves_every_stage() {
        let failures = vec![
            RuntimeStageFailure::Listener(ListenerError::DrainTimeout(Duration::from_secs(1))),
            RuntimeStageFailure::ActorShutdown(AuthorityActorError::Unavailable),
            RuntimeStageFailure::ActorJoin(AuthorityActorError::ThreadPanicked),
        ];
        let error = RuntimeOwnerError::Shutdown(failures);
        assert!(matches!(error, RuntimeOwnerError::Shutdown(values) if values.len() == 3));
    }

    struct RecordingRuntime {
        executed: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl cellhv_core_executor::CoreVmRuntime for RecordingRuntime {
        async fn execute(
            &self,
            operation: cellhv_core_operations::OperationJournalEntry,
        ) -> std::result::Result<Option<serde_json::Value>, cellhv_core_executor::RuntimeFailure>
        {
            self.executed.lock().unwrap().push(format!(
                "{}:{}",
                operation.operation.vm_id.as_str(),
                operation.operation.id.as_str()
            ));
            Ok(None)
        }
    }

    fn create_submission(vm: &str, op: &str) -> cellhv_core_operations::SubmitMutation {
        use cellhv_core_operations::{MutationCommand, SubmitMutation};
        use cellhv_core_types::{
            BootSpec, ComputeSpec, IdempotencyKey, ObservedPowerState, OperationId,
            RequestedPowerState, ResourceVersion, VmDefinition, VmId,
        };
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

    /// The production composition must claim, execute, and finish an accepted
    /// operation WITHOUT any external caller driving the executor: the
    /// composition-internal journal poller is the only scheduler ingress. This
    /// is the regression test for the "effect-dead journal" gap (accepted-but-
    /// never-executed operations under core-native / core-managed).
    #[tokio::test]
    async fn poller_executes_accepted_operations_in_production_composition() {
        let directory = tempfile::tempdir().unwrap();
        let paths = paths(&directory);
        let socket = directory.path().join("core.sock");
        let executed = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let owner = CoreRuntimeOwner::start(
            std::sync::Arc::new(RecordingRuntime {
                executed: executed.clone(),
            }),
            fresh(&paths, "poller-host"),
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

        owner
            .authority()
            .submit(create_submission("vm-a", "op-a"))
            .await
            .unwrap();

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let entries = owner.authority().operations().await.unwrap();
            if entries.iter().any(|entry| {
                entry.operation.id.as_str() == "op-a"
                    && entry.operation.status == OperationStatus::Succeeded
            }) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "accepted operation never reached Succeeded in the production composition"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        {
            let recorded = executed.lock().unwrap();
            assert_eq!(
                recorded
                    .iter()
                    .filter(|e| e.starts_with("vm-a:op-a"))
                    .count(),
                1,
                "the runtime must execute each accepted operation exactly once"
            );
        }
        // A working journal must report itself healthy with zero failures.
        assert!(owner.journal_scan_healthy());
        assert_eq!(owner.journal_scan_failures(), 0);
        owner.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn start_rejects_zero_duration_poller_config() {
        let directory = tempfile::tempdir().unwrap();
        let paths = paths(&directory);
        let socket = directory.path().join("core.sock");
        let invalid = [
            JournalPollerConfig {
                scan_interval: Duration::ZERO,
                scan_timeout: Duration::from_secs(1),
                drain_budget: Duration::from_secs(1),
            },
            JournalPollerConfig {
                scan_interval: Duration::from_millis(40),
                scan_timeout: Duration::ZERO,
                drain_budget: Duration::from_secs(1),
            },
            JournalPollerConfig {
                scan_interval: Duration::from_millis(40),
                scan_timeout: Duration::from_secs(1),
                drain_budget: Duration::ZERO,
            },
        ];
        for cfg in invalid {
            let result = CoreRuntimeOwner::start(
                std::sync::Arc::new(DummyRuntime),
                fresh(&paths, "native-host"),
                &socket,
                16,
                Duration::from_secs(1),
                cfg,
            )
            .await;
            // Fail-closed: a zero timing must be rejected up front, not behave
            // divergently in a release build (spin, always-unhealthy, or
            // force-abort on shutdown).
            match result {
                Ok(_owner) => panic!("zero-duration config must be rejected: {cfg:?}"),
                Err(error) => assert!(
                    matches!(error, RuntimeOwnerError::InvalidPollerConfig(_)),
                    "expected InvalidPollerConfig for {cfg:?}, got {error:?}"
                ),
            }
        }
    }
}
