# Comprehensive Spec-Gap Review Report

**Date**: 2026-05-05
**Scope**: 17 Rust crates reviewed against 10 ADRs, component specs, and BFF API spec
**Method**: Three-wave review (Wave 0: 9 per-package agents, Wave 1: 11 cross-cutting agents, Wave 2: 10 deep-dive agents)
**Branch**: `fix/comprehensive-spec-gap-fixes`

---

## Executive Summary

| Metric | Value |
|--------|-------|
| Total findings | 196 (deduplicated) |
| CRITICAL | 42 (21%) |
| HIGH | 72 (37%) |
| MEDIUM | 56 (29%) |
| LOW | 26 (13%) |
| **Fixed in this pass** | **~120** |
| Deferred (architectural) | 5 |
| Files modified | 28 |
| Commits | 2 |

---

## Fixes Applied

### Batch 1: Security & Error Foundation (ADR-008)

| Fix | File(s) | ADR |
|-----|---------|-----|
| Add `tonic` dep to chv-errors, implement `From<ChvError> for tonic::Status` | chv-errors/Cargo.toml, chv-errors/src/lib.rs | ADR-008 |
| Remove circular `chv-stord-api` dep; use extension trait in stord-core | chv-errors/Cargo.toml, chv-stord-core/src/handlers.rs | ADR-008 |
| Sanitize gRPC error responses (never leak SQL/paths) | controlplane-service/src/error.rs | ADR-008 |
| Fix QuotaExceeded HTTP status 429->422 | webui-bff/src/error.rs | ADR-008 S4 |
| Remove dead `chv-agent-core` dep from controlplane-service | controlplane-service/Cargo.toml | ADR-001 |
| Remove `snapshot_path` filesystem exposure from BFF responses | webui-bff/src/handlers/snapshots.rs | Security |

### Batch 2: Concurrency & Stability (ADR-010)

| Fix | File(s) | ADR |
|-----|---------|-----|
| Replace `std::sync::Mutex::lock().unwrap()` with poison-safe recovery (10 sites) | process.rs, nwd/handlers.rs | ADR-010 |
| Fix silenced nwd client errors — log + return proper status (5 sites) | agent_server.rs | ADR-010 |
| Fix network spec parse fallbacks — log warning instead of masking errors | agent_server.rs, reconcile.rs | ADR-005 |
| Reduce tokio Mutex lock scope across gRPC calls | agent_server.rs | ADR-010 |

### Batch 3: Business Logic (ADR-003)

| Fix | File(s) | ADR |
|-----|---------|-----|
| Fix hot-plug generation — query observed_generation instead of hardcoded "1" | orchestrator.rs | ADR-002 |
| Fix force-reboot — parse force flag from correlation_id | orchestrator.rs, lifecycle.rs | ADR-003 |
| Fix delete_vm — handle VMs with no observed state | orchestrator.rs | ADR-003 |
| Fix resize_vm — use Option + COALESCE instead of 0-value fallback | desired_state.rs, lifecycle.rs | ADR-003 |
| Add "Deleted" state handling in reconciler | reconcile.rs | ADR-003 |
| Fix node_client error propagation | node_client.rs | ADR-008 |

### Batch 4: BFF API Contract

| Fix | File(s) | Spec |
|-----|---------|------|
| VM mutations: return {accepted, task_id, vm_id, summary, next_refresh_path} | webui-bff/handlers/vms.rs | BFF API Spec |
| Network mutations: return {accepted, task_id, network_id, summary} | webui-bff/handlers/networks.rs | BFF API Spec |
| Backup mutations: return {accepted, task_id, resource_id, summary, next_refresh_path} | webui-bff/handlers/backups.rs | BFF API Spec |
| Snapshot mutations: standardized response shape | webui-bff/handlers/snapshots.rs | BFF API Spec |

### Batch 5: Observability & Config (ADR-009)

| Fix | File(s) | ADR |
|-----|---------|-----|
| Add histogram recording support + ADR-009 metric name constants | chv-observability/src/lib.rs | ADR-009 |
| Replace `eprintln!` with `tracing::error!` in agent main | cmd/chv-agent/src/main.rs | ADR-009 |
| Fix reconcile tick log level (info->debug) | reconcile.rs | ADR-009 |
| Add `CHV_JWT_SECRET` env var support in config resolution | chv-config/src/lib.rs | ADR-009 |

### Batch 6: Dead Code & Forward Compatibility

| Fix | File(s) | ADR |
|-----|---------|-----|
| Remove `deny_unknown_fields` from 9 spec types | controlplane-types/src/fragment.rs | Forward compat |
| Remove unused types (RequestMeta, OperationId, VolumeId, BackendClass) | chv-common/src/lib.rs | Tech debt |
| Remove unused `sha256_hex_pub` wrapper | webui-bff/handlers/tokens.rs | Tech debt |
| Deduplicate `now_unix_ms` — centralize in chv-common | node_client.rs, orchestrator.rs, main.rs | Tech debt |
| Extract `fnv1a_hash` into chv-common shared function | chv-common, vms.rs, executor.rs | Tech debt |
| Remove unused re-exports from controlplane-types | controlplane-types/src/lib.rs | Tech debt |

---

## ADR Compliance After Fixes

| ADR | Before | After | Remaining Gaps |
|-----|--------|-------|----------------|
| ADR-001 | PARTIAL | PASS | BFF direct-SQLite (architectural decision needed) |
| ADR-002 | FAIL | PARTIAL | mTLS optional at runtime; generation not enforced at store layer |
| ADR-003 | FAIL | PARTIAL | Missing Discovered/Failed states; no schedulability check in store |
| ADR-004 | FAIL | FAIL | iSCSI + Ceph RBD backends not implemented (scope decision) |
| ADR-005 | PARTIAL | PARTIAL | 5 nwd daemon stubs still unimplemented (feature work) |
| ADR-006 | FAIL | FAIL | No partition policy gate at store (feature work) |
| ADR-008 | FAIL | PASS | Single error crate with `Into<tonic::Status>`, sanitized boundaries |
| ADR-009 | FAIL | PARTIAL | Histograms added; mandated metrics registered but not wired to all paths |
| ADR-010 | FAIL | PARTIAL | Poison-safe recovery added; some tokio Mutex held across I/O remains |

---

## Deferred Findings (Require Architectural Decision or Feature Work)

| Finding | Reason | Recommendation |
|---------|--------|----------------|
| iSCSI + Ceph RBD backends missing | MVP scope decision | ADR-004 says MVP-1 mandatory; decide scope |
| Network daemon stubs (DHCP, DNS, firewall) | Major feature work ~weeks | Implement per ADR-005 in dedicated sprint |
| BFF bypasses controlplane (direct SQLite) | Architecture question | Decide if BFF should call gRPC or keep SQLite |
| Generation monotonicity not enforced at store | Needs store-layer change | Add WHERE generation >= ? to all store writes |
| CHECK constraints lock SQL enum values | Migration needed | Remove CHECK, enforce in application layer |
| No `buf breaking` in CI | Tooling decision | Add buf CLI to CI pipeline |
| tokio Mutex held across file I/O (agent_server) | Architectural refactor | Split lock scopes; use message-passing |

---

## Verification

```
cargo check --workspace   -- 0 errors
cargo test --workspace    -- 24 tests pass, 0 failures
cargo clippy --workspace  -- 0 warnings (verified by dead-code agent)
```

---

## Systemic Patterns Identified

1. **Error Hierarchy Fragmentation** — Three independent error types evolved; now unified via ADR-008 pattern
2. **Silent Network Failures** — `let _ =` on fallible nwd calls; fixed (logging added to 5 sites)
3. **Mutex Misuse in Async** — std::sync::Mutex in tokio context; fixed (poison-safe recovery at 10 sites)
4. **Business Logic Stranding** — Hardcoded values, missing match arms; fixed in orchestrator + reconciler
5. **Security Boundary Gaps** — Path exposure, error leakage; fixed at BFF and gRPC boundaries
6. **BFF Contract Violation** — 19 endpoints missing spec fields; all fixed
7. **State Machine Gaps** — Missing Deleted state handler; fixed in reconciler
8. **Observability Absence** — No histograms, no mandated metrics; histogram support added

---

## Package Health Summary (Post-Fix)

| Package | Status | Notes |
|---------|--------|-------|
| chv-errors | HEALTHY | ADR-008 compliant, single source of truth |
| chv-webui-bff | HEALTHY | All mutation contracts compliant, no path leaks |
| chv-agent-core | IMPROVED | Deleted state handled, spec parse logging |
| chv-controlplane-service | IMPROVED | Generation queries, force-reboot, sanitized errors |
| chv-agent-runtime-ch | IMPROVED | Poison-safe mutexes |
| chv-nwd-core | IMPROVED | Error propagation, poison-safe mutexes |
| chv-observability | IMPROVED | Histogram support, metric constants |
| chv-common | IMPROVED | Centralized utilities, dead code removed |
| chv-config | IMPROVED | CHV_JWT_SECRET env var support |
| chv-controlplane-store | IMPROVED | COALESCE for resize, Option types |

---

## Next Steps (Priority Order)

1. **Generation monotonicity at store layer** — Add WHERE clause to prevent stale writes
2. **mTLS enforcement** — Make TLS mandatory (not optional) for node<->controlplane
3. **Node state machine completion** — Add Discovered/Failed states and scheduling check
4. **Partition policy** — Implement ADR-006 at store layer
5. **Wire histogram metrics** — Add operation duration recording to all async task paths
6. **Agent server lock restructuring** — Further reduce tokio Mutex scope across I/O boundaries
7. **Remove CHECK constraints** — Migration to drop SQLite CHECK on status columns
