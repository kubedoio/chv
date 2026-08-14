use crate::migration::receiver::MigrationReceiver;
use chv_stord_api::chv_stord_api::{
    migration_message, storage_migration_service_server::StorageMigrationService, MigrationMessage,
    MigrationReady,
};
use chv_stord_backends::StorageBackend;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::Stream;
use tonic::{Request, Response, Status, Streaming};
use tracing::{error, info};

/// Maximum volume size accepted for a migration (16 TiB).
const MAX_MIGRATION_SIZE_BYTES: u64 = 16 * 1024 * 1024 * 1024 * 1024;

/// Tonic service implementation for the storage migration receiver.
///
/// This handles incoming bidirectional streams. The first message in the
/// stream must be InitMigration, which causes this node to act as the
/// migration destination (receiver).
pub struct StorageMigrationServiceImpl<B: StorageBackend> {
    backend: Arc<B>,
    runtime_dir: Option<std::path::PathBuf>,
}

impl<B: StorageBackend> StorageMigrationServiceImpl<B> {
    pub fn new(backend: Arc<B>, runtime_dir: std::path::PathBuf) -> Self {
        Self {
            backend,
            runtime_dir: Some(runtime_dir),
        }
    }

    /// Defense in depth at the migration gRPC boundary.
    ///
    /// The local backend builds the receiving volume path as
    /// `runtime_dir/{volume_id}.img`, so a hostile peer could escape the
    /// runtime directory via `../` components in `volume_id`. The agent-side
    /// `is_safe_id` check already blocks such ids before they reach stord,
    /// but a peer stord is its own trust boundary: reject unsafe ids here
    /// too, and additionally verify that the resolved receiving path stays
    /// inside `runtime_dir` (fail closed on any resolution error).
    fn validate_receiving_volume_id(&self, volume_id: &str) -> Result<(), Status> {
        if !chv_common::is_safe_id(volume_id) {
            return Err(Status::invalid_argument(format!(
                "volume_id '{volume_id}' is not a safe id (path separators or traversal rejected)"
            )));
        }
        if let Some(runtime_dir) = self.runtime_dir.as_deref() {
            let dest = runtime_dir.join(format!("{}.img", volume_id));
            let canonical = crate::handlers::canonicalize_or_ancestor(&dest)
                .map_err(|e| Status::permission_denied(e.to_string()))?;
            let canonical_runtime = crate::handlers::canonicalize_or_ancestor(runtime_dir)
                .map_err(|e| Status::permission_denied(e.to_string()))?;
            if !canonical.starts_with(&canonical_runtime) {
                return Err(Status::permission_denied(format!(
                    "receiving volume path '{}' escapes runtime_dir",
                    dest.display()
                )));
            }
        }
        Ok(())
    }
}

type ResponseStream = Pin<Box<dyn Stream<Item = Result<MigrationMessage, Status>> + Send>>;

#[tonic::async_trait]
impl<B: StorageBackend> StorageMigrationService for StorageMigrationServiceImpl<B> {
    type StreamBlocksStream = ResponseStream;

    async fn stream_blocks(
        &self,
        request: Request<Streaming<MigrationMessage>>,
    ) -> Result<Response<Self::StreamBlocksStream>, Status> {
        let mut inbound = request.into_inner();

        // Read the first message to determine role
        let first_msg = inbound
            .message()
            .await?
            .ok_or_else(|| Status::invalid_argument("stream closed without sending a message"))?;

        let init = match first_msg.payload {
            Some(migration_message::Payload::Init(init)) => init,
            _ => {
                return Err(Status::invalid_argument(
                    "first message must be InitMigration",
                ));
            }
        };

        if init.size_bytes > MAX_MIGRATION_SIZE_BYTES {
            return Err(Status::invalid_argument(format!(
                "InitMigration size_bytes {} exceeds maximum allowed {} bytes",
                init.size_bytes, MAX_MIGRATION_SIZE_BYTES
            )));
        }

        // Defense in depth: never let a peer-supplied volume_id escape the
        // runtime directory when the backend constructs the receiving path.
        self.validate_receiving_volume_id(&init.volume_id)?;

        info!(
            volume_id = %init.volume_id,
            size_bytes = init.size_bytes,
            block_size = init.block_size,
            format = %init.format,
            "received InitMigration, starting receiver"
        );

        // Create the receiving volume
        let export = self
            .backend
            .create_receiving_volume(&init.volume_id, init.size_bytes, &init.format)
            .await
            .map_err(|e| {
                error!(
                    volume_id = %init.volume_id,
                    error = %e,
                    "failed to create receiving volume"
                );
                Status::internal(format!("failed to create receiving volume: {e}"))
            })?;

        let dest_volume_id = init.volume_id.clone();
        let handle = export.attachment_handle.clone();

        // Set up outgoing response channel
        let (tx, rx) = mpsc::channel::<MigrationMessage>(256);

        // Send MigrationReady
        let ready_msg = MigrationMessage {
            payload: Some(migration_message::Payload::Ready(MigrationReady {
                dest_volume_id: dest_volume_id.clone(),
            })),
        };
        tx.send(ready_msg)
            .await
            .map_err(|_| Status::internal("failed to send MigrationReady: channel closed"))?;

        // Spawn the receiver loop
        let backend = self.backend.clone();
        let volume_id = dest_volume_id.clone();
        tokio::spawn(async move {
            let receiver =
                MigrationReceiver::new(backend, volume_id.clone(), handle, init.size_bytes);
            if let Err(e) = receiver.run(inbound, tx).await {
                error!(
                    volume_id = %volume_id,
                    error = %e,
                    "migration receiver failed"
                );
            }
        });

        // Return the response stream
        let output_stream = ReceiverStream::new(rx);
        let mapped_stream: ResponseStream =
            Box::pin(tokio_stream::StreamExt::map(output_stream, Ok));

        Ok(Response::new(mapped_stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chv_stord_backends::LocalFileBackend;

    fn make_service(
        runtime_dir: std::path::PathBuf,
    ) -> StorageMigrationServiceImpl<LocalFileBackend> {
        StorageMigrationServiceImpl::new(
            Arc::new(LocalFileBackend::new(runtime_dir.clone())),
            runtime_dir,
        )
    }

    #[test]
    fn receiving_volume_id_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let svc = make_service(dir.path().to_path_buf());
        assert!(svc.validate_receiving_volume_id("../escape").is_err());
        assert!(svc
            .validate_receiving_volume_id("../../etc/passwd")
            .is_err());
        assert!(svc.validate_receiving_volume_id("a/b").is_err());
        assert!(svc.validate_receiving_volume_id("a\\b").is_err());
        assert!(svc.validate_receiving_volume_id("a..b").is_err());
        assert!(svc.validate_receiving_volume_id("").is_err());
    }

    #[test]
    fn receiving_volume_id_accepts_safe_ids() {
        let dir = tempfile::tempdir().unwrap();
        let svc = make_service(dir.path().to_path_buf());
        assert!(svc.validate_receiving_volume_id("vol-1").is_ok());
        assert!(svc.validate_receiving_volume_id("vm_2.disk").is_ok());
    }

    #[test]
    fn receiving_volume_id_fails_closed_on_unresolvable_runtime_dir() {
        // A symlink loop under runtime_dir cannot be canonicalized; the
        // containment check must fail closed instead of skipping.
        let dir = tempfile::tempdir().unwrap();
        let loop_path = dir.path().join("loop");
        std::os::unix::fs::symlink(&loop_path, &loop_path).unwrap();
        let svc = make_service(loop_path);
        assert!(svc.validate_receiving_volume_id("vol-1").is_err());
    }
}
