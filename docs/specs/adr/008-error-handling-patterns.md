# ADR-008: Error Handling Patterns

## Status
Accepted

## Date
2026-04-26

## Context
CHV is a Rust-based control plane and node agent with multiple concurrent daemons. Errors propagate across gRPC boundaries, HTTP BFF boundaries, SQLite transactions, and async task boundaries. Without consistent error handling:
- Panics in service code crash daemons and orphan VM processes
- Opaque error strings make debugging production issues impossible
- Inconsistent HTTP status codes confuse API consumers
- Retry storms occur when transient errors are not distinguished from fatal ones

## Decision
Use `chv-errors` as the single structured error crate across the entire Rust workspace. Follow these rules:

### 1. Never panic in service code
- `unwrap()`, `expect()`, and `panic!` are forbidden in production service paths
- Mutex poisoning must be handled gracefully via `map_err` or `unwrap_or_else(|e| e.into_inner())`
- Configuration parsing at startup may use `expect()` with a descriptive message only for truly invariant defaults

### 2. Structured error types via `chv-errors`
- Every crate defines its error enum in `chv-errors` or reuses existing variants
- Common variants: `NotFound`, `Conflict`, `BadRequest`, `Unauthorized`, `QuotaExceeded`, `Internal`, `BackendUnavailable`
- Each variant carries context (resource kind, ID, reason) suitable for both logs and user-facing messages

### 3. gRPC status mapping
- `chv-errors` provides `Into<tonic::Status>` conversions
- `Internal` errors map to `UNKNOWN` gRPC status with a sanitized message
- `NotFound` maps to `NOT_FOUND`, `Conflict` to `ALREADY_EXISTS`, `BadRequest` to `INVALID_ARGUMENT`
- Never leak internal details (file paths, stack traces) across the gRPC boundary

### 4. HTTP BFF mapping
- `BffError` converts to appropriate `axum::http::StatusCode`
- `QuotaExceeded` returns `422 UNPROCESSABLE_ENTITY` with a structured JSON body
- No `unreachable!()` branches in conversion functions

### 5. Transient vs fatal
- `BackendUnavailable` signals a downstream dependency failure (retriable)
- `Internal` signals an invariant violation or bug (not retriably by default)
- Callers decide retry policy based on the error variant, not string matching

### 6. Detached background tasks must be tracked, not fire-and-forget
- gRPC handlers that spawn long-running work (e.g. `migrate_vm` ACKing
  immediately and running migration in the background) **must** retain a
  handle to the spawned task so its terminal `Err` is observable.
- Every detached task is registered in a per-service registry keyed by
  `operation_id` and reaped when the task completes. The registry stores an
  `AbortHandle` (force-drop at next `.await`) and a `CancellationToken`
  (cooperative bail-out at phase boundaries).
- The agent-side
  [`MigrationTaskRegistry`](../../../crates/chv-agent-core/src/migration_registry.rs)
  is the canonical example: `migrate_vm` registers the spawned future so
  operator-issued cancel and agent shutdown can both abort it; the reaper
  logs terminal status and removes the entry.
- Plain `tokio::spawn(...);` with the `JoinHandle` dropped on the floor is
  **forbidden** for any task that can outlive its caller. The compiler will
  not catch this — code review and tests must.

## Alternatives Considered

### `anyhow` everywhere
- Pros: ergonomic, easy to write
- Cons: loses error variant discrimination at API boundaries; callers cannot distinguish retriable from fatal errors
- Rejected: we need structured errors for gRPC/HTTP mapping and retry decisions

### `thiserror` per-crate without a shared enum
- Pros: each crate owns its error types
- Cons: no unified mapping to gRPC/HTTP; translation code scattered and inconsistent
- Rejected: centralizing in `chv-errors` ensures uniform behavior across all services

## Consequences
- All new error variants must be added to `chv-errors`
- Agents and control plane share the same error taxonomy
- Tests can assert on specific error variants rather than string fragments
- Future work: add `ErrorCode` integer constants for machine-readable client handling
