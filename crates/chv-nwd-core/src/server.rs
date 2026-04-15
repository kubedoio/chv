use crate::executor::NetworkExecutor;
use crate::handlers::NetworkServiceImpl;
use crate::state::TopologyTable;
use crate::store::TopologyStore;
use chv_errors::ChvError;
use chv_nwd_api::chv_nwd_api::network_service_server::NetworkServiceServer;
use chv_observability::Metrics;
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
    pub fn new(executor: E, metrics: Metrics, store: Option<TopologyStore>) -> Self {
        let executor = Arc::new(executor);
        let topologies = Arc::new(TopologyTable::new());
        let mut inner = NetworkServiceImpl::new(executor, topologies, Arc::new(metrics));
        if let Some(store) = store {
            inner.set_store(store);
        }
        Self { inner }
    }

    pub async fn serve(self, socket_path: &Path, db_path: Option<&Path>) -> Result<(), ChvError> {
        // Hydrate topologies from SQLite if db_path provided
        if let Some(db) = db_path {
            let db = db.to_path_buf();
            match tokio::task::spawn_blocking(move || TopologyStore::new(&db)).await {
                Ok(Ok(store)) => match tokio::task::spawn_blocking(move || store.list()).await {
                    Ok(Ok(states)) => {
                        let table = self.inner.topologies();
                        for s in states {
                            table.upsert(s);
                        }
                        info!(
                            count = table.list().len(),
                            "hydrated topologies from SQLite"
                        );
                    }
                    Ok(Err(e)) => {
                        tracing::warn!(error = %e, "failed to list topologies from SQLite; continuing with empty topology table")
                    }
                    Err(e) => tracing::warn!(error = %e, "failed to join list topologies task"),
                },
                Ok(Err(e)) => {
                    tracing::warn!(error = %e, "failed to open SQLite store; continuing with empty topology table")
                }
                Err(e) => tracing::warn!(error = %e, "failed to join open SQLite store task"),
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

        if let Err(e) = tokio::fs::remove_file(socket_path).await {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(ChvError::Io {
                    path: socket_path.to_string_lossy().to_string(),
                    source: e,
                });
            }
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
