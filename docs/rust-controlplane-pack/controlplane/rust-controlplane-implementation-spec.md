# Rust Control Plane Implementation Spec

## Goal

Implement the CHV control plane in Rust so the backend stack stays consistent with:
- `chv-agent`
- `chv-stord`
- `chv-nwd`
- Cloud Hypervisor integration assumptions

The WebUI remains Svelte/TypeScript. This spec concerns only the backend control plane.

## Why Rust here

The control plane is a long-running systems service that needs:
- typed contracts
- clear error boundaries
- state-machine discipline
- safe concurrency
- versioned service boundaries

The current Rust ecosystem already fits this architecture well:
- `tonic` for gRPC over HTTP/2
- `axum` for HTTP/admin/API services
- `sqlx` for async typed SQL access
- `tracing` and OpenTelemetry integration for instrumentation
- Tokio as the async runtime

## Technology baseline

### Recommended stack
- Language: Rust stable
- Runtime: Tokio
- Internal RPC: `tonic`
- Admin/API/BFF HTTP: `axum`
- Persistence: PostgreSQL + `sqlx`
- Serialization: `serde`
- Logging/diagnostics: `tracing`
- Telemetry export: OpenTelemetry hooks
- Config: structured environment/config file loading
- Migrations: `sqlx` migrations or equivalent migration mechanism

## Architecture shape

### Services
The control plane should be one Rust workspace with multiple crates.

Recommended top-level binaries:
- `chv-controlplane`: main gRPC + internal orchestration service
- `chv-api`: optional HTTP/admin/BFF service if you separate external HTTP from internal gRPC

Recommended shared crates:
- `chv-types`
- `chv-errors`
- `chv-state`
- `chv-store`
- `chv-proto`
- `chv-auth`
- `chv-telemetry`

### Responsibility boundaries

#### Control plane owns
- node enrollment and certificate rotation workflows
- desired-state persistence
- observed-state ingestion
- scheduling intent
- node readiness and placement eligibility
- lifecycle command issuing to nodes
- audit/event persistence
- compatibility and rollout policy

#### Control plane does not own
- direct Cloud Hypervisor access
- direct storage datapath
- direct host network datapath
- node-local emergency manipulation outside `chv-agent`

## Persistence model

### Recommended database
PostgreSQL first.

### Suggested tables
- `nodes`
- `node_versions`
- `node_inventory`
- `node_states`
- `vms`
- `vm_desired_state`
- `vm_observed_state`
- `volumes`
- `volume_desired_state`
- `volume_observed_state`
- `networks`
- `network_desired_state`
- `network_observed_state`
- `operations`
- `events`
- `alerts`
- `maintenance_windows`
- `compatibility_matrix`

Keep desired and observed state separate.

## API shape

### Internal contract
Use the existing `control-plane-node.proto` as the source of truth.

### Transport rules
- gRPC over mTLS for control-plane ↔ node communication
- control plane remains authoritative for desired state
- nodes report observed state and preserve runtime state during outage

### HTTP/admin layer
Use `axum` for:
- operator/admin endpoints
- health/readiness
- WebUI-facing BFF endpoints
- auth/session integration later

Do not let the browser call node gRPC services directly in MVP-1.

## Scheduling and orchestration model

MVP-1 does not need a complex scheduler.

It does need:
- node filtering by readiness state
- placement eligibility checks
- maintenance/drain awareness
- version/compatibility awareness
- explicit rejection reasons

## Security model

- bootstrap token for node enrollment
- node cert issuance and rotation
- mTLS between control plane and nodes
- no direct remote VMM access
- no broad admin mutation without audit event

## Error model

Use a stable typed error space.

Suggested categories:
- auth/authz
- validation
- stale generation
- not found
- conflict
- unsupported
- dependency unavailable
- timeout
- internal

Map these clearly to:
- gRPC status codes
- HTTP status codes
- persisted operation/event failure records

## Observability

### Minimum required
- structured logs with correlation IDs
- request/operation IDs
- state transition events
- node command latency
- database latency/error counters
- reconciliation results
- readiness endpoints

### Telemetry rule
Instrument from day one, but keep the first export path simple.

## Workspace recommendation

```text
/controlplane
  Cargo.toml
  /crates
    /chv-proto
    /chv-types
    /chv-errors
    /chv-state
    /chv-store
    /chv-auth
    /chv-telemetry
    /chv-scheduler
  /services
    /chv-controlplane
    /chv-api
```

## Implementation phases

### Phase 1
- Rust workspace skeleton
- proto generation crate
- config loading
- tracing/bootstrap
- health/readiness endpoints
- PostgreSQL connection
- enrollment service
- basic node inventory/state persistence

### Phase 2
- desired state persistence
- lifecycle command persistence
- node gRPC client layer
- operation journal
- telemetry ingestion

### Phase 3
- basic placement logic
- maintenance/drain flows
- compatibility checks
- WebUI-facing admin HTTP endpoints

## Non-negotiables

- do not move Cloud Hypervisor access into the control plane
- do not bypass `chv-agent`
- do not collapse desired and observed state
- do not skip versioned contracts
- do not build the first version as ad hoc REST-only without the typed gRPC core
