# ADR-010: Async Runtime Safety

## Status
Accepted

## Date
2026-04-26

## Context
CHV daemons are async Rust applications built on Tokio. Several components use mutexes to protect shared state (VM runtime maps, rate limiters, HTTP join handles). Using `std::sync::Mutex` in async code creates two hazards:
1. **Blocking the async runtime** — holding a `std::sync::Mutex` across an `.await` point blocks the executor thread
2. **Panics on poison** — `std::sync::Mutex::lock().unwrap()` panics if the mutex is poisoned, crashing the daemon

These hazards were observed in production code paths: `console_server.rs` (WebSocket handlers), `container.rs` (gRPC shutdown), and `vm_runtime.rs` (VM state mutations).

## Decision

### 1. Use `tokio::sync::Mutex` in async contexts
- Any mutex held across an `.await` point must be `tokio::sync::Mutex`
- Use `async fn` for lock acquisition: `.lock().await`
- This applies to HTTP handlers, gRPC service methods, WebSocket callbacks, and background tasks

### 2. Use `std::sync::Mutex` only in synchronous contexts
- Synchronous helper functions that do not `.await` may use `std::sync::Mutex` for lower overhead
- Lock acquisition must never panic: use `.map_err()` to return `ChvError::Internal` in `Result`-returning functions, or `.unwrap_or_else(|e| e.into_inner())` in infallible contexts to recover from poison gracefully

### 3. Minimize lock scope
- Drop locks as soon as the critical section ends (explicit `drop()` or scoped blocks)
- Never hold a lock while performing I/O, RPC calls, or long computations
- Split operations into "read lock → compute → write lock" when possible

### 4. Prefer message passing for complex coordination
- For cross-task state that does not require synchronous reads, use `tokio::sync::mpsc` or `tokio::sync::watch`
- The supervisor in `chv-agent-core` uses this pattern for daemon lifecycle signals

## Examples

```rust
// CORRECT: tokio::sync::Mutex in async handler
async fn check_rate_limit(
    vm_id: &str,
    rate_limiter: &Arc<tokio::sync::Mutex<HashMap<String, Instant>>>,
) -> Option<Response> {
    let mut limits = rate_limiter.lock().await;
    // ... check and update ...
    None
}

// CORRECT: std::sync::Mutex in sync helper with graceful poison recovery
pub fn get(&self, vm_id: &str) -> Option<VmRecord> {
    self.vms
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(vm_id)
        .cloned()
}

// WRONG: std::sync::Mutex + unwrap() in async path (crashes on poison, blocks runtime)
async fn bad_handler(state: Arc<State>) {
    let mut map = state.map.lock().unwrap(); // DO NOT DO THIS
    some_async_call().await; // Blocks thread if another task panicked while holding lock
}
```

## Alternatives Considered

### `parking_lot::Mutex`
- Pros: faster than `std::sync::Mutex`, no poisoning
- Cons: still blocks the async executor thread if held across `.await`
- Rejected: does not solve the primary problem (blocking async runtime)

### `async-lock` crate
- Pros: `async` mutexes with additional primitives (RwLock, Semaphore)
- Cons: additional dependency when `tokio::sync` already covers our needs
- Rejected: Tokio's built-in sync primitives are sufficient and reduce dependency surface

## Consequences
- All async mutex usage must be audited during code review
- Existing `std::sync::Mutex` in async paths must be migrated to `tokio::sync::Mutex`
- Tests for async mutex consumers must be `#[tokio::test]` async tests
- Future work: add a Clippy lint or CI check to detect `std::sync::Mutex` in async fn bodies
