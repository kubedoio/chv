# ADR-009: Logging and Observability

## Status
Accepted

## Date
2026-04-26

## Context
CHV runs as a distributed system of daemons on hypervisor hosts and control-plane nodes. Logs are the primary diagnostic tool for operators. Without structured logging:
- Log aggregation across multiple daemons is impossible
- Correlating events for a single VM lifecycle requires manual grep
- Secret material (JWT tokens, bootstrap tokens) may leak to stderr
- Production incidents cannot be traced across service boundaries

## Decision
Use `tracing` as the unified logging framework across all Rust code. Follow these rules:

### 1. `tracing` everywhere, never `println!` / `eprintln!`
- Library crates must use `tracing::info!`, `tracing::warn!`, `tracing::error!`, `tracing::debug!`
- `println!` and `eprintln!` are forbidden in library and service code
- CLI tools may use `println!` for user-facing output only

### 2. Structured fields, not string interpolation
- Prefer `tracing::info!(vm_id = %id, "VM started")` over `tracing::info!("VM {} started", id)`
- Structured fields enable filtering and aggregation in log collectors
- Operation IDs must be attached as `operation_id` span fields (see ADR-002 and chv-agent-spec)

### 3. Secret redaction
- Never log JWT secrets, bootstrap tokens, or private keys
- Log file paths where secrets are loaded (`tracing::info!("loaded jwt_secret from {}", path)`) but never the secret value

### 4. Prometheus metrics endpoint
- `chv-controlplane` exposes a `/metrics` endpoint for Prometheus scraping
- Key metrics: VM state counts, node health, operation latency, gRPC error rates
- Metrics names follow Prometheus conventions: `chv_vms_total`, `chv_nodes_ready`, etc.

### 5. Log levels
- `error!` — daemon crashes, invariant violations, unrecoverable downstream failures
- `warn!` — recoverable issues (token replay detected, retry exhaustion, degraded node)
- `info!` — lifecycle events (VM created, node enrolled, service started)
- `debug!` — request/response details, reconciliation loop iterations
- `trace!` — internal state dumps, per-packet details (use sparingly)

## Alternatives Considered

### `log` crate + `env_logger`
- Pros: simple, widely understood
- Cons: no structured fields, no async-aware spans, harder to correlate across tasks
- Rejected: `tracing` provides spans and structured fields that are essential for multi-daemon debugging

### `slog` with JSON drain
- Pros: structured logging, configurable drains
- Cons: heavier API, less ecosystem integration with tonic/axum
- Rejected: `tracing` is the de facto standard in the Tokio ecosystem and integrates with OpenTelemetry for future expansion

## Consequences
- All crates must declare `tracing` as a dependency
- Log output format is environment-configurable (pretty for dev, JSON for production)
- Future work: OpenTelemetry trace export for cross-service request tracing
