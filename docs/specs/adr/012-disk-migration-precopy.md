# ADR-012 — Block-Level Disk Migration (Pre-Copy)

## Status
Accepted

## Date
2026-05-07

## Context
Live migration requires moving VM disks between nodes. v1 has no shared storage (Ceph/NFS deferred to a future version). Each node uses local storage backends (file-based or LVM).

The existing `StorageBackend` trait has open, close, attach, detach, snapshot, and clone operations but no export or import methods. Cloud Hypervisor supports live memory migration via `/api/v1/vm.send-migration` and `/api/v1/vm.receive-migration` (TCP socket streaming). Disk migration must happen before or during memory migration; post-copy disk (reading from source while the VM runs on destination) is fragile and deferred.

Hot-plug and unplug of virtio-block devices is already supported via the CH adapter.

## Decision
- Pre-copy disk migration: stream all disk blocks to the destination while the VM continues running on the source
- Dirty block tracking via bitmap at the source stord: 1 bit per block (block_size configurable, default 4 MB)
- Iterative convergence: after bulk copy, re-send dirty blocks in rounds until dirty_blocks < threshold or max_rounds (default 10) is reached
- After disk converges: CH live-migrates memory via vm.send-migration / vm.receive-migration
- Final phase: brief VM pause, flush remaining dirty disk blocks and final memory pages, resume on destination
- Inter-node transport: gRPC bidirectional streaming between stord daemons via a new StorageMigrationService
- Authentication: mTLS using existing agent certificates
- New `StorageBackend` trait methods required:
  - `export_blocks(volume_id, block_range) -> Stream<BlockChunk>`
  - `import_blocks(volume_id, stream: Stream<BlockChunk>)`
  - `enable_dirty_tracking(volume_id) -> Result<()>`
  - `get_dirty_bitmap(volume_id) -> Result<Bitmap>`
  - `clear_dirty_bitmap(volume_id) -> Result<()>`
- Shared storage (Ceph, NFS) is a future optimization where disk migration becomes unnecessary because both nodes access the same volume
- Rollback: if migration fails before VM pause, source continues normally (disk unchanged); if it fails during final sync, source resumes (still holds complete state)

## Consequences
Pros:
- Works with any local storage backend (file, LVM, future backends)
- No shared storage infrastructure required
- Downtime limited to the convergence phase (sub-second for low write-rate workloads)
- Rollback is straightforward (source never loses its copy until cleanup)
- Migration can be monitored and cancelled at any phase

Cons:
- Total migration time is proportional to disk size and write rate
- Source node must remain healthy until migration completes
- High-write-rate VMs may never converge (forced cutover after max_rounds)
- Double storage consumption during migration (both nodes hold a copy)
- Dirty block tracking adds overhead to volume writes during migration
- Network bandwidth between nodes is consumed by block streaming

## Guardrails
- Source MUST NOT delete its volume until the control plane confirms the destination VM is running
- Dirty tracking MUST be disabled after migration completes (success or failure)
- Block streaming MUST respect backpressure from the destination
- Each block chunk MUST include an integrity check (CRC32)
- Migration MUST be cancellable at any phase (control plane can abort)
- If the source node fails during migration, mark the migration as Failed (no automatic recovery in v1)

## Related ADRs
- **ADR-011** (single-node-CP): migration orchestration lives in the single control plane
- **ADR-013** (network-overlay): network continuity after disk and memory migration
- **ADR-006** (partition-policy): migrations are denied during control-plane partition
- **ADR-004** (storage-datapath): defines the `StorageBackend` trait being extended
