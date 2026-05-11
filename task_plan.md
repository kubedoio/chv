# Task Plan: Implement All P0-P3 Production Gaps

## Goal
Fix all 21 findings from the product readiness gap analysis (2 CRITICAL, 8 HIGH, 11 MEDIUM) to bring the platform from 67% to ~90% completeness.

## Branch
`feat/production-gaps-p0-p3`

## Phases
- [x] Phase 1: Research and plan (identify all files, assess complexity)
- [ ] Phase 2: Implement via parallel subagents (6 streams)
- [ ] Phase 3: Verify compilation and tests
- [ ] Phase 4: Commit and deliver

## Execution Streams (Parallel Subagents)

### Stream A: Agent Core (C1 + M3 + M9 + host monitoring H6)
**Agent**: general-purpose
**Files**: `crates/chv-agent-core/src/agent_server.rs`, `crates/chv-agent-core/src/metrics_server.rs`
**Tasks**:
1. C1: Wire ConnectivityTracker into create_vm/migrate_vm — reject if Disconnected
2. M9: Same fix (tracker gating is M9)
3. M3: Add host resource metrics (CPU/mem/disk) to metrics_server.rs (sysinfo crate)
4. H6: Same as M3 (host resource monitoring)

### Stream B: Storage & Migration (C2 + H1 + H2)
**Agent**: general-purpose
**Files**: `crates/chv-controlplane-service/src/migration.rs`, `crates/chv-stord-backends/src/`
**Tasks**:
1. C2: Implement real `disable_source_dirty_tracking()` via agent RPC
2. H1: Implement iSCSI StorageBackend (struct + trait impl)
3. H2: Implement Ceph RBD StorageBackend (struct + trait impl)

### Stream C: Network / eBPF (H3 + M1)
**Agent**: general-purpose
**Files**: `crates/chv-nwd-core/src/ebpf.rs`, `crates/chv-nwd-core/src/`
**Tasks**:
1. H3: Replace eBPF stub methods with real libbpf-rs BPF map operations
2. M1: Add basic link health/BFD monitoring module to nwd

### Stream D: Operational / Release (H4 + H5 + M4 + M5 + M10 + M11)
**Agent**: general-purpose
**Files**: `scripts/build-release.sh`, `crates/chv-controlplane-service/src/`
**Tasks**:
1. H5: Add chvctl to release tarball (build-release.sh)
2. H4: Add upgrade/rollback orchestration framework (new module)
3. M4: Add failure matrix circuit breaker automation
4. M5: Add compatibility matrix version enforcement
5. M10: Add cert rotation helper to chv-config
6. M11: Progressive migration timeouts (replace flat 7200s)

### Stream E: WebUI Gaps (H7 + H8 + M6 + M7 + M8)
**Agent**: general-purpose
**Files**: `ui/src/routes/vms/[id]/+page.svelte`, `ui/src/routes/images/+page.svelte`, `ui/src/lib/bff/`
**Tasks**:
1. H7: Add events and metrics tabs to VM detail page
2. H8: Fix images delete to use BFF client
3. M6: Implement list_volumes BFF handler
4. M7: Implement list_storage_pools BFF handler
5. M8: Add type/resource filter dropdowns to events page

### Stream F: Reconciler & Observability (M2 + ADR-002 convergence)
**Agent**: general-purpose
**Files**: `crates/chv-controlplane-service/src/reconcile.rs`, `crates/chv-agent-core/src/reconcile.rs`
**Tasks**:
1. M2: Add convergence metrics (drift count, convergence time) to reconciler
2. Expose reconciler metrics via CP health endpoint

## Key Decisions
- Use `sysinfo` crate for host resource monitoring (CPU/mem/disk) — lightweight, cross-platform
- Use `libbpf-rs` for real BPF map operations (already a dependency pattern in nwd)
- iSCSI backend: use `open-iscsi` CLI tooling via Command (industry standard)
- Ceph RBD backend: use `librbd` via `rados-rs` or CLI `rbd` commands
- Upgrade framework: rolling-upgrade orchestrator in controlplane-service
- Cert rotation: add `CertWatcher` that reloads TLS certs on SIGHUP or file change

## Errors Encountered
- (none yet)

## Status
**Currently in Phase 2** — Dispatching parallel subagents
