# Live Migration Orchestration Spec

## Purpose
Orchestrates full VM live migration between two nodes, coordinating disk pre-copy, memory migration via Cloud Hypervisor, and post-migration validation. Runs as part of the single control plane orchestrator.

## Owner
chv-controlplane-service (orchestrator module)

## Scope
- Coordinates: source agent, destination agent, source stord, destination stord, source nwd, destination nwd
- Does NOT: perform actual data transfer (stord does that), manage VXLAN tunnels (nwd does that)

## State Machine

```
                    ┌──────────────────────────────────────────────┐
                    │                                              │

  ┌─────────┐    ┌──▼──────────┐    ┌──────────────┐    ┌────────────────┐
  │ Pending  │───►│ PreCopyDisk │───►│ConvergingDisk│───►│MemoryMigration │
  └─────────┘    └─────────────┘    └──────────────┘    └────────────────┘
                       │                    │                     │
                       │ (fail)             │ (fail)              │ (fail)
                       ▼                    ▼                     ▼
                  ┌──────────┐        ┌──────────┐         ┌──────────┐
                  │RolledBack│        │RolledBack│         │  Failed  │
                  └──────────┘        └──────────┘         └──────────┘

    ┌────────────────┐    ┌────────────┐    ┌───────────┐
    │MemoryMigration │───►│   Paused   │───►│ Completed │
    └────────────────┘    └────────────┘    └───────────┘
                               │
                               │ (fail)
                               ▼
                          ┌──────────┐
                          │RolledBack│
                          └──────────┘
```

## Phases

### Phase 1: PreCopyDisk
**Trigger:** MigrateVm operation dispatched by orchestrator

**Actions:**
1. CP validates: source node is TenantReady, dest node is TenantReady, VM is Running
2. CP validates: dest node has sufficient resources (CPU, memory, disk)
3. CP instructs dest stord: prepare receiving volume (same size, same format as source)
4. CP instructs dest nwd: join VNI for VM's network (if not already participating)
5. CP instructs source stord: enable dirty block tracking on VM's volume
6. CP instructs source stord: begin block export stream to dest stord (gRPC streaming)
7. Progress: reported via source agent heartbeat (bytes_transferred, total_bytes, phase)

**Exit conditions:**
- Success: bulk copy complete (all blocks transferred at least once) → ConvergingDisk
- Failure: dest volume creation fails, stream error, timeout → RolledBack

### Phase 2: ConvergingDisk
**Actions:**
1. Source stord reads dirty bitmap, streams only dirty blocks to dest
2. After each round: clear bitmap, wait for next round of writes
3. CP monitors: dirty_block_count from source stord (reported in heartbeat)
4. Convergence check: if dirty_blocks < threshold (configurable, default: 1024 blocks = 4GB at 4MB block size) → proceed

**Exit conditions:**
- Success: dirty rate below threshold → MemoryMigration
- Forced: max_rounds reached (default 10) → MemoryMigration (forced cutover)
- Failure: stream error, source node unreachable, timeout → RolledBack

### Phase 3: MemoryMigration
**Actions:**
1. CP instructs dest agent: open CH migration receiving socket (TCP, port from pool)
2. Dest agent calls CH API: `PUT /api/v1/vm.receive-migration` with `{"receiver_url": "tcp://0.0.0.0:{port}"}`
3. CP instructs source agent: start memory migration to dest socket
4. Source agent calls CH API: `PUT /api/v1/vm.send-migration` with `{"receiver_url": "tcp://{dest_ip}:{port}"}`
5. CH handles iterative memory pre-copy (dirty pages tracked internally by CH)
6. CP monitors: CH converges memory (optional: poll `/api/v1/vm.dirty-log` for page count)

**Exit conditions:**
- Success: CH reports migration entering final phase (VM will pause) → Paused
- Failure: dest agent unreachable, CH API error, timeout → Failed (NOT RolledBack — partial state may exist on dest)

### Phase 4: Paused (Final Sync)
**Actions:**
1. CH pauses VM on source (automatic as part of send-migration completion)
2. Source stord: flush final dirty blocks to dest (last bitmap read + stream)
3. Dest stord: acknowledges all blocks received
4. CH: final memory pages transferred to dest
5. Dest agent: calls CH `PUT /api/v1/vm.resume` (or CH auto-resumes after receive completes)
6. VM is now running on destination

**Exit conditions:**
- Success: VM confirmed running on dest → Completed (proceed to Phase 5)
- Failure: dest fails to resume, final sync error → RolledBack (source still has everything)

### Phase 5: Completed (Validation and Cleanup)
**Actions:**
1. CP waits for dest agent heartbeat confirming VM state = Running
2. Dest nwd: sends gratuitous ARP for VM's IP/MAC
3. CP updates SQLite: VM placement → dest node
4. CP updates SQLite: operation status → Completed
5. CP instructs source: cleanup (delete source volume copy, release resources)
6. CP instructs source nwd: remove FDB entries for migrated VM's MAC (if only VM on that network)

**Note:** This phase is post-completion. Failures here do not affect the VM (already running on dest). Logged as warnings.

## Rollback Behavior

| Failure Point | Action | VM Continuity |
|---|---|---|
| Phase 1 (PreCopyDisk) | Abort stream, delete dest volume, disable dirty tracking on source | VM never stopped, continues on source |
| Phase 2 (ConvergingDisk) | Same as Phase 1 | VM never stopped, continues on source |
| Phase 3 (MemoryMigration) | Cannot cleanly rollback if CH is mid-transfer. Mark Failed. | Manual recovery required |
| Phase 4 (Paused) | If dest fails: source still has all data. Resume on source. | Brief pause experienced by VM |

## Timeouts

| Phase | Default Timeout | Calculation |
|---|---|---|
| PreCopyDisk | disk_size_gb * 60s | 100GB disk = ~100 min at 1Gbps |
| ConvergingDisk | 300s per round, total 3000s | 10 rounds max |
| MemoryMigration | memory_size_gb * 30s + 120s | 16GB RAM = ~10 min |
| Paused (final sync) | 60s | Should be fast (small residual) |
| Total | sum of above + 300s buffer | Hard abort if exceeded |

## Required Proto Messages

```protobuf
message MigrateVmRequest {
  string vm_id = 1;
  string source_node_id = 2;
  string destination_node_id = 3;
  MigrationConfig config = 4;
}

message MigrationConfig {
  uint32 dirty_threshold_blocks = 1;  // default: 1024
  uint32 max_convergence_rounds = 2;  // default: 10
  uint32 block_size_bytes = 3;        // default: 4194304 (4MB)
  uint32 total_timeout_seconds = 4;   // 0 = use calculated default
}

message MigrationProgress {
  string vm_id = 1;
  string operation_id = 2;
  MigrationPhase phase = 3;
  uint64 bytes_transferred = 4;
  uint64 total_bytes = 5;
  uint32 convergence_round = 6;
  uint64 dirty_blocks_remaining = 7;
  float progress_percent = 8;
}

enum MigrationPhase {
  MIGRATION_PHASE_UNSPECIFIED = 0;
  MIGRATION_PHASE_PENDING = 1;
  MIGRATION_PHASE_PRECOPY_DISK = 2;
  MIGRATION_PHASE_CONVERGING_DISK = 3;
  MIGRATION_PHASE_MEMORY_MIGRATION = 4;
  MIGRATION_PHASE_PAUSED = 5;
  MIGRATION_PHASE_COMPLETED = 6;
  MIGRATION_PHASE_FAILED = 7;
  MIGRATION_PHASE_ROLLED_BACK = 8;
}
```

## Operation Integration

- New operation type: `MigrateVm` in orchestrator dispatch table
- Operation follows existing retry/timeout pattern (but migration is NOT retried automatically — failure requires operator review)
- Operation reaper: if migration operation stale > 2x total_timeout, mark Failed and alert

## Configuration

| Parameter | Default | Description |
|---|---|---|
| migration.dirty_threshold_blocks | 1024 | Blocks remaining before forcing memory migration phase |
| migration.max_convergence_rounds | 10 | Maximum dirty sync iterations |
| migration.block_size_bytes | 4194304 | 4MB chunks for block streaming |
| migration.memory_migration_port_range | 49152-49200 | TCP port pool for CH migration sockets |
| migration.total_timeout_multiplier | 1.5 | Multiplier on calculated timeout |

## Non-goals
- Automatic retry of failed migrations (operator must review)
- Multi-VM batch migration (one at a time per orchestrator)
- Post-copy disk fallback (v1 is pre-copy only)
- Cross-controlplane migration (each CP manages its own cluster)

## Recovery model
- If CP crashes during migration: on restart, find in-progress migration operations, mark them Failed (cannot safely resume mid-migration)
- If source agent crashes during Phase 1-2: mark Failed, dest cleans up partial volume on next heartbeat
- If dest agent crashes during Phase 3-4: mark Failed, source resumes VM (manual cleanup of dest)
