use crate::handlers::StorageServiceImpl;
use crate::migration::sender::MigrationTlsConfig;
use crate::migration::service::StorageMigrationServiceImpl;
use crate::migration::tls_config::MigrationServerTls;
use crate::migration::MAX_MIGRATION_MESSAGE_SIZE_BYTES;
use crate::session::SessionTable;
use crate::store::SessionStore;
use chv_errors::ChvError;
use chv_observability::Metrics;
use chv_stord_api::chv_stord_api::storage_migration_service_server::StorageMigrationServiceServer;
use chv_stord_api::chv_stord_api::storage_service_server::StorageServiceServer;
use chv_stord_backends::StorageBackend;
use nix::unistd::{chown, Group};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use tokio::net::{TcpListener, UnixListener};
use tokio_stream::wrappers::{TcpListenerStream, UnixListenerStream};
use tonic::transport::Server;
use tracing::info;

pub struct StorageServer<B: StorageBackend> {
    inner: StorageServiceImpl<B>,
    migration_service: StorageMigrationServiceImpl<B>,
    /// Validated material for the migration receiver mTLS TCP listener
    /// (issue #390). `None` = source-only stord: no TCP listener is opened.
    migration_server_tls: Option<MigrationServerTls>,
}

impl<B: StorageBackend> StorageServer<B> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        backend: B,
        runtime_dir: std::path::PathBuf,
        metrics: Metrics,
        backend_allowlist: Vec<String>,
        path_allowlist: Vec<std::path::PathBuf>,
        device_allowlist: Vec<String>,
        migration_dest_allowlist: Vec<String>,
        migration_tls: Option<MigrationTlsConfig>,
        migration_server_tls: Option<MigrationServerTls>,
        store: Option<SessionStore>,
    ) -> Self {
        let backend = Arc::new(backend);
        let sessions = Arc::new(SessionTable::new());
        let mut inner = StorageServiceImpl::new(
            backend.clone(),
            sessions,
            Arc::new(metrics),
            runtime_dir.clone(),
            backend_allowlist,
            path_allowlist,
            device_allowlist,
            migration_dest_allowlist,
            migration_tls,
        );
        if let Some(store) = store {
            inner.set_store(store);
        }
        let migration_service = StorageMigrationServiceImpl::new(backend, runtime_dir);
        Self {
            inner,
            migration_service,
            migration_server_tls,
        }
    }

    pub async fn serve(self, socket_path: &Path, db_path: Option<&Path>) -> Result<(), ChvError> {
        // Hydrate sessions from SQLite if db_path provided
        if let Some(db) = db_path {
            let db = db.to_path_buf();
            match tokio::task::spawn_blocking(move || SessionStore::new(&db)).await {
                Ok(Ok(store)) => match store.list().await {
                    Ok(sessions) => {
                        let table = self.inner.sessions();
                        for s in sessions {
                            table.upsert(s);
                        }
                        info!(count = table.list().len(), "hydrated sessions from SQLite");
                    }
                    Err(e) => tracing::warn!(error = %e, "failed to list sessions from SQLite"),
                },
                Ok(Err(e)) => {
                    tracing::warn!(error = %e, "failed to open SQLite store; continuing with empty session table")
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to open SQLite store; continuing with empty session table")
                }
            }
        }

        if let Some(parent) = socket_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| ChvError::Io {
                    path: parent.to_string_lossy().to_string(),
                    source: e,
                })?;
        }

        if socket_path.exists() {
            tokio::fs::remove_file(socket_path)
                .await
                .map_err(|e| ChvError::Io {
                    path: socket_path.to_string_lossy().to_string(),
                    source: e,
                })?;
        }

        let uds = UnixListener::bind(socket_path).map_err(|e| ChvError::Io {
            path: socket_path.to_string_lossy().to_string(),
            source: e,
        })?;

        tokio::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600))
            .await
            .map_err(|e| ChvError::Io {
                path: socket_path.to_string_lossy().to_string(),
                source: e,
            })?;

        // Ensure the socket is owned by the chv-stord group for defense in depth.
        if let Ok(Some(group)) = Group::from_name("chv-stord") {
            let _ = chown(socket_path, None::<nix::unistd::Uid>, Some(group.gid));
        }

        let uds_stream = UnixListenerStream::new(uds);

        let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
        health_reporter
            .set_serving::<StorageServiceServer<StorageServiceImpl<B>>>()
            .await;

        info!(socket = %socket_path.display(), "starting chv-stord server");

        // Migration receiver mTLS TCP listener (issue #390). Binding happens
        // here, before the UDS server starts, so a bind failure is a startup
        // error (fail-closed) rather than a silent missing listener.
        let tls_serve = match self.migration_server_tls {
            Some(tls) => {
                let listener =
                    TcpListener::bind(tls.listen_addr)
                        .await
                        .map_err(|e| ChvError::Io {
                            path: tls.listen_addr.to_string(),
                            source: e,
                        })?;
                info!(
                    addr = %tls.listen_addr,
                    "storage migration receiver listening on {} (mTLS, client auth required)",
                    tls.listen_addr
                );
                Some(serve_migration_tls(
                    listener,
                    tls,
                    self.migration_service.clone(),
                ))
            }
            None => None,
        };

        let uds_serve = Server::builder()
            .layer(chv_observability::GrpcMetricsLayer::new())
            .add_service(health_service)
            .add_service(StorageServiceServer::new(self.inner))
            .add_service(
                StorageMigrationServiceServer::new(self.migration_service)
                    .max_decoding_message_size(MAX_MIGRATION_MESSAGE_SIZE_BYTES),
            )
            .serve_with_incoming(uds_stream);

        match tls_serve {
            None => uds_serve.await.map_err(|e| ChvError::Internal {
                reason: format!("server error: {e}"),
            }),
            // Fail fast: if either listener dies the daemon reports the error
            // instead of limping along with half its serving surface. Log at
            // the death site so operators see which listener failed even if
            // the process exit path does not print the error.
            Some(tls_serve) => tokio::select! {
                result = uds_serve => result.map_err(|e| {
                    tracing::error!("stord UDS server failed: {e}");
                    ChvError::Internal {
                        reason: format!("server error: {e}"),
                    }
                }),
                result = tls_serve => result.inspect_err(|e| {
                    tracing::error!("stord migration TLS listener failed: {e}");
                }),
            },
        }
    }
}

/// Serve `StorageMigrationService` on an already-bound TCP listener with
/// mandatory client-certificate (mTLS) authentication.
///
/// # What enforces mandatory client auth
///
/// The `ServerTlsConfig` is built with `client_ca_root(...)` and *without*
/// `client_auth_optional(true)` (tonic 0.12). Internally tonic then builds a
/// rustls `WebPkiClientVerifier` **without** `allow_unauthenticated()` and
/// installs it via `ServerConfig::with_client_cert_verifier`, so the TLS
/// handshake itself fails unless the peer presents a certificate that chains
/// to `tls.client_ca_pem`. This is the same mechanism the control plane uses
/// (`cmd/chv-controlplane/src/bootstrap.rs`).
///
/// This function is also the seam exercised by the loopback mTLS proof tests
/// (`tests/migration_mtls.rs`), which assert that clients without a valid
/// identity are rejected at the TLS layer.
pub async fn serve_migration_tls<B: StorageBackend>(
    listener: TcpListener,
    tls: MigrationServerTls,
    service: StorageMigrationServiceImpl<B>,
) -> Result<(), ChvError> {
    let identity = tonic::transport::Identity::from_pem(tls.cert_pem.clone(), tls.key_pem.clone());
    let tls_config = tonic::transport::ServerTlsConfig::new()
        .identity(identity)
        .client_ca_root(tonic::transport::Certificate::from_pem(
            tls.client_ca_pem.clone(),
        ));

    Server::builder()
        .layer(chv_observability::GrpcMetricsLayer::new())
        .tls_config(tls_config)
        .map_err(|e| ChvError::Internal {
            reason: format!("migration TLS server config error: {e}"),
        })?
        .add_service(
            StorageMigrationServiceServer::new(service)
                // BlockChunks carry a full 4 MiB migration block plus
                // protobuf overhead, exceeding tonic's 4 MiB default.
                .max_decoding_message_size(MAX_MIGRATION_MESSAGE_SIZE_BYTES),
        )
        .serve_with_incoming(TcpListenerStream::new(listener))
        .await
        .map_err(|e| ChvError::Internal {
            reason: format!("migration TLS server error: {e}"),
        })
}
