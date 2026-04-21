# TODOS

## Sprint 10 — Snapshots, Export/Import, Metrics

### Extend orchestrator to dispatch SnapshotVm/RestoreSnapshot ✅ DONE

**What:** Add `SnapshotVm` and `RestoreSnapshot` to `orchestrator.rs:110` dispatch match. Wire `NodeClient` to call agent `snapshot_vm` / `restore_snapshot` gRPC methods.

**Why:** The control plane lifecycle service creates operations but the orchestrator never dispatches them. Snapshots are dead on arrival.

**Context:** `crates/chv-controlplane-service/src/orchestrator.rs` handles `CreateVm`, `StartVm`, `StopVm`, etc. Add two new arms. The agent already accepts these RPCs (`agent_server.rs:1161`). See `process.rs:708` for the CH HTTP API call.

**Effort:** M → CC: S
**Priority:** P0
**Depends on:** None

### Wire BFF snapshot create to MutationService ✅ DONE

**What:** Replace direct DB insert in `create_snapshot` with a `mutations.mutate_vm(vm_id, "snapshot", ...)` call. Delete the standalone `create_snapshot` direct-DB path or repurpose it for metadata-only operations.

**Why:** BFF bypasses control plane for snapshots, violating the architecture boundary and leaving execution orphaned.

**Context:** `crates/chv-webui-bff/src/handlers/snapshots.rs:65` does direct SQL. `bff_mutations.rs:154` already supports `action: "snapshot"` but passes empty `destination`. Fix destination to use `snapshot_path`.

**Effort:** M → CC: S
**Priority:** P0
**Depends on:** TODO #1

### Build real vm_metrics pipeline ✅ DONE

**What:** New migration `0018_vm_metrics.sql` with `(vm_id, collected_at, cpu_percent, memory_bytes_used, memory_bytes_total, disk_bytes_read, disk_bytes_written, net_bytes_rx, net_bytes_tx)`. Agent polls CH `/api/v1/vm.counters` on every telemetry tick and includes counters in `VmStateReport`. Control plane stores them in `vm_metrics`. BFF `get_metrics` serves top consumers from latest snapshot per VM.

**Why:** `vm_observed_state` has no resource columns. The plan's claim is impossible.

**Context:** CH HTTP API exposes counters. `crates/chv-agent-runtime-ch/src/process.rs` already makes CH API calls. Model after `observed_state.rs` store patterns.

**Effort:** L → CC: M
**Priority:** P1
**Depends on:** None

### Add export/import backend with async tasks ✅ DONE

**What:** BFF handlers `export_vm` and `import_vm`. Export copies disk image to `/run/chv/exports/` and creates a task. Import validates qcow2 header, copies to `/run/chv/agent/vms/{id}/`, creates VM record. Return task ID for polling.

**Why:** UI stubs exist but endpoints return 404.

**Context:** `ui/src/lib/api/client.ts:727` expects `/api/v1/vms/{vmId}/export` and `/api/v1/vms/import`. Use `MutationService` or direct control plane RPC. Must stream large files, not buffer in memory.

**Effort:** L → CC: M
**Priority:** P1
**Depends on:** None

### Add snapshot UI tab with full state coverage ✅ DONE

**What:** `VmSnapshots.svelte` component. List table, create modal, delete confirm, restore confirm (with VM-stopped check). Handle loading, empty, error, success, partial/creating states.

**Why:** Backend exists (after TODO #1/#2) but no UI surface.

**Context:** Add tab to `ui/src/routes/vms/[id]/+page.svelte`. API client methods already exist (`client.ts:492`). Follow existing `SectionCard` + `InventoryTable` patterns.

**Effort:** M → CC: S
**Priority:** P1
**Depends on:** TODO #1, #2

### Harden export/import security ✅ DONE (included in #4)

**What:** Add file size limit (10GB default), path traversal validation on `export_id`, sanitize import filename, require `require_operator_or_admin` for export/import mutations.

**Why:** Export/Import introduces file upload/download attack surface.

**Context:** `axum` body limit for uploads. `validate_id()` for `export_id`. Check disk space before export.

**Effort:** S → CC: S
**Priority:** P2
**Depends on:** TODO #4

## Completed

