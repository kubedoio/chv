# Comprehensive Review Report

**Date**: 2026-05-04
**Branch**: `fix/comprehensive-review-wave3`
**Scope**: Full workspace (21 crates, 43 files modified)

## Review Process

Three-wave comprehensive review with 20+ specialized agents:
- **Wave 0**: 10 per-package deep review agents (one per major crate)
- **Wave 1**: 8/11 foundation agents (security, business logic, architecture, error handling, tests, type design, code quality, language specialist)
- **Wave 2**: 10/10 deep-dive agents (performance, concurrency, API contract, dependency, error messages, dead code, naming, observability, config safety, migration safety)

## Findings Summary

| Severity | Found | Fixed | Deferred |
|----------|-------|-------|----------|
| CRITICAL | 11 | 11 | 0 |
| HIGH | 28 | 8 | 20 |
| MEDIUM | 40+ | 0 | 40+ |
| LOW | 25+ | 0 | 25+ |

## CRITICAL Fixes (All 11 Fixed)

### C1. resize_vm was a complete no-op
**File**: `crates/chv-controlplane-service/src/lifecycle.rs`
**Fix**: Implemented `persist_intent_and_accept` pattern with new `VmResourcesPatchInput` struct and `set_vm_resources` method in the store layer.

### C2. correlation_id always NULL for lifecycle operations
**File**: `crates/chv-controlplane-service/src/lifecycle.rs`
**Fix**: Propagate `meta.operation_id` as correlation_id when non-empty to both `OperationCreateInput` and `EventAppendInput`.

### C3. Span::enter() across .await — undefined behavior
**Files**: `crates/chv-stord-core/src/handlers.rs`, `crates/chv-nwd-core/src/handlers.rs`
**Fix**: Removed all 18 `span.enter()` guard patterns. Spans remain as metadata attached to log context without holding guards across await points.

### C4. Silent persistence failure in nwd-core
**File**: `crates/chv-nwd-core/src/handlers.rs`
**Fix**: `persist_upsert` and `persist_remove` now match on both `Ok(Ok(()))`, `Ok(Err(e))`, and `Err(e)` with proper error logging.

### C5. Raw FD used after OwnedFd takes ownership
**File**: `crates/chv-agent-runtime-ch/src/process.rs`
**Fix**: Moved `dup()` call before `OwnedFd::from_raw_fd()` takes ownership, preventing use-after-move UB.

### C6. snapshot/restore correlation_id write silently dropped
**File**: `crates/chv-controlplane-service/src/lifecycle.rs`
**Fix**: Replaced `let _ = sqlx::query(...)` with proper `if let Err(e) = ...` error logging patterns.

### C7. Serial VM dispatch blocks orchestrator
**File**: `crates/chv-controlplane-service/src/orchestrator.rs`
**Fix**: Converted serial loop to concurrent dispatch using `futures::future::join_all` with pinned boxed futures.

### C8. Double mutex acquisition per VM in reconcile_vms
**File**: `crates/chv-agent-core/src/reconcile.rs`
**Fix**: Combined two separate `cache.lock().await` calls per VM (one for generation, one for spec_json) into a single lock acquisition that extracts both values.

### C9. BFF Internal errors carry no correlation ID
**File**: `crates/chv-webui-bff/src/correlation_middleware.rs`
**Fix**: Middleware now always generates a correlation_id (even if client doesn't send one) and returns it in `x-correlation-id` response header.

### C10. BffError::Internal strips error cause
**File**: `crates/chv-webui-bff/src/handlers/tokens.rs`
**Fix**: All 4 `.map_err()` handlers now include the sqlx error in the message string (logged only, not exposed to client).

### C11. ResizeVolume silent no-op when resize_bytes missing
**File**: `crates/chv-controlplane-service/src/bff_mutations.rs`
**Fix**: `resize_bytes.unwrap_or(...)` replaced with `.ok_or_else(|| BffError::BadRequest(...))` — returns 400 if resize_bytes is absent.

## HIGH Fixes (8 of 28 Fixed)

### H1-H6. Missing ownership checks (Security)
**Files**: `handlers/vms.rs`, `handlers/snapshots.rs`, `handlers/exports.rs`
**Fix**: Added `require_vm_owner()` checks to: get_vm_console, get_vm_console_url, list_vm_snapshots, delete_snapshot, export_vm, download_export.

### H7. Console token reuses JWT secret without audience
**File**: `crates/chv-webui-bff/src/handlers/vms.rs`
**Fix**: Added `aud: "chv:console"` field to ConsoleTokenClaims to prevent JWT token type confusion.

## HIGH Deferred (20 remaining — tracking)

These findings are real but require more design consideration:
- H8: Quota management operator-to-user escalation
- H9: POST creation endpoints return 200 not 201
- H10-H13: Performance (O(n) session scan, N+1 node resolution, TOCTOU, async lock during parse)
- H14-H18: Business logic (network deletion without VM check, state machine gaps)
- H19-H21: Architecture (dependency inversion, ghost dep, API versioning)
- H22-H25: Error handling (useless status field, gRPC code mismatch, health endpoint logic)
- H26-H28: Observability/config (session metrics, empty JWT secret, migration data loss)

## Verification

```
cargo build --workspace    OK (0 errors)
cargo test --workspace     OK (294 tests pass)
cargo clippy --workspace   OK (0 warnings with -D warnings)
```

## Changes by Area

| Area | Files | Lines Changed |
|------|-------|---------------|
| Control-plane service | 8 | +232/-115 |
| Control-plane store | 2 | +59/-1 |
| Agent core | 1 | +14/-15 |
| Agent runtime | 1 | +2/-2 |
| NWD core | 1 | +20/-10 |
| Stord core | 1 | +24/-24 |
| WebUI BFF | 11 | +100/-50 |
| Other (Cargo, lock) | 2 | +18/0 |
