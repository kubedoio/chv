# Task Plan: Sprint 6 — VM Runtime Correctness

## Goal

Fix VM path uniqueness, per-VM log directories, and add CH API support for runtime operations (stop/start/resize/hotplug).

## Phases
- [ ] Phase 1: Research current state
- [x] Phase 2: Plan approach
- [ ] Phase 3: Implement
- [ ] Phase 4: Verify and deliver

## Tasks

### T1: VM runtime paths must use UUID, not hostname/name (P1 correctness)

**Problem:** `reconcile.rs:347` constructs socket paths as `/run/chv/agent/vm-{vm_id}.sock`.
The `vm_id` IS already a UUID in the database. However, there are test fixtures using
`vm-1`, `vm-orphan` style IDs. The real issue the user sees may be in the UI where
routes show `/vm/vm-hostname/xxx` patterns. Need to verify.

**Actual paths in production code:**
- `reconcile.rs:347`: `format!("/run/chv/agent/vm-{}.sock", vm_id)` — vm_id is UUID, correct
- Test fixtures use non-UUID IDs (`vm-1`, `vm-orphan`) — acceptable for tests
- UI routes use `[id]` param which comes from `vm_id` (UUID) — correct

**Action:** Audit all places where VM names (display_name) are used where IDs should be.
The `attached_vm_name` fields in volumes are display values (correct for UI display).
Check if any URL construction or file path uses `display_name` instead of `vm_id`.

### T2: Per-VM log subdirectories in agent runtime (P2 operations)

**Problem:** Agent logs go to a single directory. There's no per-VM subdirectory structure
for logs, console output, or runtime artifacts.

**Solution:** Create `/run/chv/agent/vms/{vm_id}/` directory per VM containing:
- `console.log` — redirected serial output (for persistence after disconnect)
- `vm.sock` — API socket (move from flat `/run/chv/agent/vm-{id}.sock`)
- `vm.pid` — PID file for the CH process

Update `reconcile.rs` to create the subdirectory. Update `process.rs` to use it.

### T3: CH API operations — resize, hotplug, runtime control (P1 platform)

**Problem:** The `CloudHypervisorAdapter` trait only has create/start/stop/delete/reboot.
Cloud Hypervisor supports runtime operations via its HTTP API over the Unix socket:
- `PUT /api/v1/vm.resize` — change vCPUs and memory
- `PUT /api/v1/vm.add-device` — hotplug devices
- `PUT /api/v1/vm.remove-device` — hot-unplug devices
- `GET /api/v1/vm.info` — get VM state

**Solution:** Extend the `CloudHypervisorAdapter` trait with:
```rust
async fn resize_vm(&self, vm_id: &str, cpus: Option<u32>, memory_bytes: Option<u64>) -> Result<(), ChvError>;
async fn vm_info(&self, vm_id: &str) -> Result<VmInfo, ChvError>;
```

Implement via `ch_api_request` to the Unix socket (same pattern as start/stop).

### T4: Wire resize/info through agent server gRPC (P2 platform)

**Problem:** The agent gRPC server has `resize_volume` as unimplemented (Phase 4).
But there's no `resize_vm` or `vm_info` RPC at all.

**Solution:** Check proto definitions. If proto already defines these RPCs, implement
them. If not, add to proto and implement.

## Key Questions
1. Are VM IDs actually UUIDs in all code paths, or do some paths use display_name?
2. Does the proto define resize_vm or vm_info RPCs?
3. What CH API version are we targeting?

## Decisions Made
- Per-VM subdirectories: `/run/chv/agent/vms/{vm_id}/` is the new standard
- CH API operations go through the existing Unix socket HTTP pattern in process.rs
- Trait extension is backward-compatible (default implementations return unimplemented)

## Status
**Currently in Phase 3** - Starting implementation
