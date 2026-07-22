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
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeOwnerError {
    #[error("Core runtime composition is not native-only: {0}")]
    Ineligible(&'static str),
    #[error(transparent)]
    Actor(#[from] AuthorityActorError),
    #[error("Core runtime listener startup failed: {primary}; cleanup failures: {cleanup:?}")]
    Startup {
        primary: ListenerError,
        cleanup: Vec<RuntimeStageFailure>,
    },
    #[error("Core runtime shutdown failures: {0:?}")]
    Shutdown(Vec<RuntimeStageFailure>),
}

#[derive(Debug, Error)]
pub enum RuntimeStageFailure {
    #[error("listener: {0}")]
    Listener(ListenerError),
    #[error("actor shutdown: {0}")]
    ActorShutdown(AuthorityActorError),
    #[error("actor join: {0}")]
    ActorJoin(AuthorityActorError),
}

pub type Result<T> = std::result::Result<T, RuntimeOwnerError>;

/// Sole owner of the bounded native-only Core runtime composition.
pub struct CoreRuntimeOwner {
    listener: Option<CoreApiListener>,
    authority: Option<AuthorityHandle>,
    actor_join: Option<AuthorityActorJoin>,
    executor: Option<cellhv_core_executor::JournalExecutor>,
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
    ) -> Result<Self> {
        let (service, kind, runtime_guard, provenance) = activated.into_runtime_parts();
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
            Err(_) => {
                let mut cleanup = Vec::new();
                if let Err(error) = authority.shutdown().await {
                    cleanup.push(RuntimeStageFailure::ActorShutdown(error));
                }
                drop(authority);
                if let Err(error) = actor_join.join().await {
                    cleanup.push(RuntimeStageFailure::ActorJoin(error));
                }
                return Err(RuntimeOwnerError::Startup {
                    primary: ListenerError::DrainTimeout(Duration::from_secs(0)), // Hack for error match
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
                return Err(RuntimeOwnerError::Startup {
                    primary: error,
                    cleanup,
                });
            }
        };
        Ok(Self {
            listener: Some(listener),
            authority: Some(authority),
            actor_join: Some(actor_join),
            executor: Some(executor),
            kind,
            provenance,
            runtime_guard: Some(runtime_guard),
        })
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

        let executor = self
            .executor
            .take()
            .expect("executor is present before shutdown");
        drop(executor); // shutdown the executor before the actor

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
        drop(self.executor.take());
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
    use cellhv_core_types::{HostId, HostIdentity, ResourceVersion};
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
                Duration::from_secs(1)
            )
            .await,
            Err(RuntimeOwnerError::Actor(
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
                Duration::from_secs(1)
            )
            .await,
            Err(RuntimeOwnerError::Startup { .. })
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
                Duration::from_secs(1)
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
                Duration::from_secs(1)
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
}
