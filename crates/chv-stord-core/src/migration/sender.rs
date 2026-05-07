use crate::migration::flow_control::SendWindow;
use chv_stord_api::chv_stord_api::{
    migration_message, storage_migration_service_client::StorageMigrationServiceClient, AckStatus,
    BlockChunk, FinalSync, FinalizeComplete, InitMigration, MigrationMessage, RoundComplete,
    RoundStart,
};
use chv_stord_backends::StorageBackend;
use std::sync::Arc;
use tokio::sync::mpsc;
use tonic::transport::Channel;
use tracing::{debug, error, info, warn};

const DEFAULT_BLOCK_SIZE: u64 = 4_194_304; // 4 MB

/// Drives the source side of a storage migration.
///
/// This is invoked by the control plane / agent to migrate a volume
/// from the local node to a remote peer's stord.
pub struct MigrationSender<B: StorageBackend> {
    backend: Arc<B>,
    volume_id: String,
    handle: String,
    block_size: u64,
    send_window: SendWindow,
}

impl<B: StorageBackend> MigrationSender<B> {
    pub fn new(backend: Arc<B>, volume_id: String, handle: String) -> Self {
        Self {
            backend,
            volume_id,
            handle,
            block_size: DEFAULT_BLOCK_SIZE,
            send_window: SendWindow::new(),
        }
    }

    pub fn with_block_size(mut self, block_size: u64) -> Self {
        self.block_size = block_size;
        self
    }

    /// Start a migration to a peer node at the given gRPC endpoint.
    ///
    /// This opens a bidirectional stream, sends InitMigration, waits for
    /// MigrationReady, then performs bulk copy followed by dirty sync rounds.
    pub async fn start_migration(mut self, endpoint: String) -> Result<(), tonic::Status> {
        let channel = Channel::from_shared(endpoint.clone())
            .map_err(|e| tonic::Status::internal(format!("invalid endpoint: {e}")))?
            .connect()
            .await
            .map_err(|e| tonic::Status::unavailable(format!("failed to connect to peer: {e}")))?;

        let mut client = StorageMigrationServiceClient::new(channel);

        // Set up outgoing message channel
        let (tx, rx) = mpsc::channel::<MigrationMessage>(256);
        let rx_stream = tokio_stream::wrappers::ReceiverStream::new(rx);

        // Start the bidirectional stream
        let response = client.stream_blocks(rx_stream).await?;
        let mut inbound = response.into_inner();

        // Get volume size
        let volume_size = self
            .backend
            .volume_size(&self.volume_id, &self.handle)
            .await
            .map_err(|e| tonic::Status::internal(format!("failed to get volume size: {e}")))?;

        // Send InitMigration
        let init_msg = MigrationMessage {
            payload: Some(migration_message::Payload::Init(InitMigration {
                volume_id: self.volume_id.clone(),
                size_bytes: volume_size,
                block_size: self.block_size as u32,
                format: "raw".to_string(),
                checksum_type: "crc32".to_string(),
            })),
        };
        tx.send(init_msg)
            .await
            .map_err(|_| tonic::Status::internal("failed to send InitMigration: channel closed"))?;

        info!(
            volume_id = %self.volume_id,
            volume_size,
            block_size = self.block_size,
            "sent InitMigration to peer"
        );

        // Wait for MigrationReady
        let ready_msg = inbound
            .message()
            .await?
            .ok_or_else(|| tonic::Status::internal("stream closed before MigrationReady"))?;

        match ready_msg.payload {
            Some(migration_message::Payload::Ready(ref ready)) => {
                info!(
                    dest_volume_id = %ready.dest_volume_id,
                    "peer is ready to receive migration"
                );
            }
            Some(migration_message::Payload::Error(ref err)) => {
                return Err(tonic::Status::internal(format!(
                    "peer returned error: {}",
                    err.message
                )));
            }
            _ => {
                return Err(tonic::Status::internal(
                    "unexpected message; expected MigrationReady",
                ));
            }
        }

        // Bulk copy phase
        info!(volume_id = %self.volume_id, "starting bulk copy phase");
        let total_chunks = self.bulk_copy(&tx, &mut inbound, volume_size).await?;
        info!(
            volume_id = %self.volume_id,
            total_chunks,
            "bulk copy phase complete"
        );

        // Send FinalSync (VM is paused at this point)
        let final_sync_msg = MigrationMessage {
            payload: Some(migration_message::Payload::FinalSync(FinalSync {
                vm_paused: true,
            })),
        };
        tx.send(final_sync_msg)
            .await
            .map_err(|_| tonic::Status::internal("failed to send FinalSync: channel closed"))?;

        // Send FinalizeComplete
        let finalize_msg = MigrationMessage {
            payload: Some(migration_message::Payload::FinalizeComplete(
                FinalizeComplete {
                    total_bytes: volume_size,
                    total_chunks: total_chunks as u64,
                    volume_checksum: Vec::new(),
                },
            )),
        };
        tx.send(finalize_msg).await.map_err(|_| {
            tonic::Status::internal("failed to send FinalizeComplete: channel closed")
        })?;

        // Wait for FinalizeAck
        let ack_msg = inbound
            .message()
            .await?
            .ok_or_else(|| tonic::Status::internal("stream closed before FinalizeAck"))?;

        match ack_msg.payload {
            Some(migration_message::Payload::FinalizeAck(ref ack)) => {
                if ack.verified {
                    info!(volume_id = %self.volume_id, "migration finalized successfully");
                } else {
                    error!(
                        volume_id = %self.volume_id,
                        error = %ack.error_message,
                        "migration finalization failed"
                    );
                    return Err(tonic::Status::internal(format!(
                        "finalization failed: {}",
                        ack.error_message
                    )));
                }
            }
            _ => {
                return Err(tonic::Status::internal(
                    "unexpected message; expected FinalizeAck",
                ));
            }
        }

        Ok(())
    }

    /// Perform the bulk copy phase: read all blocks and stream them to the receiver.
    async fn bulk_copy(
        &mut self,
        tx: &mpsc::Sender<MigrationMessage>,
        inbound: &mut tonic::Streaming<MigrationMessage>,
        volume_size: u64,
    ) -> Result<u32, tonic::Status> {
        let mut sequence_num: u32 = 0;
        let mut offset: u64 = 0;

        while offset < volume_size {
            // Wait if send window is full
            while !self.send_window.can_send() {
                self.wait_for_ack(inbound).await?;
            }

            let length = std::cmp::min(self.block_size, volume_size - offset);

            let data = self
                .backend
                .read_block(&self.volume_id, &self.handle, offset, length)
                .await
                .map_err(|e| {
                    tonic::Status::internal(format!("read_block failed at offset {offset}: {e}"))
                })?;

            let is_sparse = is_all_zeros(&data);
            let crc32 = if is_sparse { 0 } else { crc32fast::hash(&data) };

            let chunk_data = if is_sparse { Vec::new() } else { data };

            sequence_num += 1;
            let chunk_msg = MigrationMessage {
                payload: Some(migration_message::Payload::Chunk(BlockChunk {
                    offset,
                    data: chunk_data,
                    crc32,
                    is_sparse,
                    sequence_num,
                })),
            };

            tx.send(chunk_msg).await.map_err(|_| {
                tonic::Status::internal("failed to send BlockChunk: channel closed")
            })?;

            self.send_window.sent();

            // Check for acks non-blockingly if there might be some
            if self.send_window.should_request_ack() {
                self.try_receive_ack(inbound).await?;
            }

            offset += self.block_size;
        }

        // Drain remaining acks
        while !self.send_window.can_send() {
            self.wait_for_ack(inbound).await?;
        }

        Ok(sequence_num)
    }

    /// Perform a dirty sync round: read dirty bitmap, send only dirty blocks.
    #[allow(dead_code)]
    pub async fn dirty_sync_round(
        &mut self,
        tx: &mpsc::Sender<MigrationMessage>,
        inbound: &mut tonic::Streaming<MigrationMessage>,
        round_num: u32,
    ) -> Result<u64, tonic::Status> {
        let bitmap = self
            .backend
            .get_dirty_bitmap(&self.volume_id, &self.handle)
            .await
            .map_err(|e| tonic::Status::internal(format!("get_dirty_bitmap failed: {e}")))?;

        self.backend
            .clear_dirty_bitmap(&self.volume_id, &self.handle)
            .await
            .map_err(|e| tonic::Status::internal(format!("clear_dirty_bitmap failed: {e}")))?;

        let dirty_block_count = bitmap.iter().map(|b| b.count_ones() as u64).sum::<u64>();

        // Send RoundStart
        let round_start_msg = MigrationMessage {
            payload: Some(migration_message::Payload::RoundStart(RoundStart {
                round_num,
                dirty_block_count,
            })),
        };
        tx.send(round_start_msg)
            .await
            .map_err(|_| tonic::Status::internal("failed to send RoundStart: channel closed"))?;

        let mut blocks_sent: u64 = 0;
        let mut bytes_sent: u64 = 0;
        let mut sequence_num = self.send_window.ack_interval(); // continue from bulk

        // Iterate bitmap bits
        for (byte_idx, byte) in bitmap.iter().enumerate() {
            for bit_idx in 0..8u32 {
                if byte & (1 << bit_idx) == 0 {
                    continue;
                }

                let block_index = (byte_idx as u64) * 8 + bit_idx as u64;
                let offset = block_index * self.block_size;

                // Wait if send window is full
                while !self.send_window.can_send() {
                    self.wait_for_ack(inbound).await?;
                }

                let data = self
                    .backend
                    .read_block(&self.volume_id, &self.handle, offset, self.block_size)
                    .await
                    .map_err(|e| {
                        tonic::Status::internal(format!(
                            "read_block failed at offset {offset}: {e}"
                        ))
                    })?;

                let is_sparse = is_all_zeros(&data);
                let crc32 = if is_sparse { 0 } else { crc32fast::hash(&data) };
                let chunk_data = if is_sparse { Vec::new() } else { data.clone() };

                sequence_num += 1;
                let chunk_msg = MigrationMessage {
                    payload: Some(migration_message::Payload::Chunk(BlockChunk {
                        offset,
                        data: chunk_data,
                        crc32,
                        is_sparse,
                        sequence_num,
                    })),
                };

                tx.send(chunk_msg).await.map_err(|_| {
                    tonic::Status::internal("failed to send BlockChunk: channel closed")
                })?;

                self.send_window.sent();
                blocks_sent += 1;
                bytes_sent += data.len() as u64;

                if self.send_window.should_request_ack() {
                    self.try_receive_ack(inbound).await?;
                }
            }
        }

        // Send RoundComplete
        let round_complete_msg = MigrationMessage {
            payload: Some(migration_message::Payload::RoundComplete(RoundComplete {
                round_num,
                blocks_sent,
                bytes_sent,
            })),
        };
        tx.send(round_complete_msg)
            .await
            .map_err(|_| tonic::Status::internal("failed to send RoundComplete: channel closed"))?;

        debug!(
            round_num,
            blocks_sent, bytes_sent, "dirty sync round complete"
        );

        Ok(blocks_sent)
    }

    /// Block until an Ack is received from the inbound stream.
    async fn wait_for_ack(
        &mut self,
        inbound: &mut tonic::Streaming<MigrationMessage>,
    ) -> Result<(), tonic::Status> {
        let timeout = self.send_window.timeout();
        let msg = tokio::time::timeout(timeout, inbound.message())
            .await
            .map_err(|_| tonic::Status::deadline_exceeded("timed out waiting for Ack"))?
            .map_err(|e| tonic::Status::internal(format!("stream error: {e}")))?
            .ok_or_else(|| tonic::Status::internal("stream closed while waiting for Ack"))?;

        self.handle_inbound_message(msg)
    }

    /// Try to receive an Ack without blocking (non-blocking check).
    async fn try_receive_ack(
        &mut self,
        inbound: &mut tonic::Streaming<MigrationMessage>,
    ) -> Result<(), tonic::Status> {
        // Use a very short timeout to check if there's a pending message
        match tokio::time::timeout(std::time::Duration::from_millis(1), inbound.message()).await {
            Ok(Ok(Some(msg))) => self.handle_inbound_message(msg),
            Ok(Ok(None)) => Err(tonic::Status::internal("stream closed unexpectedly")),
            Ok(Err(e)) => Err(tonic::Status::internal(format!("stream error: {e}"))),
            Err(_) => Ok(()), // timeout = no message available, that's fine
        }
    }

    /// Process an inbound message (expected to be Ack or Backpressure).
    #[allow(clippy::result_large_err)]
    fn handle_inbound_message(&mut self, msg: MigrationMessage) -> Result<(), tonic::Status> {
        match msg.payload {
            Some(migration_message::Payload::Ack(ref ack)) => {
                if ack.status() == AckStatus::AckCrcMismatch {
                    warn!(
                        sequence = ack.last_sequence_num,
                        "receiver reported CRC mismatch"
                    );
                    return Err(tonic::Status::data_loss(
                        "CRC mismatch reported by receiver",
                    ));
                }
                if ack.status() == AckStatus::AckWriteError {
                    error!(
                        sequence = ack.last_sequence_num,
                        "receiver reported write error"
                    );
                    return Err(tonic::Status::internal("write error reported by receiver"));
                }
                self.send_window.acked(ack.last_sequence_num);
                Ok(())
            }
            Some(migration_message::Payload::Backpressure(ref bp)) => {
                debug!(
                    slow_down_factor = bp.slow_down_factor,
                    "backpressure received"
                );
                // For now just log; future: adjust send rate
                Ok(())
            }
            Some(migration_message::Payload::Error(ref err)) => Err(tonic::Status::internal(
                format!("migration error from peer: {}", err.message),
            )),
            _ => {
                warn!("unexpected message type during send phase");
                Ok(())
            }
        }
    }
}

/// Start a storage migration to a remote peer.
///
/// This is the top-level entry point called by the control plane / agent.
/// It creates a MigrationSender and drives the full migration lifecycle.
pub async fn start_migration_to_peer<B: StorageBackend>(
    endpoint: String,
    volume_id: String,
    handle: String,
    backend: Arc<B>,
) -> Result<(), tonic::Status> {
    let sender = MigrationSender::new(backend, volume_id, handle);
    sender.start_migration(endpoint).await
}

/// Check if a byte slice is all zeros (indicates a sparse block).
fn is_all_zeros(data: &[u8]) -> bool {
    // Process in 8-byte chunks for efficiency
    let (prefix, aligned, suffix) = unsafe { data.align_to::<u64>() };
    prefix.iter().all(|&b| b == 0)
        && aligned.iter().all(|&w| w == 0)
        && suffix.iter().all(|&b| b == 0)
}
