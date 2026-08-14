use chv_stord_api::chv_stord_api::{
    migration_message, Ack, AckStatus, FinalizeAck, MigrationMessage,
};
use chv_stord_backends::StorageBackend;
use std::sync::Arc;
use tokio::sync::mpsc;
use tonic::Streaming;
use tracing::{debug, error, info, warn};

const DEFAULT_ACK_INTERVAL: u32 = 64;

/// Drives the destination side of a storage migration.
///
/// The receiver is created when an incoming stream starts with InitMigration.
/// It creates a receiving volume, acknowledges readiness, then processes
/// incoming BlockChunk messages, writing data to the volume.
pub struct MigrationReceiver<B: StorageBackend> {
    backend: Arc<B>,
    volume_id: String,
    handle: String,
    size_bytes: u64,
    ack_interval: u32,
    blocks_received: u32,
    blocks_since_last_ack: u32,
    last_sequence_num: u32,
}

impl<B: StorageBackend> MigrationReceiver<B> {
    /// Create a new receiver (called after InitMigration is parsed).
    pub fn new(backend: Arc<B>, volume_id: String, handle: String, size_bytes: u64) -> Self {
        Self {
            backend,
            volume_id,
            handle,
            size_bytes,
            ack_interval: DEFAULT_ACK_INTERVAL,
            blocks_received: 0,
            blocks_since_last_ack: 0,
            last_sequence_num: 0,
        }
    }

    /// Run the receiver loop: process incoming messages and write blocks.
    ///
    /// This is spawned as a task and communicates back via the `tx` channel.
    /// However the stream ends, the receiving volume is closed exactly once:
    /// `create_receiving_volume` acquired a backend reference (an iSCSI
    /// session ref) that must be released to keep the backend refcount
    /// balanced.
    pub async fn run(
        mut self,
        mut inbound: Streaming<MigrationMessage>,
        tx: mpsc::Sender<MigrationMessage>,
    ) -> Result<(), tonic::Status> {
        let result = self.run_inner(&mut inbound, &tx).await;
        if let Err(e) = self.backend.close(&self.volume_id, &self.handle).await {
            warn!(
                volume_id = %self.volume_id,
                error = %e,
                "failed to release receiving volume after migration stream ended"
            );
        }
        result
    }

    async fn run_inner(
        &mut self,
        inbound: &mut Streaming<MigrationMessage>,
        tx: &mpsc::Sender<MigrationMessage>,
    ) -> Result<(), tonic::Status> {
        loop {
            let msg = match inbound.message().await? {
                Some(m) => m,
                None => {
                    info!(volume_id = %self.volume_id, "inbound stream closed");
                    break;
                }
            };

            match msg.payload {
                Some(migration_message::Payload::Chunk(chunk)) => {
                    self.handle_chunk(&chunk, tx).await?;
                }
                Some(migration_message::Payload::RoundStart(ref rs)) => {
                    debug!(
                        round_num = rs.round_num,
                        dirty_blocks = rs.dirty_block_count,
                        "dirty sync round starting"
                    );
                }
                Some(migration_message::Payload::RoundComplete(ref rc)) => {
                    debug!(
                        round_num = rc.round_num,
                        blocks_sent = rc.blocks_sent,
                        bytes_sent = rc.bytes_sent,
                        "dirty sync round complete"
                    );
                }
                Some(migration_message::Payload::FinalSync(ref fs)) => {
                    info!(
                        volume_id = %self.volume_id,
                        vm_paused = fs.vm_paused,
                        "received FinalSync"
                    );
                }
                Some(migration_message::Payload::FinalizeComplete(ref fc)) => {
                    info!(
                        volume_id = %self.volume_id,
                        total_bytes = fc.total_bytes,
                        total_chunks = fc.total_chunks,
                        "received FinalizeComplete"
                    );

                    // Send FinalizeAck
                    let ack_msg = MigrationMessage {
                        payload: Some(migration_message::Payload::FinalizeAck(FinalizeAck {
                            verified: true,
                            error_message: String::new(),
                        })),
                    };
                    tx.send(ack_msg).await.map_err(|_| {
                        tonic::Status::internal("failed to send FinalizeAck: channel closed")
                    })?;

                    info!(
                        volume_id = %self.volume_id,
                        blocks_received = self.blocks_received,
                        "migration receive complete"
                    );
                    break;
                }
                Some(migration_message::Payload::Error(ref err)) => {
                    error!(
                        volume_id = %self.volume_id,
                        code = ?err.code,
                        message = %err.message,
                        "received MigrationError from sender"
                    );
                    return Err(tonic::Status::internal(format!(
                        "sender reported error: {}",
                        err.message
                    )));
                }
                _ => {
                    warn!(volume_id = %self.volume_id, "unexpected message in receiver loop");
                }
            }
        }

        Ok(())
    }

    /// Process a single BlockChunk: verify CRC, write to volume, send Ack if needed.
    async fn handle_chunk(
        &mut self,
        chunk: &chv_stord_api::chv_stord_api::BlockChunk,
        tx: &mpsc::Sender<MigrationMessage>,
    ) -> Result<(), tonic::Status> {
        self.last_sequence_num = chunk.sequence_num;

        // Reject chunks that would write past the end of the receiving volume.
        let chunk_end = chunk
            .offset
            .checked_add(chunk.data.len() as u64)
            .ok_or_else(|| {
                tonic::Status::invalid_argument(format!(
                    "BlockChunk at offset {} overflows u64",
                    chunk.offset
                ))
            })?;
        if chunk_end > self.size_bytes {
            return Err(tonic::Status::invalid_argument(format!(
                "BlockChunk at offset {} with {} bytes exceeds volume size {}",
                chunk.offset,
                chunk.data.len(),
                self.size_bytes
            )));
        }

        // Verify CRC32 (skip for sparse blocks)
        if !chunk.is_sparse {
            let computed_crc = crc32fast::hash(&chunk.data);
            if computed_crc != chunk.crc32 {
                warn!(
                    offset = chunk.offset,
                    sequence = chunk.sequence_num,
                    expected_crc = chunk.crc32,
                    computed_crc,
                    "CRC mismatch"
                );
                // Send Ack with mismatch status
                let ack_msg = MigrationMessage {
                    payload: Some(migration_message::Payload::Ack(Ack {
                        last_offset: chunk.offset,
                        last_sequence_num: chunk.sequence_num,
                        status: AckStatus::AckCrcMismatch.into(),
                    })),
                };
                tx.send(ack_msg)
                    .await
                    .map_err(|_| tonic::Status::internal("failed to send Ack: channel closed"))?;
                return Err(tonic::Status::data_loss(format!(
                    "CRC mismatch at offset {}",
                    chunk.offset
                )));
            }
        }

        // Write the block
        if chunk.is_sparse {
            // For sparse blocks, write zeros. If the volume is sparse-allocated,
            // the backend may optimize this away.
            let zeros = vec![0u8; chunk.data.len().max(1)];
            // Only write if there's a meaningful block size. Sparse blocks with
            // empty data means the backend should already have zeros.
            if !chunk.data.is_empty() {
                self.backend
                    .write_block(&self.volume_id, &self.handle, chunk.offset, &zeros)
                    .await
                    .map_err(|e| {
                        tonic::Status::internal(format!(
                            "write_block failed at offset {}: {e}",
                            chunk.offset
                        ))
                    })?;
            }
            // If data is empty and is_sparse, skip write (volume was zero-initialized)
        } else {
            self.backend
                .write_block(&self.volume_id, &self.handle, chunk.offset, &chunk.data)
                .await
                .map_err(|e| {
                    tonic::Status::internal(format!(
                        "write_block failed at offset {}: {e}",
                        chunk.offset
                    ))
                })?;
        }

        self.blocks_received += 1;
        self.blocks_since_last_ack += 1;

        // Send Ack every ack_interval blocks
        if self.blocks_since_last_ack >= self.ack_interval {
            let ack_msg = MigrationMessage {
                payload: Some(migration_message::Payload::Ack(Ack {
                    last_offset: chunk.offset,
                    last_sequence_num: chunk.sequence_num,
                    status: AckStatus::AckOk.into(),
                })),
            };
            tx.send(ack_msg)
                .await
                .map_err(|_| tonic::Status::internal("failed to send Ack: channel closed"))?;
            self.blocks_since_last_ack = 0;
        }

        Ok(())
    }
}
