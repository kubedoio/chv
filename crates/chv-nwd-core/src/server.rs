use crate::handlers::NetworkServiceImpl;
use crate::state::TopologyTable;
use chv_errors::ChvError;
use chv_observability::Metrics;
use chv_nwd_api::chv_nwd_api::network_service_server::NetworkServiceServer;
use crate::executor::NetworkExecutor;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;
use tracing::info;

pub struct NetworkServer<E: NetworkExecutor> {
    inner: NetworkServiceImpl<E>,
}

impl<E: NetworkExecutor> NetworkServer<E> {
    pub fn new(executor: E, metrics: Metrics) -> Self {
        let executor = Arc::new(executor);
        let topologies = Arc::new(TopologyTable::new());
        Self {
            inner: NetworkServiceImpl::new(executor, topologies, Arc::new(metrics)),
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

        tokio::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o660))
            .await
            .map_err(|e| ChvError::Io {
                path: socket_path.to_string_lossy().to_string(),
                source: e,
            })?;

        let uds_stream = UnixListenerStream::new(uds);

        info!(socket = %socket_path.display(), "starting chv-nwd server");

        Server::builder()
            .add_service(NetworkServiceServer::new(self.inner))
            .serve_with_incoming(uds_stream)
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("server error: {e}"),
            })
    }
}
