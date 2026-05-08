# Disk Migration Protocol Spec (stord-to-stord)

## Purpose
Defines the block-level data transfer protocol between two stord instances on different nodes during VM live migration.

## Participants
- **Source stord**: reads volume blocks, tracks dirty blocks, streams data
- **Destination stord**: creates target volume, receives blocks, acknowledges
- **Control plane**: coordinates initiation and monitors progress (does not participate in data transfer)

## Transport
- gRPC bidirectional streaming over mTLS
- New protobuf service: `StorageMigrationService`
- Port: same as stord's inter-node gRPC port (control plane provides endpoint to source)
- Authentication: mTLS using agent node certificates (already provisioned via enrollment)

## Protocol Phases

### Phase 1: Handshake

```
Source                          Destination
  │                                 │
  │  InitMigration(volume_meta)     │
  │────────────────────────────────►│
  │                                 │
  │  MigrationReady(dest_volume_id) │
  │◄────────────────────────────────│
```

- Source sends: volume_id, size_bytes, block_size, format (raw/qcow2), checksum_type
- Dest creates: empty volume of same size and format
- Dest responds: ready with dest_volume_id, or error (insufficient space, etc.)

### Phase 2: Bulk Copy

```
Source                          Destination
  │                                 │
  │  BlockChunk(offset, data, crc)  │
  │────────────────────────────────►│  (sequential, full volume)
  │  BlockChunk(offset, data, crc)  │
  │────────────────────────────────►│
  │        ...                      │
  │                                 │
  │  Ack(last_offset, status)       │
  │◄────────────────────────────────│  (periodic acks for flow control)
  │                                 │
  │  BulkCopyComplete               │
  │────────────────────────────────►│
  │                                 │
  │  BulkCopyAck                    │
  │◄────────────────────────────────│
```

- Source reads volume sequentially, sends blocks (offset, data bytes, CRC32)
- Block size: configurable, default 4MB (4,194,304 bytes)
- Dest writes blocks at matching offsets
- Flow control: dest sends Ack every N blocks (default: every 64 blocks = 256MB)
- Source pauses if Ack not received within flow_control_timeout (default: 30s)
- Zero blocks (all zeros) MAY be sent as sparse indicator (offset + length, no data payload)

### Phase 3: Dirty Sync (iterative)

```
Source                          Destination
  │                                 │
  │  DirtySyncRound(round_num,      │
  │    bitmap_summary)              │
  │────────────────────────────────►│
  │                                 │
  │  BlockChunk(offset, data, crc)  │  (only dirty blocks)
  │────────────────────────────────►│
  │        ...                      │
  │                                 │
  │  RoundComplete(round_num,       │
  │    dirty_count)                 │
  │────────────────────────────────►│
  │                                 │
  │  RoundAck(round_num)            │
  │◄────────────────────────────────│
```

- After bulk copy, source reads dirty bitmap (blocks written since tracking started)
- Sends only dirty blocks in each round
- After sending: clears bitmap, starts next round if dirty_count > threshold
- CP monitors dirty_count per round to detect convergence
- Rounds continue until: dirty_count < threshold OR max_rounds reached

### Phase 4: Finalize

```
Source                          Destination
  │                                 │
  │  FinalSync(is_vm_paused=true)   │
  │────────────────────────────────►│
  │                                 │
  │  BlockChunk(offset, data, crc)  │  (final dirty blocks)
  │────────────────────────────────►│
  │        ...                      │
  │                                 │
  │  FinalizeComplete(total_bytes,  │
  │    total_blocks, checksum)      │
  │────────────────────────────────►│
  │                                 │
  │  FinalizeAck(verified=true)     │
  │◄────────────────────────────────│
```

- VM is paused (by CH, coordinated by CP)
- Source flushes ALL remaining dirty blocks
- Source sends FinalizeComplete with total integrity info
- Dest verifies and acknowledges
- After FinalizeAck: CP proceeds with CH memory migration finalization

## Dirty Block Tracking

**Mechanism:** Device-mapper snapshot or direct I/O interception at stord level

**Implementation options (source stord chooses based on backend):**
1. **File backend**: COW snapshot of volume file, track modified blocks via file extent queries
2. **LVM backend**: dm-snapshot or dm-thin with external bitmap
3. **Generic**: userspace bitmap updated on every write() to volume (simplest, works for all backends)

**Bitmap spec:**
- 1 bit per block (at 4MB block_size: 256 bits per GB, 32KB bitmap for 1TB volume)
- Bitmap is local to source stord (not transferred)
- Cleared atomically after each sync round
- Must be consistent: a block marked dirty MUST contain data written after the last clear

## Block Chunk Message

```protobuf
message BlockChunk {
  uint64 offset = 1;          // byte offset in volume
  bytes data = 2;             // block data (up to block_size bytes)
  uint32 crc32 = 3;           // CRC32 of data field
  bool is_sparse = 4;         // if true, data is empty, block is all zeros
  uint32 sequence_num = 5;    // monotonically increasing per stream
}
```

## Flow Control

- Dest sends Ack every `ack_interval` blocks (default: 64)
- Source maintains send window (max unacked chunks, default: 128)
- If window full: source blocks until Ack received
- If no Ack within 30s: source assumes dest failed, reports error to CP
- Dest can send `Backpressure(slow_down_factor)` to dynamically reduce source send rate

## Integrity

- Every BlockChunk includes CRC32 of the data field
- Dest verifies CRC32 on receipt; mismatch → NACK that chunk → source retransmits
- FinalizeComplete includes: total_bytes_transferred, total_chunks, full volume checksum (optional, if < 10GB)
- For volumes > 10GB: per-round checksums instead of full volume checksum

## Resumability

| Phase | Resumable? | Strategy |
|---|---|---|
| Bulk copy | Yes | Resume from last Ack'd offset (sequence_num) |
| Dirty sync | Partial | Current round restarts (bitmap re-read); previous rounds not repeated |
| Finalize | No | Must restart finalize from current dirty bitmap |

## Error Handling

| Error | Source Action | Dest Action | CP Action |
|---|---|---|---|
| Stream disconnect | Pause, report to CP | Hold partial volume | Retry or abort |
| CRC mismatch | Retransmit chunk | NACK chunk | Monitor retransmit count |
| Dest disk full | N/A | Report error to CP | Abort migration |
| Source volume I/O error | Report to CP | Hold partial volume | Abort migration |
| Timeout (no Ack) | Report to CP | N/A | Abort migration |

## Proto Service Definition

```protobuf
service StorageMigrationService {
  // Bidirectional streaming for block transfer
  rpc StreamBlocks(stream MigrationMessage) returns (stream MigrationMessage);
}

message MigrationMessage {
  oneof payload {
    InitMigration init = 1;
    MigrationReady ready = 2;
    BlockChunk chunk = 3;
    Ack ack = 4;
    Backpressure backpressure = 5;
    RoundStart round_start = 6;
    RoundComplete round_complete = 7;
    FinalSync final_sync = 8;
    FinalizeComplete finalize_complete = 9;
    FinalizeAck finalize_ack = 10;
    Error error = 11;
  }
}

message InitMigration {
  string volume_id = 1;
  uint64 size_bytes = 2;
  uint32 block_size = 3;
  string format = 4;           // "raw" or "qcow2"
  string checksum_type = 5;    // "crc32"
}

message MigrationReady {
  string dest_volume_id = 1;
}

message Ack {
  uint64 last_offset = 1;
  uint32 last_sequence_num = 2;
  AckStatus status = 3;
}

enum AckStatus {
  ACK_OK = 0;
  ACK_CRC_MISMATCH = 1;
  ACK_WRITE_ERROR = 2;
}

message Backpressure {
  float slow_down_factor = 1;  // 0.5 = halve send rate
}

message RoundStart {
  uint32 round_num = 1;
  uint64 dirty_block_count = 2;
}

message RoundComplete {
  uint32 round_num = 1;
  uint64 blocks_sent = 2;
  uint64 bytes_sent = 3;
}

message FinalSync {
  bool vm_paused = 1;
}

message FinalizeComplete {
  uint64 total_bytes = 1;
  uint64 total_chunks = 2;
  bytes volume_checksum = 3;   // optional, for small volumes
}

message FinalizeAck {
  bool verified = 1;
  string error_message = 2;   // populated if verified=false
}

message Error {
  ErrorCode code = 1;
  string message = 2;
}

enum ErrorCode {
  ERROR_UNSPECIFIED = 0;
  ERROR_DISK_FULL = 1;
  ERROR_IO_ERROR = 2;
  ERROR_VOLUME_NOT_FOUND = 3;
  ERROR_CHECKSUM_MISMATCH = 4;
  ERROR_TIMEOUT = 5;
}
```

## Configuration

| Parameter | Default | Description |
|---|---|---|
| block_size_bytes | 4194304 | Size of each transferred chunk (4MB) |
| ack_interval | 64 | Blocks between acknowledgments |
| send_window | 128 | Max unacked chunks before blocking |
| flow_control_timeout_secs | 30 | Timeout waiting for Ack |
| max_retransmits_per_chunk | 3 | CRC failures before aborting |
| sparse_detection | true | Skip all-zero blocks |

## Non-goals
- Compression (may add later if network is bottleneck)
- Encryption beyond mTLS (data encrypted in transit by TLS)
- Multi-volume parallel streams (one stream per volume in v1)
- Bandwidth throttling (OS-level tc/qdisc if needed)

## Security requirements
- mTLS mandatory (reject plaintext connections)
- Source validates dest certificate against CP-issued CA
- Volume data never written to intermediate storage (direct stream)
- Temp dest volume deleted on failure (no dangling partial copies)

## Recovery model
- Stream break during bulk copy: resumable from last Ack
- Stream break during dirty sync: restart current round
- Stream break during finalize: CP decides — retry finalize or abort entire migration
- stord crash during receive: on restart, delete incomplete dest volume, report to CP

## Implementation Status

| Protocol Phase | File | Status |
|---|---|---|
| Phase 1: Handshake (InitMigration/MigrationReady) | sender.rs, receiver.rs | DONE |
| Phase 2: Bulk Copy (sequential block streaming) | sender.rs `bulk_copy()` | DONE |
| Phase 2: Flow control (SendWindow, Ack) | flow_control.rs | DONE |
| Phase 2: CRC32 per chunk | sender.rs, receiver.rs | DONE |
| Phase 2: Sparse block detection | sender.rs `is_all_zeros()` | DONE |
| Phase 2: Resumability from last Ack | sender.rs `last_acknowledged_offset` | DONE |
| **Phase 3: Dirty sync rounds** | sender.rs | **MISSING** |
| Phase 3: RoundStart/RoundComplete messages | proto defined, never sent | **MISSING** |
| Phase 3: Bitmap read → send dirty → clear → repeat | not implemented | **MISSING** |
| Phase 3: Convergence monitoring | CP `wait_for_convergence` exists, never receives data | **PARTIAL** |
| Phase 4: FinalSync with vm_paused=true | sender.rs sends vm_paused=false | **INCORRECT** |
| Phase 4: Final dirty block flush | not implemented | **MISSING** |
| Phase 4: FinalizeComplete with checksum | sender.rs sends empty checksum | **PARTIAL** |
| Phase 4: FinalizeAck verification | receiver.rs | DONE |
| Backpressure handling | proto exists, sender ignores | **MISSING** |
| mTLS on stream connection | sender.rs uses plain channel | **MISSING** |
| Retransmit on CRC NACK | receiver NACKs, sender retransmits | DONE |

### Implementation Priority

The dirty sync rounds (Phase 3) are the highest priority gap. Without them:
- All writes to the source volume during bulk copy are lost on the destination
- The `wait_for_convergence` function in the CP never converges (no progress reported)
- The FinalSync phase has nothing meaningful to flush (no bitmap tracking active writes)

This is a **data loss bug** in production for any VM with active I/O during migration.
