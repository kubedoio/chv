use crate::handlers::StorageServiceImpl;
use crate::session::SessionTable;
use chv_errors::ChvError;
use chv_observability::Metrics;
use chv_stord_api::chv_stord_api::storage_service_server::StorageServiceServer;
use chv_stord_backends::StorageBackend;
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
    pub fn new(backend: B, metrics: Metrics) -> Self {
        let backend = Arc::new(backend);
        let sessions = Arc::new(SessionTable::new());
        Self {
            inner: StorageServiceImpl::new(backend, sessions, Arc::new(metrics)),
        }
    }

    pub async fn serve(self, socket_path: &Path) -> Result<(), ChvError> {
        if let Some(parent) = socket_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                ChvError::Io {
                    path: parent.to_string_lossy().to_string(),
                    source: e,
                }
            })?;
        }

        if socket_path.exists() {
            tokio::fs::remove_file(socket_path).await.map_err(|e| {
                ChvError::Io {
                    path: socket_path.to_string_lossy().to_string(),
                    source: e,
                }
            })?;
        }

        let uds = UnixListener::bind(socket_path).map_err(|e| ChvError::Io {
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
