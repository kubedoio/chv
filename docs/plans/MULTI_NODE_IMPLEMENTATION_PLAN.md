# Multi-Node Architecture Implementation Plan

## Goal

Implement ADR-011 (single-node CP), ADR-012 (disk migration pre-copy), ADR-013 (network overlay VXLAN + eBPF), and the three component specs (live-migration-spec, disk-migration-protocol-spec, vxlan-overlay-spec) as production Rust code in the existing CHV workspace.

## Current State (as of 2026-05-07)

| Component | Exists | State |
|-----------|--------|-------|
| `StorageBackend` trait | Yes | `crates/chv-stord-backends/src/trait.rs` — has open, close, attach, detach, health, resize, snapshot, clone, restore, delete. No migration methods. |
| `NetworkExecutor` trait | Yes | `crates/chv-nwd-core/src/executor.rs` — has ensure_topology, delete_topology, attach_vm_nic, detach_vm_nic, firewall, NAT, DHCP, DNS, expose/withdraw. No VXLAN/FDB/eBPF. |
| `Orchestrator` | Yes | `crates/chv-controlplane-service/src/orchestrator.rs` — tick loop, dispatches operations via gRPC to agents. `MigrateVm` referenced in `requires_schedulable_node` but NOT in `dispatch_operation` match arm. |
| `chv-nwd-api.proto` | Yes | `TopologySpec` has network_id, tenant_id, bridge_name, namespace_name, subnet_cidr, gateway_ip, options. No VNI, no VTEP, no overlay fields. |
| `chv-stord-api.proto` | Yes | `StorageService` with volume lifecycle. No migration streaming RPCs. |
| `control-plane-node.proto` | Yes | Enrollment, Inventory, Reconcile, Lifecycle, Telemetry services. No MigrateVm RPC, no migration progress reporting. |
| SQLite migrations | 35 exist | Up to `0035_node_last_seen.sql`. No VTEP registry, no VNI tables, no migration tracking tables. |
| eBPF programs | None | No `.bpf.c` files, no eBPF loading code. |
| Inter-stord gRPC | None | stord only exposes local `StorageService`. No peer-to-peer streaming. |

## Constraints

- Rust workspace, `tonic-build` for proto generation
- Proto files in `/proto` are source of truth
- SQLite WAL mode, single-writer (ADR-011)
- mTLS for all inter-node communication (existing agent certificates)
- No shared storage in v1 (ADR-012)
- Kernel VXLAN module for encap/decap, NOT eBPF (ADR-013)
- eBPF only for policy enforcement (TC hooks)
- All control-plane coordination through existing gRPC channel
- Agents operate autonomously during CP outage (ADR-006)

---

## Phase 1: Proto Contracts & Database Schema

**Goal:** Define all wire formats and persistence schema. Nothing compiles into runtime logic yet. Everything downstream depends on these definitions being correct.

### 1.1 — Extend `chv-nwd-api.proto`

Add to existing `TopologySpec`:
```protobuf
uint32 vni = 8;                          // 0 = no overlay (bridge-only)
repeated VtepEndpoint vtep_endpoints = 9;
OverlayType overlay_type = 10;
```

Add new messages and enums per `vxlan-overlay-spec.md`:
- `VtepEndpoint { string node_id; string vtep_ip; uint32 vtep_port; }`
- `OverlayType { OVERLAY_NONE = 0; OVERLAY_VXLAN = 1; }`
- `SecurityPolicy { string vm_id; string network_id; PolicyAction default_action; repeated SecurityRule rules; }`
- `SecurityRule { Direction direction; Protocol protocol; string src_cidr; string dst_cidr; PortRange src_port; PortRange dst_port; PolicyAction action; uint32 priority; }`
- `PortRange { uint32 min; uint32 max; }`
- `Direction { DIRECTION_BOTH = 0; DIRECTION_INGRESS = 1; DIRECTION_EGRESS = 2; }`
- `Protocol { PROTOCOL_ANY = 0; PROTOCOL_TCP = 1; PROTOCOL_UDP = 2; PROTOCOL_ICMP = 3; }`
- `PolicyAction { POLICY_DENY = 0; POLICY_ALLOW = 1; }`
- `RateLimitPolicy { string vm_id; uint64 rate_bps; uint64 burst_bytes; }`
- `FdbEntry { string mac_address; string vtep_ip; }`

Add new RPCs to `NetworkService`:
```protobuf
rpc UpdateOverlay(UpdateOverlayRequest) returns (UpdateOverlayResponse);
rpc UpdateSecurityPolicy(SecurityPolicy) returns (UpdateSecurityPolicyResponse);
rpc UpdateRateLimit(RateLimitPolicy) returns (UpdateRateLimitResponse);
rpc GetOverlayStatus(GetOverlayStatusRequest) returns (OverlayStatus);
```

Add request/response messages per spec:
- `UpdateOverlayRequest { string network_id; uint32 vni; repeated VtepEndpoint vtep_endpoints; repeated FdbEntry fdb_entries; }`
- `UpdateOverlayResponse { Result result; }`
- `UpdateSecurityPolicyResponse { Result result; }`
- `UpdateRateLimitResponse { Result result; }`
- `GetOverlayStatusRequest { string network_id; }`
- `OverlayStatus { string network_id; uint32 vni; bool vxlan_interface_up; uint32 fdb_entry_count; uint32 ebpf_programs_loaded; }`

### 1.2 — New proto file: `proto/node/chv-stord-migration.proto`

New service `StorageMigrationService` per `disk-migration-protocol-spec.md`:
```protobuf
service StorageMigrationService {
  rpc StreamBlocks(stream MigrationMessage) returns (stream MigrationMessage);
}
```

All messages defined in the spec:
- `MigrationMessage` (oneof: init, ready, chunk, ack, backpressure, round_start, round_complete, final_sync, finalize_complete, finalize_ack, error)
- `InitMigration { volume_id, size_bytes, block_size, format, checksum_type }`
- `MigrationReady { dest_volume_id }`
- `BlockChunk { offset, data, crc32, is_sparse, sequence_num }`
- `Ack { last_offset, last_sequence_num, status }`
- `AckStatus { ACK_OK = 0; ACK_CRC_MISMATCH = 1; ACK_WRITE_ERROR = 2; }`
- `Backpressure { float slow_down_factor }`
- `RoundStart { round_num, dirty_block_count }`
- `RoundComplete { round_num, blocks_sent, bytes_sent }`
- `FinalSync { bool vm_paused }`
- `FinalizeComplete { total_bytes, total_chunks, volume_checksum }`
- `FinalizeAck { verified, error_message }`
- `Error { code, message }`
- `ErrorCode { ERROR_UNSPECIFIED = 0; ERROR_DISK_FULL = 1; ERROR_IO_ERROR = 2; ERROR_VOLUME_NOT_FOUND = 3; ERROR_CHECKSUM_MISMATCH = 4; ERROR_TIMEOUT = 5; }`

### 1.3 — Extend `control-plane-node.proto`

Add to `LifecycleService`:
```protobuf
rpc MigrateVm(MigrateVmRequest) returns (AckResponse);
```

Add messages per `live-migration-spec.md`:
- `MigrateVmRequest { RequestMeta meta; string node_id; string vm_id; string source_node_id; string destination_node_id; MigrationConfig config; }`
- `MigrationConfig { uint32 dirty_threshold_blocks; uint32 max_convergence_rounds; uint32 block_size_bytes; uint32 total_timeout_seconds; }`
- `MigrationProgress { string vm_id; string operation_id; MigrationPhase phase; uint64 bytes_transferred; uint64 total_bytes; uint32 convergence_round; uint64 dirty_blocks_remaining; float progress_percent; }`
- `MigrationPhase` enum (UNSPECIFIED, PENDING, PRECOPY_DISK, CONVERGING_DISK, MEMORY_MIGRATION, PAUSED, COMPLETED, FAILED, ROLLED_BACK)

Add to `TelemetryService`:
```protobuf
rpc ReportMigrationProgress(MigrationProgress) returns (AckResponse);
```

### 1.4 — SQLite migrations

**`0036_vtep_registry.sql`:**
```sql
CREATE TABLE vtep_registry (
    node_id TEXT NOT NULL PRIMARY KEY REFERENCES nodes(node_id),
    vtep_ip TEXT NOT NULL,
    vtep_port INTEGER NOT NULL DEFAULT 4789,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX idx_vtep_registry_ip ON vtep_registry(vtep_ip);
```

**`0037_vni_allocation.sql`:**
```sql
ALTER TABLE networks ADD COLUMN vni INTEGER DEFAULT 0;
ALTER TABLE networks ADD COLUMN overlay_type TEXT NOT NULL DEFAULT 'none';

CREATE TABLE vni_allocations (
    vni INTEGER NOT NULL PRIMARY KEY,
    network_id TEXT NOT NULL REFERENCES networks(network_id),
    allocated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    released_at TEXT  -- NULL means active; set on network deletion
);
CREATE INDEX idx_vni_allocations_network ON vni_allocations(network_id);
CREATE INDEX idx_vni_allocations_released ON vni_allocations(released_at);
```

**`0038_migration_operations.sql`:**
```sql
CREATE TABLE migrations (
    migration_id TEXT NOT NULL PRIMARY KEY,
    operation_id TEXT NOT NULL REFERENCES operations(operation_id),
    vm_id TEXT NOT NULL REFERENCES vms(vm_id),
    source_node_id TEXT NOT NULL,
    destination_node_id TEXT NOT NULL,
    phase TEXT NOT NULL DEFAULT 'Pending',
    bytes_transferred INTEGER NOT NULL DEFAULT 0,
    total_bytes INTEGER NOT NULL DEFAULT 0,
    convergence_round INTEGER NOT NULL DEFAULT 0,
    dirty_blocks_remaining INTEGER NOT NULL DEFAULT 0,
    started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    completed_at TEXT,
    error_message TEXT
);
CREATE INDEX idx_migrations_vm ON migrations(vm_id);
CREATE INDEX idx_migrations_phase ON migrations(phase);
CREATE INDEX idx_migrations_operation ON migrations(operation_id);
```

**`0039_security_policies.sql`:**
```sql
CREATE TABLE security_policies (
    policy_id TEXT NOT NULL PRIMARY KEY,
    vm_id TEXT NOT NULL,
    network_id TEXT NOT NULL,
    default_action TEXT NOT NULL DEFAULT 'deny',
    rules_json TEXT NOT NULL DEFAULT '[]',
    version INTEGER NOT NULL DEFAULT 1,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX idx_security_policies_vm ON security_policies(vm_id);
CREATE INDEX idx_security_policies_network ON security_policies(network_id);

CREATE TABLE rate_limit_policies (
    vm_id TEXT NOT NULL PRIMARY KEY,
    rate_bps INTEGER NOT NULL,
    burst_bytes INTEGER NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
```

### 1.5 — Verification

- `cargo build --workspace` compiles with new proto files (generated code exists in `gen/rust`)
- New migrations apply cleanly on fresh DB
- No runtime code changes yet; only contracts and schema

---

## Phase 2: StorageBackend Trait Extension (Dirty Tracking)

**Goal:** Extend the `StorageBackend` trait with migration methods. Implement for the `local` (file) backend first. LVM backend deferred.

### 2.1 — Extend `StorageBackend` trait

Add to `crates/chv-stord-backends/src/trait.rs`:

```rust
/// Enable dirty block tracking on an open volume.
/// Must be called before bulk copy starts.
async fn enable_dirty_tracking(
    &self,
    volume_id: &str,
    handle: &str,
    block_size: u64,
) -> Result<(), ChvError>;

/// Read the current dirty bitmap. Each set bit = one block
/// written since last clear. Returns byte vec (1 bit per block).
async fn get_dirty_bitmap(
    &self,
    volume_id: &str,
    handle: &str,
) -> Result<Vec<u8>, ChvError>;

/// Atomically clear the dirty bitmap. Call after reading and
/// before starting the next sync round.
async fn clear_dirty_bitmap(
    &self,
    volume_id: &str,
    handle: &str,
) -> Result<(), ChvError>;

/// Disable dirty tracking. Must be called on migration complete or abort.
async fn disable_dirty_tracking(
    &self,
    volume_id: &str,
    handle: &str,
) -> Result<(), ChvError>;

/// Read a range of bytes from the volume for export.
/// Returns raw block data.
async fn read_block(
    &self,
    volume_id: &str,
    handle: &str,
    offset: u64,
    length: u64,
) -> Result<Vec<u8>, ChvError>;

/// Write a block at the given offset. Used by destination stord.
async fn write_block(
    &self,
    volume_id: &str,
    handle: &str,
    offset: u64,
    data: &[u8],
) -> Result<(), ChvError>;

/// Get volume size in bytes (needed for bulk copy planning).
async fn volume_size(
    &self,
    volume_id: &str,
    handle: &str,
) -> Result<u64, ChvError>;

/// Create an empty volume of given size and format for receiving migration data.
async fn create_receiving_volume(
    &self,
    volume_id: &str,
    size_bytes: u64,
    format: &str,
) -> Result<VolumeExport, ChvError>;

/// Delete a volume completely (used for cleanup on migration failure).
async fn delete_volume(
    &self,
    volume_id: &str,
) -> Result<(), ChvError>;
```

### 2.2 — Implement for `LocalBackend` (`crates/chv-stord-backends/src/local.rs`)

- `enable_dirty_tracking`: allocate in-memory `BitVec` (1 bit per block). Install write interception via `io_uring` or wrapping the file handle to mark bits on every `write()`. Per spec: "userspace bitmap updated on every write() to volume (simplest, works for all backends)."
- `get_dirty_bitmap`: return clone of current BitVec as `Vec<u8>`.
- `clear_dirty_bitmap`: atomically zero the BitVec.
- `disable_dirty_tracking`: drop the BitVec and remove write interception.
- `read_block`: `pread` at offset for length bytes.
- `write_block`: `pwrite` at offset with data.
- `volume_size`: `fstat` or metadata query on the volume file.
- `create_receiving_volume`: create a sparse file of given size (or qcow2 if format matches).
- `delete_volume`: `unlink` the volume file.

### 2.3 — Implement for `LvmBackend` (`crates/chv-stord-backends/src/lvm.rs`)

- Same interface but using `dm-snapshot` for dirty tracking where available.
- Fallback to userspace bitmap if dm-snapshot not available.
- `read_block` / `write_block`: direct block device I/O.
- `create_receiving_volume`: `lvcreate` a new LV.
- `delete_volume`: `lvremove`.

### 2.4 — Unit tests

- Test dirty bitmap: write 3 blocks, verify bitmap has exactly 3 bits set, clear, verify empty.
- Test read/write round-trip: write block, read back, compare.
- Test create/delete receiving volume lifecycle.
- All tests use tempfiles (no real LVM required for local backend tests).

---

## Phase 3: Storage Migration Service (stord-to-stord streaming)

**Goal:** Implement the `StorageMigrationService` gRPC service in `chv-stord-core`, enabling peer-to-peer block streaming per `disk-migration-protocol-spec.md`.

### 3.1 — New crate: `crates/chv-stord-migration`

Alternatively, add a `migration` module to `chv-stord-core`. Decision: add as module to `chv-stord-core` (fewer crate boundaries, stord already holds the backend reference).

Files:
- `crates/chv-stord-core/src/migration/mod.rs` — re-exports
- `crates/chv-stord-core/src/migration/service.rs` — tonic service implementation
- `crates/chv-stord-core/src/migration/sender.rs` — source-side streaming logic
- `crates/chv-stord-core/src/migration/receiver.rs` — dest-side receiving logic
- `crates/chv-stord-core/src/migration/flow_control.rs` — send window, ack tracking, backpressure

### 3.2 — Service implementation

`StorageMigrationService::stream_blocks`:
- Bidirectional stream per proto definition.
- First message from source: `InitMigration` (volume_id, size_bytes, block_size, format, checksum_type).
- Dest receives, calls `create_receiving_volume`, sends `MigrationReady`.
- Source streams `BlockChunk` messages sequentially (bulk copy).
- Dest writes each chunk via `write_block`, verifies CRC32.
- Dest sends `Ack` every `ack_interval` (64) blocks.
- Source maintains send window (128 max unacked). Blocks if window full.
- Sparse detection: if block is all zeros, send `is_sparse = true` with empty data.
- After bulk copy: `BulkCopyComplete` (modeled as `RoundComplete` with round_num=0).
- Dirty sync rounds: source sends `RoundStart`, then dirty blocks, then `RoundComplete`.
- Finalize: source sends `FinalSync`, final dirty blocks, `FinalizeComplete`. Dest responds `FinalizeAck`.

### 3.3 — Flow control

- Sender: track `next_sequence_num`, `last_acked_sequence`, compute window.
- Receiver: count received chunks, send `Ack` every N.
- `Backpressure` message from receiver: sender adjusts delay between chunks.
- Timeout: if no `Ack` within 30s, sender aborts with `Error(ERROR_TIMEOUT)`.

### 3.4 — Integrity

- CRC32 per `BlockChunk` (use `crc32fast` crate).
- On mismatch: receiver sends `Ack` with `ACK_CRC_MISMATCH`. Sender retransmits (max 3 retries per chunk, then abort).

### 3.5 — Resumability

- Bulk copy: on reconnect, resume from `last_acked_sequence * block_size` offset.
- Dirty sync: restart current round from fresh bitmap read.
- Finalize: not resumable, must restart finalize phase.

### 3.6 — Server registration

- `chv-stord` binary (`cmd/chv-stord/main.rs`) adds `StorageMigrationService` to its tonic server.
- Listens on same port as `StorageService` (separate service on same server).
- mTLS enforced (reject plaintext). Uses existing agent node certificates.

### 3.7 — Integration test

- Two in-process stord instances (source + dest) with tempdir backends.
- Create a 16MB test volume on source with known pattern.
- Run full migration: bulk copy + 1 dirty round + finalize.
- Verify dest volume is byte-identical to source.
- Test: CRC mismatch recovery (inject corruption, verify retransmit).
- Test: sparse block detection (verify zero block not sent as data).

---

## Phase 4: VXLAN Overlay Networking (chv-nwd)

**Goal:** Implement VXLAN interface lifecycle, FDB management, and overlay RPCs in `chv-nwd-core`.

### 4.1 — Extend `NetworkExecutor` trait

Add to `crates/chv-nwd-core/src/executor.rs`:

```rust
async fn create_vxlan_interface(
    &self,
    network_id: &str,
    vni: u32,
    vtep_ip: &str,
    vtep_port: u32,
    bridge_name: &str,
) -> Result<(), ChvError>;

async fn delete_vxlan_interface(
    &self,
    network_id: &str,
    vni: u32,
) -> Result<(), ChvError>;

async fn add_fdb_entry(
    &self,
    network_id: &str,
    vni: u32,
    mac_address: &str,
    peer_vtep_ip: &str,
) -> Result<(), ChvError>;

async fn delete_fdb_entry(
    &self,
    network_id: &str,
    vni: u32,
    mac_address: &str,
    peer_vtep_ip: &str,
) -> Result<(), ChvError>;

async fn replace_fdb_entry(
    &self,
    network_id: &str,
    vni: u32,
    mac_address: &str,
    new_vtep_ip: &str,
) -> Result<(), ChvError>;

async fn send_gratuitous_arp(
    &self,
    network_id: &str,
    bridge_name: &str,
    vni: u32,
    vm_ip: &str,
) -> Result<(), ChvError>;

async fn set_arp_suppression(
    &self,
    network_id: &str,
    vni: u32,
    enable: bool,
) -> Result<(), ChvError>;

async fn get_overlay_status(
    &self,
    network_id: &str,
    vni: u32,
) -> Result<OverlayStatusInfo, ChvError>;
```

### 4.2 — Implement in `LinuxExecutor`

Each method maps directly to commands from the spec:

- `create_vxlan_interface`:
  ```
  ip link add vxlan{VNI} type vxlan id {VNI} local {VTEP_IP} dstport {port} nolearning
  ip link set vxlan{VNI} master {bridge_name}
  ip link set vxlan{VNI} up
  ip link set dev {bridge_name} mtu {underlay_mtu - 50}
  ```
- `delete_vxlan_interface`: `ip link del vxlan{VNI}`
- `add_fdb_entry`: `bridge fdb append {MAC} dev vxlan{VNI} dst {PEER_VTEP_IP}`
- `delete_fdb_entry`: `bridge fdb del {MAC} dev vxlan{VNI} dst {PEER_VTEP_IP}`
- `replace_fdb_entry`: `bridge fdb replace {MAC} dev vxlan{VNI} dst {NEW_VTEP_IP}`
- `send_gratuitous_arp`:
  ```
  arping -U -c 3 -I {bridge_name} {VM_IP}
  arping -A -c 3 -I vxlan{VNI} {VM_IP}
  ```
- `set_arp_suppression`: `bridge link set dev vxlan{VNI} neigh_suppress on|off`
- `get_overlay_status`: check link state of vxlan interface, count FDB entries via `bridge fdb show dev vxlan{VNI}`

### 4.3 — Modify `ensure_topology`

When `TopologySpec.vni > 0` and `overlay_type == OVERLAY_VXLAN`:
1. Create bridge and namespace as before.
2. Call `create_vxlan_interface` to add VXLAN to the bridge.
3. For each `vtep_endpoint` in the spec, no automatic FDB entries (those come via `UpdateOverlay`).

When `vni == 0`: existing bridge-only behavior (unchanged).

### 4.4 — New RPC handlers

Add to `crates/chv-nwd-core/src/handlers.rs`:

- `update_overlay`: receive `UpdateOverlayRequest`, create VXLAN interface if not exists, apply FDB entries (add missing, remove stale).
- `update_security_policy`: receive `SecurityPolicy`, write eBPF maps (Phase 6).
- `update_rate_limit`: receive `RateLimitPolicy`, update eBPF rate limit map (Phase 6).
- `get_overlay_status`: query interface state and FDB count.

### 4.5 — MTU handling

- On VXLAN interface creation, read underlay interface MTU.
- Set bridge MTU to `min(underlay_mtu - 50, 1500)`.
- If underlay >= 1550, inner MTU = 1500 (transparent to VMs).
- If underlay == 1500, inner MTU = 1450.

### 4.6 — Tests

- Unit test: `create_vxlan_interface` generates correct `ip link add` command args.
- Unit test: `add_fdb_entry` generates correct `bridge fdb append` args.
- Unit test: `ensure_topology` with VNI > 0 calls VXLAN creation.
- Integration test (requires Linux, runs in CI with network namespaces): create real VXLAN, add FDB, verify with `bridge fdb show`.

---

## Phase 5: Live Migration Orchestrator (Control Plane)

**Goal:** Implement the `MigrateVm` operation type in the orchestrator, following the 5-phase state machine from `live-migration-spec.md`.

### 5.1 — Migration state machine module

New file: `crates/chv-controlplane-service/src/migration.rs`

```rust
pub enum MigrationPhase {
    Pending,
    PreCopyDisk,
    ConvergingDisk,
    MemoryMigration,
    Paused,
    Completed,
    Failed,
    RolledBack,
}

pub struct MigrationState {
    pub migration_id: String,
    pub operation_id: String,
    pub vm_id: String,
    pub source_node_id: String,
    pub dest_node_id: String,
    pub phase: MigrationPhase,
    pub config: MigrationConfig,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub convergence_round: u32,
    pub dirty_blocks_remaining: u64,
}
```

### 5.2 — Orchestrator dispatch for `MigrateVm`

Add arm to `dispatch_operation` in `orchestrator.rs`:

```rust
"MigrateVm" => {
    self.execute_migration(row).await
}
```

`execute_migration` is a long-running async fn that drives all 5 phases:

### 5.3 — Phase 1: PreCopyDisk

1. Validate source node is TenantReady, dest node is TenantReady, VM is Running.
2. Validate dest has sufficient resources (query node inventory: CPU, memory, disk).
3. CP → dest nwd: `UpdateOverlay` (join VNI for VM's network).
4. CP → dest stord: call `StorageMigrationService.StreamBlocks` on dest (dest listens).
5. CP → source stord: initiate StreamBlocks to dest stord endpoint (source connects to dest).
6. Actually: CP instructs source agent to start the migration stream. Source stord connects to dest stord directly (peer-to-peer, no CP relay).
7. CP monitors progress via `ReportMigrationProgress` telemetry from source agent.
8. Update `migrations` table: phase = PreCopyDisk.

### 5.4 — Phase 2: ConvergingDisk

1. Source stord internally transitions from bulk copy to dirty sync rounds.
2. CP monitors `dirty_blocks_remaining` from telemetry.
3. When `dirty_blocks_remaining < config.dirty_threshold_blocks` (default 1024) → Phase 3.
4. If `convergence_round >= config.max_convergence_rounds` (default 10) → force Phase 3.
5. Update `migrations` table each round.

### 5.5 — Phase 3: MemoryMigration

1. CP → dest agent: open CH migration receiving socket.
   - Dest agent calls CH API: `PUT /api/v1/vm.receive-migration` with `{"receiver_url": "tcp://0.0.0.0:{port}"}`.
   - Port from pool: `config.memory_migration_port_range` (49152-49200).
2. CP → source agent: start memory migration.
   - Source agent calls CH API: `PUT /api/v1/vm.send-migration` with `{"receiver_url": "tcp://{dest_ip}:{port}"}`.
3. CH handles iterative memory pre-copy internally.
4. CP polls source agent for CH state.
5. When CH reports migration entering final phase → Phase 4 (Paused).

### 5.6 — Phase 4: Paused (Final Sync)

1. CH pauses VM on source automatically.
2. CP → source stord: send `FinalSync(vm_paused=true)` on the migration stream.
3. Source stord flushes final dirty blocks.
4. Dest stord sends `FinalizeAck(verified=true)`.
5. CH transfers final memory pages to dest.
6. Dest agent: CH resumes VM (auto-resume after receive completes).
7. VM is now running on destination.

### 5.7 — Phase 5: Completed (Validation & Cleanup)

1. CP waits for dest agent heartbeat confirming VM state = Running.
2. CP → dest nwd: send gratuitous ARP for VM's IP/MAC (`send_gratuitous_arp`).
3. CP → all peer nwd: `UpdateOverlay` with updated FDB (VM's MAC now at dest VTEP).
4. CP updates SQLite: VM placement → dest node, migration phase = Completed.
5. CP → source: cleanup (delete source volume, release resources).
6. CP → source nwd: remove FDB entries for migrated VM's MAC.

### 5.8 — Rollback handling

| Phase | Failure → Action |
|-------|-----------------|
| PreCopyDisk | Abort stream, delete dest volume, disable dirty tracking on source. VM continues on source. → RolledBack |
| ConvergingDisk | Same as PreCopyDisk. → RolledBack |
| MemoryMigration | Cannot cleanly rollback mid-CH-transfer. → Failed (manual recovery) |
| Paused | If dest fails to resume: source still has all data. Resume on source. → RolledBack |
| Completed | Post-completion. Cleanup failures logged as warnings. |

### 5.9 — Timeouts

Per `live-migration-spec.md`:
- PreCopyDisk: `disk_size_gb * 60s`
- ConvergingDisk: `300s per round, total 3000s`
- MemoryMigration: `memory_size_gb * 30s + 120s`
- Paused (final sync): `60s`
- Total: sum + 300s buffer

Implement as `tokio::time::timeout` wrapping each phase.

### 5.10 — CP-side VTEP registry management

New functions in `chv-controlplane-store`:
- `register_vtep(node_id, vtep_ip, vtep_port)` — called during enrollment when node reports inventory.
- `get_vteps_for_network(network_id)` — returns all nodes participating in a VNI.
- `allocate_vni(network_id)` — atomically allocate next free VNI (range 1-16777214, skip recently released within 24h).
- `release_vni(network_id)` — set `released_at` (24h reuse delay per spec).

### 5.11 — Operation reaper extension

Migration operations stuck > 2x total_timeout → mark Failed and alert (per spec).

### 5.12 — Tests

- Unit test: state machine transitions (Pending → PreCopyDisk → ... → Completed).
- Unit test: rollback at each phase.
- Unit test: timeout calculation per spec.
- Integration test: mock source/dest agents, drive full migration flow.

---

## Phase 6: eBPF Policy Enforcement

**Goal:** Implement eBPF program loading and map management in `chv-nwd` for per-VM security policies and rate limiting.

### 6.1 — eBPF programs (C, compiled with clang/BPF CO-RE)

New directory: `ebpf/`

Files:
- `ebpf/policy_tc.bpf.c` — TC classifier program
- `ebpf/Makefile` — builds `.o` files with clang
- `ebpf/vmlinux.h` — generated BPF CO-RE header

The program implements:
- Parse packet headers (Ethernet → IP → TCP/UDP/ICMP).
- Hash vm_id from TC metadata.
- Look up rules in `rule_map` (BPF_MAP_TYPE_HASH), iterate by priority.
- Match: src_ip/mask, dst_ip/mask, src_port range, dst_port range, protocol.
- First matching rule determines action (ALLOW/DENY).
- If no rule matches: use default_action from a per-VM defaults map.
- Rate limiting: token bucket per `rate_map`.
- Stats: increment per-CPU counters in `stats_map`.

Maps (per spec):
- `rule_map`: key={vm_id_hash, direction, priority}, value={src_ip, src_mask, dst_ip, dst_mask, src_port_min/max, dst_port_min/max, protocol, action}
- `rate_map`: key={vm_id_hash}, value={tokens, last_refill_ns, rate_bps, burst_bytes}
- `stats_map`: key={vm_id_hash, direction}, value={packets_allowed, packets_denied, bytes_allowed, bytes_denied} (PERCPU_ARRAY)

### 6.2 — eBPF loader module in chv-nwd

New file: `crates/chv-nwd-core/src/ebpf.rs`

Use `libbpf-rs` crate for loading and map manipulation:
- `load_policy_program(tap_name)` — load TC program, attach to TAP egress.
- `load_ingress_program(bridge_name)` — load TC program, attach to bridge ingress.
- `update_rules(vm_id, rules: Vec<SecurityRule>)` — write entries to rule_map.
- `update_rate_limit(vm_id, rate_bps, burst_bytes)` — write to rate_map.
- `read_stats(vm_id)` — read from stats_map (percpu aggregation).
- `detach_program(interface)` — detach TC program from interface.

### 6.3 — Integrate with RPC handlers

- `update_security_policy` handler: parse SecurityPolicy proto, call `update_rules`.
- `update_rate_limit` handler: call `update_rate_limit`.
- On `attach_vm_nic`: after creating TAP, call `load_policy_program` on the TAP.
- On `detach_vm_nic`: call `detach_program` before deleting TAP.

### 6.4 — Default-deny on failure

Per spec: "nwd reports error to CP. Default-deny until program loads successfully."
- If eBPF program fails to load (verifier rejection, missing file), log error, report health degraded.
- Until loaded: TC default behavior is PASS (kernel default). To enforce default-deny: add a simple nftables rule on the TAP that drops all traffic, remove it once eBPF loads.

### 6.5 — Stats collection

Per spec: `ebpf.stats_interval_secs = 10`
- Background task reads eBPF stats every 10s.
- Emits Prometheus metrics: `chv_vm_packets_total{vm_id, direction, action}`, `chv_vm_bytes_total{vm_id, direction, action}`.

### 6.6 — Build integration

- `ebpf/Makefile` target produces `.o` files.
- Installed to `/usr/lib/chv/ebpf/` (per `ebpf.program_path` config).
- `make release` includes eBPF compilation step.
- CI: requires `clang`, `llvm`, `libbpf-dev` build deps.

### 6.7 — Tests

- Unit test: rule map serialization/deserialization.
- Integration test (requires Linux kernel with BPF): load program in a network namespace, verify packet classification with known traffic.

---

## Phase 7: Agent-Side Migration Coordination

**Goal:** The agent (`chv-agent-core`) must handle CP instructions for migration: open CH receive socket, call CH send-migration, report progress.

### 7.1 — New agent RPC handlers

The existing agent receives instructions via `ApplyVmDesiredState` or new dedicated messages. Based on the proto, `MigrateVm` goes to the agent on the source node. The agent needs:

- Handler for `MigrateVm` instruction (source agent):
  1. Open CH migration send: `PUT /api/v1/vm.send-migration` with `receiver_url`.
  2. Report progress via `ReportMigrationProgress` to CP.
  3. On completion: report VM stopped on source.

- Handler for "prepare migration receive" (dest agent):
  1. Open CH migration receive: `PUT /api/v1/vm.receive-migration`.
  2. Return the port/address to CP.
  3. On completion: report VM running on dest.

### 7.2 — CH adapter extension

In `crates/chv-agent-runtime-ch`:
- `send_migration(receiver_url: &str)` — calls CH REST API.
- `receive_migration(receiver_url: &str)` — calls CH REST API.
- Both are blocking long-running operations. Spawn as background tasks.

### 7.3 — Progress reporting

- Source agent polls CH dirty log endpoint: `GET /api/v1/vm.dirty-log`.
- Reports `MigrationProgress` to CP every 5s during memory migration phase.

---

## Phase 8: Control Plane VTEP & VNI Management

**Goal:** CP manages the VTEP registry and VNI lifecycle as part of enrollment and network creation.

### 8.1 — Enrollment extension

When a node enrolls (`EnrollmentService.EnrollNode`):
- Node reports its VTEP IP as part of `NodeInventory` (new field: `vtep_ip`).
- CP writes to `vtep_registry` table.

Proto change (minor): add `string vtep_ip = 10;` to `NodeInventory`.

### 8.2 — Network creation with VNI

When a network is created with `overlay_type = "vxlan"`:
1. CP allocates a VNI via `allocate_vni`.
2. CP writes VNI to `networks` table.
3. CP identifies which nodes have VMs on this network.
4. CP sends `UpdateOverlay` to each participating node's nwd.

### 8.3 — VM placement with overlay

When a VM is placed on a network with VNI > 0:
1. CP → dest nwd: `UpdateOverlay` (ensure VXLAN interface exists).
2. CP → all other nodes on same VNI: add FDB entry for new VM's MAC → dest VTEP.

### 8.4 — VM destruction with overlay

When a VM is destroyed:
1. CP → all peer nodes: remove FDB entry for VM's MAC.
2. If last VM on VNI for this node: CP → node nwd: remove VXLAN interface.

### 8.5 — VNI reuse delay

Per spec: "VNI not reused for 24 hours after network deletion."
- `allocate_vni` query: exclude VNIs where `released_at IS NOT NULL AND released_at > now - 24h`.
- Background task in CP: no explicit cleanup needed (stale rows are just skipped).

---

## Phase 9: End-to-End Integration & Testing

**Goal:** Wire everything together and prove the full migration path works.

### 9.1 — Multi-node integration test harness

- Two-node test setup using network namespaces (no real hardware).
- Each "node" runs: agent, stord, nwd in the same process with separate state dirs.
- CP orchestrates between them.
- Use loopback for VTEP communication.

### 9.2 — Test scenarios

1. **Happy path**: Create VM on node A, create overlay network, migrate to node B. Verify: VM running on B, FDB updated, gratuitous ARP sent, source cleaned up.
2. **Disk convergence**: VM with active writes during migration. Verify: convergence rounds occur, dirty count decreases.
3. **Failure in PreCopyDisk**: Kill dest stord mid-stream. Verify: rollback, VM continues on source.
4. **Failure in MemoryMigration**: Kill dest agent mid-CH-transfer. Verify: marked Failed.
5. **Network continuity**: Two VMs on same VNI across nodes. Ping between them before/during/after migration.
6. **Security policy**: Apply deny rule, verify packets dropped. Apply allow rule, verify packets pass.

### 9.3 — Performance baselines

- Measure block streaming throughput (MB/s) for known volume sizes.
- Measure total migration time for {1GB, 10GB, 100GB} volumes with {low, high} write rates.
- Measure gratuitous ARP convergence time after migration.

---

## Phase 10: Configuration, Observability & Documentation

### 10.1 — Configuration

Add to `chv-config`:
```toml
[overlay]
vxlan_port = 4789
vtep_interface = "auto"  # auto-detect first non-loopback with default route
nolearning = true
arp_suppress = false
inner_mtu = "auto"  # underlay_mtu - 50, capped at 1500

[ebpf]
program_path = "/usr/lib/chv/ebpf/"
default_action = "deny"
stats_interval_secs = 10

[migration]
dirty_threshold_blocks = 1024
max_convergence_rounds = 10
block_size_bytes = 4194304
memory_migration_port_range = "49152-49200"
total_timeout_multiplier = 1.5
```

### 10.2 — Observability (per ADR-009)

Prometheus metrics:
- `chv_migration_phase{vm_id, phase}` — gauge per active migration
- `chv_migration_bytes_transferred{vm_id}` — counter
- `chv_migration_duration_seconds{vm_id, outcome}` — histogram
- `chv_migration_dirty_blocks{vm_id, round}` — gauge
- `chv_vxlan_fdb_entries{network_id, node_id}` — gauge
- `chv_ebpf_packets_total{vm_id, direction, action}` — counter
- `chv_ebpf_bytes_total{vm_id, direction, action}` — counter

Tracing spans:
- `migration::phase_1`, `migration::phase_2`, ..., `migration::phase_5`
- `stord::stream_blocks`, `stord::receive_blocks`
- `nwd::update_overlay`, `nwd::update_security_policy`

### 10.3 — Operations documentation

Update `docs/OPERATIONS.md` with:
- Migration monitoring (which metrics to watch).
- Troubleshooting guide: stale FDB, VXLAN interface down, eBPF load failure.
- Backup procedures for VTEP registry and VNI state (covered by SQLite backup).

---

## Dependency Graph

```
Phase 1 (Proto + Schema)
    │
    ├─── Phase 2 (StorageBackend trait extension)
    │        │
    │        └─── Phase 3 (Storage Migration Service)
    │                     │
    │                     └─── Phase 7 (Agent migration coordination)
    │                                   │
    │                                   └─── Phase 5 (Orchestrator)
    │                                                  │
    │                                                  └─── Phase 9 (E2E testing)
    │
    ├─── Phase 4 (VXLAN overlay in nwd)
    │        │
    │        └─── Phase 6 (eBPF policy)
    │                     │
    │                     └─── Phase 8 (CP VTEP/VNI management)
    │                                   │
    │                                   └─── Phase 9 (E2E testing)
    │
    └─── Phase 10 (Config, observability, docs) — parallel with 5-9

```

Phases 2+3+7 and Phases 4+6+8 are independent tracks that can proceed in parallel after Phase 1.

---

## Estimated Scope Per Phase

| Phase | New files | Modified files | Lines (est.) | Dependencies added |
|-------|-----------|---------------|-------------|-------------------|
| 1 | 5 (protos + migrations) | 2 (existing protos) | ~400 | none |
| 2 | 0 | 3 (trait.rs, local.rs, lvm.rs) | ~500 | none |
| 3 | 5 (migration module) | 2 (server.rs, Cargo.toml) | ~1200 | `crc32fast` |
| 4 | 1 (vxlan.rs) | 2 (executor.rs, handlers.rs) | ~600 | none |
| 5 | 1 (migration.rs) | 2 (orchestrator.rs, lib.rs) | ~1000 | none |
| 6 | 3 (ebpf.rs, .bpf.c, Makefile) | 2 (handlers.rs, Cargo.toml) | ~800 | `libbpf-rs` |
| 7 | 1 (migration handler) | 2 (agent handlers, CH adapter) | ~400 | none |
| 8 | 1 (vtep store module) | 3 (enrollment, lifecycle, store) | ~400 | none |
| 9 | 3 (test harness + scenarios) | 0 | ~600 | `testcontainers` or custom |
| 10 | 0 | 4 (config, metrics, docs) | ~300 | none |

**Total estimated: ~6,200 lines of new/modified Rust + ~200 lines of eBPF C + ~400 lines of proto.**

---

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|-----------|
| eBPF kernel version dependency | Won't load on older kernels | CO-RE format + fallback to nftables rules (Phase 6.4) |
| CH live migration API instability | Send/receive may change | Pin CH version, test against known release |
| Dirty tracking overhead | Slower volume I/O during migration | Userspace bitmap is lightweight; benchmark in Phase 2 |
| VXLAN MTU mismatch across nodes | Packet drops, fragmentation | CP validates underlay MTU during enrollment (Phase 8) |
| Large volume migration timeout | 100GB+ takes hours | Configurable timeouts, forced cutover after max_rounds |
| CP crash during migration | Orphaned partial state | Reaper marks Failed on restart, dest cleanup via heartbeat |
