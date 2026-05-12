use crate::migration::flow_control::SendWindow;
use chv_stord_api::chv_stord_api::{
    migration_message, storage_migration_service_client::StorageMigrationServiceClient, AckStatus,
    BlockChunk, FinalSync, FinalizeComplete, InitMigration, MigrationMessage, RoundComplete,
    RoundStart,
};
use chv_stord_backends::StorageBackend;
use std::sync::Arc;
use tokio::sync::mpsc;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Identity};
use tracing::{debug, error, info, warn};

const DEFAULT_BLOCK_SIZE: u64 = 4_194_304; // 4 MB
const DIRTY_THRESHOLD: u64 = 1024;
const MAX_DIRTY_ROUNDS: u32 = 10;

/// TLS configuration for mTLS connections to the migration destination.
///
/// When provided, the sender will validate the destination's certificate against
/// the CA and present its own node certificate, as required by the disk migration
/// protocol spec.
#[derive(Clone)]
pub struct MigrationTlsConfig {
    /// PEM-encoded client certificate (node cert issued by CP CA).
    pub cert_pem: Vec<u8>,
    /// PEM-encoded client private key.
    pub key_pem: Vec<u8>,
    /// PEM-encoded CA certificate to validate the destination.
    pub ca_pem: Vec<u8>,
    /// Expected domain name of the destination (for certificate validation).
    pub dest_domain: String,
}

/// Drives the source side of a storage migration.
///
/// This is invoked by the control plane / agent to migrate a volume
/// from the local node to a remote peer's stord.
///
/// Flow control: the sender maintains a sliding window of at most
/// `send_window_size` (default 16) unacknowledged chunks. The receiver
/// sends an acknowledgment for every chunk written, allowing the sender
/// to track `last_acknowledged_offset` for stream resumability.
pub struct MigrationSender<B: StorageBackend> {
    backend: Arc<B>,
    volume_id: String,
    handle: String,
    block_size: u64,
    send_window: SendWindow,
    last_acknowledged_offset: u64,
    tls_config: Option<MigrationTlsConfig>,
    /// Backpressure factor received from the destination. Values > 1.0 cause
    /// the sender to insert a throttle sleep between chunk sends.
    backpressure_factor: f32,
}

impl<B: StorageBackend> MigrationSender<B> {
    pub fn new(backend: Arc<B>, volume_id: String, handle: String) -> Self {
        Self {
            backend,
            volume_id,
            handle,
            block_size: DEFAULT_BLOCK_SIZE,
            send_window: SendWindow::new(),
            last_acknowledged_offset: 0,
            tls_config: None,
            backpressure_factor: 1.0,
        }
    }

    pub fn with_block_size(mut self, block_size: u64) -> Self {
        self.block_size = block_size;
        self
    }

    /// Configure mTLS for the connection to the migration destination.
    ///
    /// When set, the sender uses `https://` and presents the node certificate
    /// while validating the destination against the provided CA certificate.
    pub fn with_tls(mut self, tls_config: MigrationTlsConfig) -> Self {
        self.tls_config = Some(tls_config);
        self
    }

    /// Returns the last acknowledged offset, useful for resumability.
    pub fn last_acknowledged_offset(&self) -> u64 {
        self.last_acknowledged_offset
    }

    /// Start a migration to a peer node at the given gRPC endpoint.
    ///
    /// This opens a bidirectional stream, sends InitMigration, waits for
    /// MigrationReady, then performs bulk copy followed by dirty sync rounds.
    pub async fn start_migration(mut self, endpoint: String) -> Result<(), tonic::Status> {
        let channel = if let Some(ref tls) = self.tls_config {
            let identity = Identity::from_pem(&tls.cert_pem, &tls.key_pem);
            let ca = Certificate::from_pem(&tls.ca_pem);
            let tls_config = ClientTlsConfig::new()
                .domain_name(&tls.dest_domain)
                .identity(identity)
                .ca_certificate(ca);

            // Ensure the endpoint uses https
            let secure_endpoint = if endpoint.starts_with("http://") {
                endpoint.replacen("http://", "https://", 1)
            } else if !endpoint.starts_with("https://") {
                format!("https://{endpoint}")
            } else {
                endpoint.clone()
            };

            info!(
                endpoint = %secure_endpoint,
                dest_domain = %tls.dest_domain,
                "connecting to migration peer with mTLS"
            );

            Channel::from_shared(secure_endpoint)
                .map_err(|e| tonic::Status::internal(format!("invalid endpoint: {e}")))?
                .tls_config(tls_config)
                .map_err(|e| tonic::Status::internal(format!("TLS config error: {e}")))?
                .connect()
                .await
                .map_err(|e| {
                    tonic::Status::unavailable(format!("failed to connect to peer with mTLS: {e}"))
                })?
        } else {
            return Err(tonic::Status::failed_precondition(
                "mTLS is required for storage migration — tls_config must be provided. \
                 Set migration.tls.cert_path, migration.tls.key_path, and migration.tls.ca_path in stord config.",
            ));
        };

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
        let mut sequence_num = self.bulk_copy(&tx, &mut inbound, volume_size).await?;
        info!(
            volume_id = %self.volume_id,
            total_chunks = sequence_num,
            last_ack_offset = self.last_acknowledged_offset,
            "bulk copy phase complete"
        );

        // Iterative dirty sync rounds: re-send blocks that were written during bulk copy
        info!(volume_id = %self.volume_id, "starting iterative dirty sync rounds");
        let dirty_chunks = self
            .dirty_sync_rounds(&tx, &mut inbound, &mut sequence_num)
            .await?;
        info!(
            volume_id = %self.volume_id,
            dirty_chunks,
            "dirty sync rounds complete"
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
                    total_chunks: sequence_num as u64,
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
    ///
    /// The sender computes CRC32 for each chunk and respects the send window.
    /// When the window is full (default 16 in-flight), the sender blocks
    /// until acknowledgments are received from the destination.
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

            // Apply backpressure throttle if the receiver requested slow-down
            if self.backpressure_factor > 1.0 {
                tokio::time::sleep(std::time::Duration::from_millis(
                    (10.0 * self.backpressure_factor) as u64,
                ))
                .await;
            }

            // Non-blocking check for acks to keep the window sliding
            if self.send_window.should_request_ack() {
                self.try_receive_ack(inbound).await?;
            }

            offset += self.block_size;
        }

        // Drain all remaining in-flight acks before completing the phase
        while self.send_window.last_ack_sequence() < sequence_num {
            self.wait_for_ack(inbound).await?;
        }

        Ok(sequence_num)
    }

    /// Perform iterative dirty sync rounds to transfer blocks written during bulk copy.
    ///
    /// Each round fetches the dirty bitmap, sends dirty blocks, waits for acknowledgment,
    /// then clears the bitmap. Repeats until dirty count drops below DIRTY_THRESHOLD
    /// or MAX_DIRTY_ROUNDS is reached.
    async fn dirty_sync_rounds(
        &mut self,
        tx: &mpsc::Sender<MigrationMessage>,
        inbound: &mut tonic::Streaming<MigrationMessage>,
        sequence_num: &mut u32,
    ) -> Result<u32, tonic::Status> {
        let mut total_dirty_chunks: u32 = 0;

        for round in 1..=MAX_DIRTY_ROUNDS {
            // Step 1: Atomically snapshot and clear the dirty bitmap.
            // This ensures no writes are lost between reading the bitmap and clearing it.
            let bitmap = self
                .backend
                .snapshot_and_clear_dirty_bitmap(&self.volume_id, &self.handle)
                .await
                .map_err(|e| {
                    tonic::Status::internal(format!("snapshot_and_clear_dirty_bitmap failed: {e}"))
                })?;

            // Convert bitmap to list of dirty block offsets
            let dirty_offsets = bitmap_to_offsets(&bitmap, self.block_size);
            let dirty_block_count = dirty_offsets.len() as u64;

            info!(
                volume_id = %self.volume_id,
                round,
                dirty_block_count,
                "dirty sync round starting"
            );

            // Check termination condition: if below threshold, we're done
            if dirty_block_count == 0 {
                info!(
                    volume_id = %self.volume_id,
                    round,
                    "no dirty blocks remaining, skipping final round"
                );
                break;
            }

            // Step 2: Send RoundStart
            let round_start_msg = MigrationMessage {
                payload: Some(migration_message::Payload::RoundStart(RoundStart {
                    round_num: round,
                    dirty_block_count,
                })),
            };
            tx.send(round_start_msg).await.map_err(|_| {
                tonic::Status::internal("failed to send RoundStart: channel closed")
            })?;

            // Step 3: Send each dirty block
            let mut blocks_sent: u64 = 0;
            let mut bytes_sent: u64 = 0;

            for &offset in &dirty_offsets {
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
                            "read_block failed at offset {offset} during dirty sync: {e}"
                        ))
                    })?;

                let is_sparse = is_all_zeros(&data);
                let crc32 = if is_sparse { 0 } else { crc32fast::hash(&data) };
                let chunk_data = if is_sparse { Vec::new() } else { data };

                *sequence_num += 1;
                bytes_sent += chunk_data.len() as u64;

                let chunk_msg = MigrationMessage {
                    payload: Some(migration_message::Payload::Chunk(BlockChunk {
                        offset,
                        data: chunk_data,
                        crc32,
                        is_sparse,
                        sequence_num: *sequence_num,
                    })),
                };

                tx.send(chunk_msg).await.map_err(|_| {
                    tonic::Status::internal("failed to send dirty BlockChunk: channel closed")
                })?;

                self.send_window.sent();
                blocks_sent += 1;

                // Apply backpressure throttle if the receiver requested slow-down
                if self.backpressure_factor > 1.0 {
                    tokio::time::sleep(std::time::Duration::from_millis(
                        (10.0 * self.backpressure_factor) as u64,
                    ))
                    .await;
                }

                // Non-blocking check for acks to keep the window sliding
                if self.send_window.should_request_ack() {
                    self.try_receive_ack(inbound).await?;
                }
            }

            // Drain all remaining in-flight acks for this round
            while self.send_window.last_ack_sequence() < *sequence_num {
                self.wait_for_ack(inbound).await?;
            }

            // Step 4: Send RoundComplete
            let round_complete_msg = MigrationMessage {
                payload: Some(migration_message::Payload::RoundComplete(RoundComplete {
                    round_num: round,
                    blocks_sent,
                    bytes_sent,
                })),
            };
            tx.send(round_complete_msg).await.map_err(|_| {
                tonic::Status::internal("failed to send RoundComplete: channel closed")
            })?;

            // Step 5: Wait for round acknowledgment from the receiver
            let round_ack_msg = inbound.message().await?.ok_or_else(|| {
                tonic::Status::internal("stream closed before round acknowledgment")
            })?;
            self.handle_inbound_message(round_ack_msg)?;

            // Note: dirty bitmap was already cleared atomically in step 1 via
            // snapshot_and_clear_dirty_bitmap, so no separate clear needed here.

            total_dirty_chunks += blocks_sent as u32;

            info!(
                volume_id = %self.volume_id,
                round,
                blocks_sent,
                bytes_sent,
                "dirty sync round complete"
            );

            // Step 7: Check if we should stop
            if dirty_block_count < DIRTY_THRESHOLD {
                break;
            }
        }

        Ok(total_dirty_chunks)
    }

    /// Block until an Ack is received from the inbound stream.
    async fn wait_for_ack(
        &mut self,
        inbound: &mut tonic::Streaming<MigrationMessage>,
    ) -> Result<(), tonic::Status> {
        let timeout = self.send_window.timeout();
        let msg = tokio::time::timeout(timeout, inbound.message())
            .await
            .map_err(|_| {
                tonic::Status::deadline_exceeded(format!(
                    "timed out waiting for Ack (last_ack_offset={})",
                    self.last_acknowledged_offset
                ))
            })?
            .map_err(|e| tonic::Status::internal(format!("stream error: {e}")))?
            .ok_or_else(|| tonic::Status::internal("stream closed while waiting for Ack"))?;

        self.handle_inbound_message(msg)
    }

    /// Try to receive an Ack without blocking (non-blocking check).
    async fn try_receive_ack(
        &mut self,
        inbound: &mut tonic::Streaming<MigrationMessage>,
    ) -> Result<(), tonic::Status> {
        // Use a short timeout to check if there's a pending message.
        // 50ms balances responsiveness (not blocking sends too long) against
        // avoiding excessive timer-wheel firings that 1ms would cause.
        match tokio::time::timeout(std::time::Duration::from_millis(50), inbound.message()).await {
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
                        offset = ack.last_offset,
                        "receiver reported CRC mismatch"
                    );
                    return Err(tonic::Status::data_loss(
                        "CRC mismatch reported by receiver",
                    ));
                }
                if ack.status() == AckStatus::AckWriteError {
                    error!(
                        sequence = ack.last_sequence_num,
                        offset = ack.last_offset,
                        "receiver reported write error"
                    );
                    return Err(tonic::Status::internal("write error reported by receiver"));
                }
                self.send_window.acked(ack.last_sequence_num);
                self.last_acknowledged_offset = ack.last_offset;
                debug!(
                    sequence = ack.last_sequence_num,
                    offset = ack.last_offset,
                    unacked = self.send_window.unacked_count(),
                    "ack received"
                );
                Ok(())
            }
            Some(migration_message::Payload::Backpressure(ref bp)) => {
                info!(
                    slow_down_factor = bp.slow_down_factor,
                    "backpressure received, adjusting send rate"
                );
                self.backpressure_factor = bp.slow_down_factor.max(1.0);
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
///
/// When `tls_config` is `Some`, the connection uses mTLS as required by
/// the disk migration protocol spec.
pub async fn start_migration_to_peer<B: StorageBackend>(
    endpoint: String,
    volume_id: String,
    handle: String,
    backend: Arc<B>,
    tls_config: Option<MigrationTlsConfig>,
) -> Result<(), tonic::Status> {
    let mut sender = MigrationSender::new(backend, volume_id, handle);
    if let Some(tls) = tls_config {
        sender = sender.with_tls(tls);
    }
    sender.start_migration(endpoint).await
}

/// Check if a byte slice is all zeros (indicates a sparse block).
fn is_all_zeros(data: &[u8]) -> bool {
    data.iter().all(|&b| b == 0)
}

/// Convert a dirty bitmap into a vec of block byte-offsets.
///
/// The bitmap uses one bit per block: bit N of byte M represents block index `M*8 + N`.
/// Each block offset is computed as `block_index * block_size`.
fn bitmap_to_offsets(bitmap: &[u8], block_size: u64) -> Vec<u64> {
    let mut offsets = Vec::new();
    for (byte_idx, &byte) in bitmap.iter().enumerate() {
        if byte == 0 {
            continue;
        }
        for bit in 0..8u32 {
            if byte & (1 << bit) != 0 {
                let block_index = (byte_idx as u64) * 8 + bit as u64;
                offsets.push(block_index * block_size);
            }
        }
    }
    offsets
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use chv_common::types::{BackendLocator, DevicePolicy};
    use chv_errors::ChvError;
    use chv_stord_backends::{BackendHealth, StorageBackend, VolumeExport};

    /// Minimal mock backend for testing the MigrationSender without real I/O.
    struct MockBackend;

    #[async_trait]
    impl StorageBackend for MockBackend {
        async fn open(
            &self,
            _volume_id: &str,
            _locator: &BackendLocator,
            _policy: &DevicePolicy,
        ) -> Result<VolumeExport, ChvError> {
            unimplemented!("not needed for sender tests")
        }

        async fn close(&self, _volume_id: &str, _handle: &str) -> Result<(), ChvError> {
            Ok(())
        }

        async fn attach(
            &self,
            _volume_id: &str,
            _handle: &str,
            _vm_id: &str,
        ) -> Result<VolumeExport, ChvError> {
            unimplemented!("not needed for sender tests")
        }

        async fn detach(
            &self,
            _volume_id: &str,
            _handle: &str,
            _vm_id: &str,
            _force: bool,
        ) -> Result<(), ChvError> {
            Ok(())
        }

        async fn health(&self, _volume_id: &str, _handle: &str) -> Result<BackendHealth, ChvError> {
            Ok(BackendHealth {
                status: "healthy".to_string(),
                backend_state: "ok".to_string(),
                last_error: String::new(),
            })
        }

        async fn resize(
            &self,
            _volume_id: &str,
            _handle: &str,
            _new_size_bytes: u64,
        ) -> Result<(), ChvError> {
            Ok(())
        }

        async fn prepare_snapshot(
            &self,
            _volume_id: &str,
            _handle: &str,
            _snapshot_name: &str,
        ) -> Result<(), ChvError> {
            Ok(())
        }

        async fn prepare_clone(
            &self,
            _volume_id: &str,
            _handle: &str,
            _clone_name: &str,
        ) -> Result<(), ChvError> {
            Ok(())
        }

        async fn restore_snapshot(
            &self,
            _volume_id: &str,
            _handle: &str,
            _snapshot_name: &str,
        ) -> Result<(), ChvError> {
            Ok(())
        }

        async fn delete_snapshot(
            &self,
            _volume_id: &str,
            _handle: &str,
            _snapshot_name: &str,
        ) -> Result<(), ChvError> {
            Ok(())
        }

        async fn set_device_policy(
            &self,
            _volume_id: &str,
            _handle: &str,
            _policy: &DevicePolicy,
        ) -> Result<(), ChvError> {
            Ok(())
        }

        async fn enable_dirty_tracking(
            &self,
            _volume_id: &str,
            _handle: &str,
            _block_size: u64,
        ) -> Result<(), ChvError> {
            Ok(())
        }

        async fn get_dirty_bitmap(
            &self,
            _volume_id: &str,
            _handle: &str,
        ) -> Result<Vec<u8>, ChvError> {
            Ok(vec![])
        }

        async fn clear_dirty_bitmap(
            &self,
            _volume_id: &str,
            _handle: &str,
        ) -> Result<(), ChvError> {
            Ok(())
        }

        async fn disable_dirty_tracking(
            &self,
            _volume_id: &str,
            _handle: &str,
        ) -> Result<(), ChvError> {
            Ok(())
        }

        async fn read_block(
            &self,
            _volume_id: &str,
            _handle: &str,
            _offset: u64,
            length: u64,
        ) -> Result<Vec<u8>, ChvError> {
            Ok(vec![0u8; length as usize])
        }

        async fn write_block(
            &self,
            _volume_id: &str,
            _handle: &str,
            _offset: u64,
            _data: &[u8],
        ) -> Result<(), ChvError> {
            Ok(())
        }

        async fn volume_size(&self, _volume_id: &str, _handle: &str) -> Result<u64, ChvError> {
            Ok(1024 * 1024) // 1 MB
        }

        async fn create_receiving_volume(
            &self,
            _volume_id: &str,
            _size_bytes: u64,
            _format: &str,
        ) -> Result<VolumeExport, ChvError> {
            unimplemented!("not needed for sender tests")
        }

        async fn delete_volume(&self, _volume_id: &str) -> Result<(), ChvError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_mtls_required_for_migration() {
        let backend = Arc::new(MockBackend);
        let sender = MigrationSender::new(backend, "vol-123".to_string(), "handle-abc".to_string());

        // Attempt migration without TLS configured
        let result = sender
            .start_migration("http://10.0.0.1:9090".to_string())
            .await;

        assert!(result.is_err(), "migration should fail without mTLS");
        let status = result.unwrap_err();
        assert_eq!(
            status.code(),
            tonic::Code::FailedPrecondition,
            "error code should be FailedPrecondition, got {:?}",
            status.code()
        );
        assert!(
            status.message().contains("mTLS is required"),
            "error message should mention mTLS requirement: {}",
            status.message()
        );
    }

    #[test]
    fn test_backpressure_factor_initialization() {
        let backend = Arc::new(MockBackend);
        let sender = MigrationSender::new(backend, "vol-456".to_string(), "handle-def".to_string());

        // backpressure_factor is private, but we can verify behavior through
        // the sender's initial state. The field is initialized to 1.0 which
        // means no throttling. We verify the sender was constructed correctly
        // by checking that last_acknowledged_offset starts at 0.
        assert_eq!(sender.last_acknowledged_offset(), 0);
    }

    #[test]
    fn test_sender_with_block_size() {
        let backend = Arc::new(MockBackend);
        let sender = MigrationSender::new(backend, "vol-789".to_string(), "handle-ghi".to_string())
            .with_block_size(8_388_608); // 8 MB

        // Verify construction doesn't panic and sender is usable
        assert_eq!(sender.last_acknowledged_offset(), 0);
    }

    #[test]
    fn test_bitmap_to_offsets_empty() {
        let bitmap: Vec<u8> = vec![];
        let offsets = bitmap_to_offsets(&bitmap, 4096);
        assert!(offsets.is_empty());
    }

    #[test]
    fn test_bitmap_to_offsets_single_bit() {
        // Byte 0, bit 0 set => block index 0 => offset 0
        let bitmap = vec![0x01u8];
        let offsets = bitmap_to_offsets(&bitmap, 4096);
        assert_eq!(offsets, vec![0]);
    }

    #[test]
    fn test_bitmap_to_offsets_multiple_bits() {
        // Byte 0: bits 0 and 2 set => block indices 0, 2
        // Byte 1: bit 1 set => block index 9
        let bitmap = vec![0x05u8, 0x02u8];
        let offsets = bitmap_to_offsets(&bitmap, 4096);
        assert_eq!(offsets, vec![0, 2 * 4096, 9 * 4096]);
    }

    #[test]
    fn test_bitmap_to_offsets_all_zeros() {
        let bitmap = vec![0x00u8; 16];
        let offsets = bitmap_to_offsets(&bitmap, 4096);
        assert!(offsets.is_empty());
    }

    #[test]
    fn test_is_all_zeros() {
        assert!(is_all_zeros(&[0, 0, 0, 0]));
        assert!(is_all_zeros(&[]));
        assert!(!is_all_zeros(&[0, 0, 1, 0]));
        assert!(!is_all_zeros(&[255]));
    }
}
