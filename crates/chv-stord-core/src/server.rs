use crate::handlers::StorageServiceImpl;
use crate::session::SessionTable;
use crate::store::SessionStore;
use chv_errors::ChvError;
use chv_observability::Metrics;
use chv_stord_api::chv_stord_api::storage_service_server::StorageServiceServer;
use chv_stord_backends::StorageBackend;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;
use tracing::info;

pub struct StorageServer<B: StorageBackend> {
    inner: StorageServiceImpl<B>,
}

impl<B: StorageBackend> StorageServer<B> {
    pub fn new(
        backend: B,
        metrics: Metrics,
        backend_allowlist: Vec<String>,
        store: Option<SessionStore>,
    ) -> Self {
        let backend = Arc::new(backend);
        let sessions = Arc::new(SessionTable::new());
        let mut inner =
            StorageServiceImpl::new(backend, sessions, Arc::new(metrics), backend_allowlist);
        if let Some(store) = store {
            inner.set_store(store);
        }
        Self { inner }
    }

    pub async fn serve(self, socket_path: &Path, db_path: Option<&Path>) -> Result<(), ChvError> {
        // Hydrate sessions from SQLite if db_path provided
        if let Some(db) = db_path {
            let db = db.to_path_buf();
            match tokio::task::spawn_blocking(move || SessionStore::new(&db)).await {
                Ok(Ok(store)) => match tokio::task::spawn_blocking(move || store.list()).await {
                    Ok(Ok(sessions)) => {
                        let table = self.inner.sessions();
                        for s in sessions {
                            table.upsert(s);
                        }
                        info!(count = table.list().len(), "hydrated sessions from SQLite");
                    }
                    Ok(Err(e)) => tracing::warn!(error = %e, "failed to list sessions from SQLite"),
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

        tokio::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o660))
            .await
            .map_err(|e| ChvError::Io {
                path: socket_path.to_string_lossy().to_string(),
                source: e,
            })?;

        let uds_stream = UnixListenerStream::new(uds);

        info!(socket = %socket_path.display(), "starting chv-stord server");

        Server::builder()
            .add_service(StorageServiceServer::new(self.inner))
            .serve_with_incoming(uds_stream)
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("server error: {e}"),
            })
    }
}
