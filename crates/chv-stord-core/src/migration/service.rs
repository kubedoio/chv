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

/// Tonic service implementation for the storage migration receiver.
///
/// This handles incoming bidirectional streams. The first message in the
/// stream must be InitMigration, which causes this node to act as the
/// migration destination (receiver).
pub struct StorageMigrationServiceImpl<B: StorageBackend> {
    backend: Arc<B>,
}

impl<B: StorageBackend> StorageMigrationServiceImpl<B> {
    pub fn new(backend: Arc<B>) -> Self {
        Self { backend }
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
            let receiver = MigrationReceiver::new(backend, volume_id.clone(), handle);
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
