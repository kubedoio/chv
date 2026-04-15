# Changelog

All notable changes to this project will be documented in this file.

## [0.0.0.2] - 2026-04-14

### Added
- Rust control plane Phase 1 foundation with inbound gRPC and HTTP admin APIs
- `chv-controlplane` binary with optional mTLS for gRPC and axum-based admin server
- `ControlPlaneService`, `LifecycleService`, `ReconcileService`, `EnrollmentService`, and `TelemetryService` implementations
- PostgreSQL-backed repositories: nodes, desired state, observed state, bootstrap tokens, network exposures
- Structured error mapping to tonic::Status with sanitized user-facing messages
- Operation journal for VM lifecycle with idempotency via resource fingerprinting
- Desired-state fragment parsers with strict validation and `deny_unknown_fields`
- Certificate enrollment with optional CA-backed issuer and bootstrap token validation
- HTTP admin endpoints: health, ready, nodes list, and Prometheus metrics
- Expanded integration tests for store, service, and API layers

### Removed
- Legacy Go control plane (`legacy/go-controlplane`) and stale references

## [0.0.0.1] - 2026-04-10

### Changed
- Simplified docker-compose configurations by removing agent service (runs on bare-metal hosts)
- Changed controller port mapping from 8080:8080 to 8088:8080 to avoid conflicts
- Removed agent dependency from controller service
