# Task Plan: Implement All Remaining Gaps (Round 3)

## Goal
Fix 7 CRITICAL + 20 highest-impact HIGH findings to push the platform from 69% to ~82% completeness, focusing on runtime wiring gaps (code exists but isn't connected to production paths).

## Branch
`feat/production-gaps-round3`

## Phases
- [x] Phase 1: Research (3 exploration subagents completed)
- [ ] Phase 2: Implement via 5 parallel subagents
- [ ] Phase 3: Verify compilation
- [ ] Phase 4: Commit and deliver

---

## Execution Streams (5 Parallel Subagents)

### Stream A: Control Plane Critical Fixes (C1, C2, H2, H3, H7, H9)

**Files to modify**:
- `crates/chv-controlplane-store/src/observed_state.rs` (lines 174-243)
- `crates/chv-agent-core/src/connectivity.rs` (line 92)
- `crates/chv-agent-core/src/reconcile.rs`
- `crates/chv-controlplane-service/src/upgrade.rs`

**Tasks**:

1. **C1: Enforce generation check in SQL** — In `observed_state.rs`, fix all 4 ACK_*_OBSERVED_GENERATION_SQL queries:
   - `ACK_NODE_OBSERVED_GENERATION_SQL` (line 174): Add `AND observed_generation <= $2` to WHERE
   - `ACK_VM_OBSERVED_GENERATION_SQL` (line 182): Add generation comparison to ON CONFLICT clause  
   - `ACK_VOLUME_OBSERVED_GENERATION_SQL` (line 203): Same pattern
   - `ACK_NETWORK_OBSERVED_GENERATION_SQL` (line 224): Same pattern
   - Reference: desired_state.rs line 73 shows the correct pattern

2. **C2: Implement reconnect flush** — In `connectivity.rs` `record_success()` (line 92):
   - Return a bool indicating state changed from Disconnected→Connected
   - In the agent reconcile loop, when record_success() returns true, call `client.flush_pending_messages(&mut cache)`
   - `flush_pending_messages()` already exists at `control_plane.rs:276`

3. **H2: State transition guards** — Add a `valid_transitions()` function:
   - Define valid state transitions as a HashMap or match
   - In `set_node_state()`, reject invalid transitions with `ChvError::failed_precondition`

4. **H3: Compound readiness for TenantReady** — Before transitioning to TenantReady, verify StorageReady AND NetworkReady have been achieved

5. **H7: Wire SystemdNodeUpgrader** — In `upgrade.rs`, replace DummyUpgrader with SystemdNodeUpgrader:
   - `UpgradeOrchestrator::new()` should use `with_systemd_upgrader()` (exists at `systemd_upgrader.rs:390`)
   - Keep DummyUpgrader only in `#[cfg(test)]` blocks

6. **H9: NodeCache add volume/network fragments** — Add volume_state and network_state fields to NodeCache struct

---

### Stream B: Storage + Migration Critical Fixes (C3, C4, H15, H16, H14)

**Files to modify**:
- `cmd/chv-stord/src/main.rs` (line 40)
- `crates/chv-agent-core/src/migration.rs` (line 467-487)
- `crates/chv-controlplane-service/src/migration.rs` (line 562)
- `crates/chv-stord-backends/src/local.rs` (line 789)

**Tasks**:

1. **C3: Report dirty_blocks_remaining** — In `crates/chv-agent-core/src/migration.rs`:
   - The `build_progress()` function exists (line 467) but is never called outside tests
   - In the migration orchestration loop, call `build_progress()` with actual dirty count from stord
   - Wire agent's control plane client to call `report_migration_progress()` periodically

2. **C4: Wire backend selection** — In `cmd/chv-stord/src/main.rs` line 40:
   - Replace hardcoded `LocalFileBackend::new(...)` with config-based factory:
   ```rust
   let backend: Box<dyn StorageBackend> = match config.backend_type.as_deref() {
       Some("iscsi") => Box::new(IscsiBackend::new(config.iscsi.clone().unwrap_or_default())?),
       Some("ceph") => Box::new(CephRbdBackend::new(config.ceph.clone().unwrap_or_default())?),
       Some("lvm") => Box::new(LvmBackend::new(config.lvm.clone().unwrap_or_default())?),
       _ => Box::new(LocalFileBackend::new(config.runtime_dir.clone())),
   };
   ```

3. **H15: FinalSync coordination with VM pause** — In `crates/chv-controlplane-service/src/migration.rs` line 562:
   - Before Paused phase transition, issue `source_client.pause_vm(vm_id)` RPC
   - Wait for pause confirmation before sending FinalSync
   - Add timeout for pause operation (30s)

4. **H16: Atomic dirty bitmap snapshot** — In `crates/chv-stord-backends/src/local.rs`:
   - Add `snapshot_and_clear_dirty_bitmap()` method that acquires write lock once
   - Clones bitmap AND resets to zeros atomically under a single write lock
   - Update sender to use this instead of separate get/clear calls

5. **H14: IOPS enforcement stub** — Add cgroup blkio integration points:
   - In StorageBackend trait, add `set_io_limits(iops: u64, bandwidth_mbps: u64)` method
   - Implement in LocalFileBackend using cgroup v2 io.max writes

---

### Stream C: Network + eBPF Critical Fixes (C5, C6, H24, H26, H27)

**Files to modify**:
- `crates/chv-nwd-core/src/server.rs` (lines 31-107)
- `crates/chv-nwd-core/src/handlers.rs` (lines 313-320)
- `crates/chv-nwd-core/src/store.rs` (lines 17-55)
- `crates/chv-nwd-core/src/ebpf.rs`

**Tasks**:

1. **C5: Wire link_monitor to event loop** — In `server.rs` `NetworkServer::serve()`:
   - After line 101, spawn `link_health_loop()` as background tokio task
   - Pass interface list from config, 30s interval, shutdown channel
   - `link_health_loop()` already exists at `link_monitor.rs:186-208`

2. **C6: eBPF failure → error, not warning** — In `handlers.rs` lines 313-320:
   - Change `tracing::warn!` to `tracing::error!`
   - Track eBPF load failure state in handler
   - Report eBPF failure in `get_network_health()` response (line 216-237)
   - Return error from attach_vm_nic if eBPF is required but fails (configurable)

3. **H24: Persist peer_vteps to SQLite** — In `store.rs`:
   - Add `peer_vteps TEXT` column to topologies table
   - Serialize peer_vteps as JSON in `upsert()` (line 38-48)
   - Deserialize from JSON in `list()` (line 76-86)
   - Add migration for existing databases

4. **H26: VNI range validation** — In `handlers.rs` line 167:
   - Add check: `if spec.vni > 16777215 { return error }`
   - Also validate in `executor.rs::create_vxlan_interface()`

5. **H27: eBPF denied counter** — In `ebpf.rs`:
   - Track denied packet count from BPF maps
   - Expose in overlay status metrics

---

### Stream D: Agent + Observability Fixes (C7, H10, H11, H42, M3)

**Files to modify**:
- `crates/chv-agent-core/src/health.rs`
- `crates/chv-agent-core/src/metrics_server.rs`
- `crates/chv-agent-core/src/reconcile.rs`

**Tasks**:

1. **C7: Disk full detection** — In `health.rs`:
   - Add resource pressure constants: `MIN_DISK_AVAILABLE_GB = 5`, `MAX_MEMORY_USAGE_PCT = 90.0`
   - Add `check_host_resources()` function using sysinfo (already imported in metrics_server)
   - In `HealthAggregator::derive_node_state()`, transition to Degraded on pressure
   - Wire into the agent tick loop

2. **H10: Emit metrics constants** — In `metrics_server.rs`:
   - Define named constants for all metric names at module top
   - Add missing metrics that the reconciler should emit:
     - `chv_agent_reconcile_drift_ema` (convergence tracking)
     - `chv_agent_reconcile_duration_seconds` (histogram)
   - Wire reconcile loop to update these in MetricsState

3. **H11: operation_id in spans** — In reconcile/migration code:
   - Generate UUID operation_id per reconcile tick
   - Add as field to tracing spans: `tracing::info_span!("reconcile", operation_id = %op_id)`
   - Propagate through control plane RPCs

4. **H42: Memory pressure detection** — Same module as C7:
   - Check `memory_used / memory_total > MAX_MEMORY_USAGE_PCT`
   - Transition to Degraded on threshold breach

5. **M3: Draining evacuation** — In `reconcile.rs` Draining match arm:
   - Iterate VMs on this node
   - For each Running VM, enqueue migration request to control plane
   - Track drain progress; transition to Maintenance when VM count = 0

---

### Stream E: Operations + CLI Fixes (H41, H6, M16, M18, M19)

**Files to modify**:
- `cmd/chvctl/src/commands/mod.rs`
- `cmd/chvctl/src/main.rs`
- `crates/chv-controlplane-service/src/bootstrap.rs` (or startup code)

**Tasks**:

1. **H41: Add missing chvctl subcommands** — In `cmd/chvctl/src/commands/`:
   - Create `storage.rs`: list-pools, show-pool, create-pool, delete-pool
   - Create `migrate.rs`: start, status, cancel, history
   - Create `upgrade.rs`: start, status, rollback, history
   - Create `health.rs`: check, report, history
   - Register all in `mod.rs` and wire to main CLI

2. **H6: Compat matrix boot gate** — In bootstrap/startup:
   - After DB init, load compat matrix from config
   - Call `matrix.check_all()` — fail startup on incompatible versions
   - Change from warn-only to hard error

3. **M16: chvctl Unix socket mode** — Add `--socket /path` flag:
   - When provided, connect to local agent Unix socket instead of HTTP
   - Useful for debug commands without network

4. **M18/M19: Compat matrix completeness** — Add nwd and stord to matrix:
   - Update the default compat.toml to include all 5 components
   - Ship the file in release tarball

---

## Key Decisions

- Generation check: Use same pattern as desired_state.rs (proven working)
- Reconnect flush: Return bool from record_success(), caller flushes
- eBPF failure: Error + degraded health (not hard failure) — allows degraded operation
- Backend selection: Config-based factory, LocalFile as default fallback
- Dirty bitmap: Single write-lock atomic snapshot+clear
- CLI subcommands: Scaffold with list/show/create/delete patterns matching existing commands
- Disk pressure: 5GB min free, 90% memory max — configurable via agent.toml

## Verification

1. `cargo check --workspace` — no errors
2. `cargo clippy --workspace -- -D warnings` — clean
3. `cargo test --workspace` — all pass

## Status
**Currently in Phase 2** — About to dispatch 5 parallel implementation subagents
