# Rust Control Plane Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the actual Rust control plane as a separate service stack, move control-plane-owned responsibilities out of `chv-agent`, and bring node/control-plane behavior into compliance with the CHV specs.

**Architecture:** Keep `chv-agent` as the sole node-side orchestrator and local Cloud Hypervisor caller. Add a separate Rust control-plane workspace slice inside the existing repo with typed proto-facing services, durable SQLite-backed state, and explicit desired/observed separation. Remove hardcoded operational literals from handlers and reconcilers by sourcing all values from typed config, persisted desired state, proto contracts, or shared domain enums.

**Tech Stack:** Rust stable, Tokio, tonic, axum, sqlx, serde, tracing, SQLite, existing generated proto crates.

---

## Current Structure Analysis

### What exists now
- Workspace binaries/services:
  - `cmd/chv-agent`
  - `cmd/chv-stord`
  - `cmd/chv-nwd`
- Existing shared crates:
  - `crates/chv-agent-core`
  - `crates/chv-agent-runtime-ch`
  - `crates/chv-config`
  - `crates/chv-errors`
  - `crates/chv-observability`
  - `crates/chv-stord-core`
  - `crates/chv-stord-backends`
  - `crates/chv-nwd-core`
- Existing generated contracts:
  - `gen/rust/control-plane-node-api`
  - `gen/rust/chv-stord-api`
  - `gen/rust/chv-nwd-api`

### What is missing
- No `chv-controlplane` binary
- No `chv-api` or admin/BFF binary
- No control-plane persistence crate
- No SQLite/sqlx integration
- No migration system
- No persisted `operations`, `events`, `alerts`, `node_states`, `*_desired_state`, `*_observed_state`

### What is in the wrong place today
- Control-plane-facing orchestration is mixed into `crates/chv-agent-core/src/agent_server.rs`
- Desired-state acceptance and observed-state advancement are mixed in `crates/chv-agent-core/src/cache.rs` and `crates/chv-agent-core/src/vm_runtime.rs`
- Control-plane outbox flush logic is lossy in `crates/chv-agent-core/src/control_plane.rs`
- Operational literals are embedded in handlers and reconcilers:
  - network bridge names
  - subnet CIDRs
  - backend class defaults
  - filesystem paths
  - state strings

## Non-Negotiable Implementation Rules

- No direct Cloud Hypervisor access in the control plane
- No desired/observed state collapse
- No new business logic literals in handlers, reconcilers, stores, or service clients
- No defaulting of runtime behavior from ad hoc string or numeric literals inside request handling
- Every operational value must come from exactly one of:
  - proto contract fields
  - persisted desired state
  - persisted policy/config rows
  - typed process configuration
  - shared domain enum/constant module for protocol-level fixed values only

## Hardcoded Literal Elimination Policy

Before implementation starts, apply this rule to every new change:

- Paths like `/run/...`, `/usr/bin/...`, `/var/lib/...` must come from typed config structs
- Values like `10.0.0.0/24`, `br0`, `local`, `Healthy`, `ok`, `error` must not be introduced inline in orchestration logic
- State names must come from enums, not freeform strings
- Event types, alert types, and error codes must come from typed domain constants or enums
- Retry intervals, thresholds, and timeouts must come from config with validated defaults
- Test data can use literals, production logic cannot

## Implementation Order

Build bottom-up:

1. shared contracts and domain types
2. persistence and migrations
3. control-plane service skeleton
4. enrollment and inventory ingestion
5. desired-state persistence
6. observed-state ingestion
7. lifecycle command journal and node RPC client
8. agent boundary cleanup and idempotency fixes
9. admin/API surface

## Phase 0: Repo Foundation

### Task 0.1: Add control-plane workspace members

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/chv-controlplane-types/Cargo.toml`
- Create: `crates/chv-controlplane-types/src/lib.rs`
- Create: `crates/chv-controlplane-store/Cargo.toml`
- Create: `crates/chv-controlplane-store/src/lib.rs`
- Create: `crates/chv-controlplane-service/Cargo.toml`
- Create: `crates/chv-controlplane-service/src/lib.rs`
- Create: `cmd/chv-controlplane/Cargo.toml`
- Create: `cmd/chv-controlplane/src/main.rs`

**Acceptance criteria:**
- New workspace members compile as empty skeleton crates
- `cmd/chv-controlplane` boots a minimal process with logger/config wiring
- No agent crate imports Cloud Hypervisor code from control-plane crates

**Verification:**
- Run: `cargo check -p chv-controlplane-types -p chv-controlplane-store -p chv-controlplane-service -p chv-controlplane`
- Expected: success

### Task 0.2: Add control-plane config without hardcoded operational literals in handlers

**Files:**
- Modify: `crates/chv-config/src/lib.rs`
- Create: `crates/chv-controlplane-types/src/config.rs`
- Modify: `cmd/chv-controlplane/src/main.rs`

**Acceptance criteria:**
- Typed config exists for bind addresses, database DSN, TLS files, and runtime settings
- Handler/service code reads values from config structs only
- No bind/path/timeout values are introduced inline in control-plane request handlers

**Verification:**
- Run: `cargo test -p chv-config`
- Run: `cargo check -p chv-controlplane`

## Phase 1: Shared Domain Model

### Task 1.1: Introduce typed control-plane enums and IDs

**Files:**
- Create: `crates/chv-controlplane-types/src/domain.rs`
- Modify: `crates/chv-controlplane-types/src/lib.rs`

**Acceptance criteria:**
- Resource kinds, node readiness states, operation statuses, event severities, and well-known event types use enums/newtypes
- Stringly-typed status handling is isolated at proto/store boundaries
- No new service logic compares raw string literals for domain state

**Verification:**
- Run: `cargo test -p chv-controlplane-types`

### Task 1.2: Define desired vs observed state models explicitly

**Files:**
- Create: `crates/chv-controlplane-types/src/state.rs`

**Acceptance criteria:**
- Separate structs exist for desired and observed state of node, VM, volume, and network resources
- Shared model makes it impossible to store desired payloads in observed fields by accident

**Verification:**
- Run: `cargo test -p chv-controlplane-types`

## Phase 2: Persistence and Migrations

### Task 2.1: Add SQLite/sqlx foundation

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/chv-controlplane-store/Cargo.toml`
- Create: `crates/chv-controlplane-store/src/db.rs`
- Create: `cmd/chv-controlplane/migrations/`

**Acceptance criteria:**
- Control-plane store uses SQLite + `sqlx`
- Database pool boots from config
- Migration runner exists in control-plane startup

**Verification:**
- Run: `cargo check -p chv-controlplane-store -p chv-controlplane`

### Task 2.2: Create initial schema for authoritative state

**Files:**
- Create: `cmd/chv-controlplane/migrations/0001_initial.sql`
- Create: `crates/chv-controlplane-store/src/schema.rs`

**Acceptance criteria:**
- Tables exist for:
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
- Schema keeps desired and observed records separate
- No state JSON blobs are used where first-class columns are required for filtering/idempotency

**Verification:**
- Run migration against local SQLite
- Run: `cargo check -p chv-controlplane-store`

### Task 2.3: Add store repositories with no freeform SQL literals in services

**Files:**
- Create: `crates/chv-controlplane-store/src/nodes.rs`
- Create: `crates/chv-controlplane-store/src/desired_state.rs`
- Create: `crates/chv-controlplane-store/src/observed_state.rs`
- Create: `crates/chv-controlplane-store/src/operations.rs`
- Create: `crates/chv-controlplane-store/src/events.rs`

**Acceptance criteria:**
- Service layer depends on repository methods, not inline SQL
- Repositories expose typed APIs for desired state, observed state, operations, and events
- Operation upsert supports idempotency by `operation_id`

**Verification:**
- Run: `cargo test -p chv-controlplane-store`

## Phase 3: Control-Plane gRPC Service Skeleton

### Task 3.1: Implement EnrollmentService server

**Files:**
- Create: `crates/chv-controlplane-service/src/enrollment.rs`
- Modify: `crates/chv-controlplane-service/src/lib.rs`
- Modify: `cmd/chv-controlplane/src/main.rs`

**Acceptance criteria:**
- `EnrollNode`, `RotateNodeCertificate`, and `ReportBootstrapResult` are served by control plane
- Enrollment persists node record, inventory, versions, and bootstrap result
- TLS material issuance/rotation logic is isolated behind an internal trait

**Verification:**
- Run: `cargo test -p chv-controlplane-service enrollment`

### Task 3.2: Implement InventoryService server

**Files:**
- Create: `crates/chv-controlplane-service/src/inventory.rs`

**Acceptance criteria:**
- Node inventory and version reports persist durably
- Reports update current snapshots and append version history where needed

**Verification:**
- Run: `cargo test -p chv-controlplane-service inventory`

### Task 3.3: Implement TelemetryService server

**Files:**
- Create: `crates/chv-controlplane-service/src/telemetry.rs`

**Acceptance criteria:**
- Node/VM/volume/network observed-state reports persist to observed-state tables only
- Event and alert reports append durable records
- `operation_id` is retained end-to-end for correlation

**Verification:**
- Run: `cargo test -p chv-controlplane-service telemetry`

## Phase 4: Desired-State Authority

### Task 4.1: Persist desired-state fragments in control plane

**Files:**
- Create: `crates/chv-controlplane-service/src/desired_state.rs`
- Modify: `crates/chv-controlplane-store/src/desired_state.rs`

**Acceptance criteria:**
- Control plane can create/update desired state for node, VM, volume, and network resources
- Generations are validated as non-empty monotonic decimal strings
- Desired state is stored before any node dispatch

**Verification:**
- Run: `cargo test -p chv-controlplane-service desired_state`

### Task 4.2: Add generation and contract validators

**Files:**
- Create: `crates/chv-controlplane-service/src/validation.rs`

**Acceptance criteria:**
- `desired_state_version` validation rejects empty and non-numeric values
- `target_node_id` and request node ID consistency checks exist
- Fragment kind/id consistency checks exist

**Verification:**
- Run: `cargo test -p chv-controlplane-service validation`

## Phase 5: Node RPC Client and Operation Journal

### Task 5.1: Implement node gRPC client from control plane to agent

**Files:**
- Create: `crates/chv-controlplane-service/src/node_client.rs`

**Acceptance criteria:**
- Control plane issues node RPCs through typed client wrappers
- mTLS config comes from typed config, not inline literals
- Client records request latency and response result per operation

**Verification:**
- Run: `cargo test -p chv-controlplane-service node_client`

### Task 5.2: Add operation journal and idempotent dispatch

**Files:**
- Modify: `crates/chv-controlplane-store/src/operations.rs`
- Create: `crates/chv-controlplane-service/src/orchestrator.rs`

**Acceptance criteria:**
- Each control-plane mutation persists an operation row before dispatch
- Replays with same `operation_id` return the persisted result or in-flight state
- Dispatch retries are safe and do not create duplicate commands

**Verification:**
- Run: `cargo test -p chv-controlplane-service orchestrator`

## Phase 6: Agent Boundary Cleanup

### Task 6.1: Strip control-plane-owned responsibilities out of `chv-agent`

**Files:**
- Modify: `crates/chv-agent-core/src/agent_server.rs`
- Modify: `crates/chv-agent-core/src/cache.rs`
- Modify: `crates/chv-agent-core/src/control_plane.rs`
- Modify: `crates/chv-agent-core/src/vm_runtime.rs`

**Acceptance criteria:**
- Agent no longer treats receipt of desired state as observation
- `node_observed_generation` is resource-specific
- Pending outbox flush preserves the full unsent tail on failure
- Agent remains the only Cloud Hypervisor caller

**Verification:**
- Run: `cargo test -p chv-agent-core`

### Task 6.2: Remove hardcoded orchestration literals from agent

**Files:**
- Modify: `crates/chv-agent-core/src/agent_server.rs`
- Modify: `crates/chv-agent-core/src/reconcile.rs`
- Modify: `crates/chv-agent-core/src/daemon_clients.rs`
- Modify: `crates/chv-config/src/lib.rs`

**Acceptance criteria:**
- No inline bridge names, subnet CIDRs, backend classes, file paths, or operational thresholds remain in agent business logic
- Such values come from desired state, config, or typed policy objects

**Verification:**
- Run: `rg -n '"br0"|10\\.0\\.0\\.0/24|"local"|"/run/|"/usr/bin/|"/var/lib/' crates/chv-agent-core crates/chv-config cmd/chv-agent`
- Expected: only config defaults, fixtures, or explicitly approved protocol constants remain

### Task 6.3: Implement drain and maintenance semantics properly

**Files:**
- Modify: `crates/chv-agent-core/src/agent_server.rs`
- Modify: `crates/chv-agent-core/src/state_machine.rs`
- Modify: `crates/chv-agent-core/src/reconcile.rs`

**Acceptance criteria:**
- `DrainNodeRequest.allow_workload_stop` changes behavior
- Exit from maintenance re-enters readiness evaluation, not a blind state assignment
- Node state transitions remain spec-compliant

**Verification:**
- Run: `cargo test -p chv-agent-core drain`
- Run: `cargo test -p chv-agent-core maintenance`

## Phase 7: HTTP/Admin API

### Task 7.1: Add `chv-api` or axum admin surface

**Files:**
- Create: `cmd/chv-api/Cargo.toml`
- Create: `cmd/chv-api/src/main.rs`
- Create: `crates/chv-controlplane-service/src/http.rs`
- Modify: `Cargo.toml`

**Acceptance criteria:**
- Health/readiness endpoints exist
- Admin endpoints expose node state, operation history, and events from persisted store
- Browser-facing API does not call node gRPC directly

**Verification:**
- Run: `cargo check -p chv-api`

## Phase 8: Test Matrix

### Task 8.1: Add contract and persistence regression tests

**Files:**
- Create: `crates/chv-controlplane-service/tests/`
- Modify: `crates/chv-agent-core/src/agent_server.rs`
- Modify: `crates/chv-agent-core/src/control_plane.rs`
- Modify: `crates/chv-agent-core/src/cache.rs`

**Acceptance criteria:**
- Tests cover:
  - empty generation rejected
  - non-numeric generation rejected on first write
  - wrong node ID rejected
  - repeated operation is idempotent
  - outbox partial failure retains all remaining messages
  - persistence failure does not ack success
  - desired and observed state stay separate
  - audit/event rows are written for admin mutations

**Verification:**
- Run: `cargo test`

## Checkpoints

### Checkpoint A: After Phase 2
- Workspace compiles
- SQLite store boots
- Migrations apply cleanly
- Desired and observed tables are separate

### Checkpoint B: After Phase 5
- Control plane can enroll nodes, persist inventory, accept telemetry, persist desired state, and issue journaled node commands
- No control-plane service code depends on Cloud Hypervisor adapter types

### Checkpoint C: After Phase 6
- Agent is back inside its intended boundary
- Generation handling is strict and resource-specific
- No lossy deferred telemetry flushing remains
- No new production logic literals remain outside config/domain modules

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Trying to “improve” agent and control plane in one pass | High | Land persistence and service skeleton first, then boundary cleanup |
| Hidden literal defaults keep creeping back in | High | Add lint-like grep checks in CI and code review checklist |
| Desired/observed split gets blurred again in shared structs | High | Separate repository APIs and structs for desired vs observed |
| Idempotency only implemented in memory | High | Journal operations in SQLite before dispatch |
| Agent request handlers still ack before durable writes | High | Treat persistence failures as request failures and test them |

## First Recommended Execution Slice

Start with these tasks only:

1. Task 0.1
2. Task 0.2
3. Task 1.1
4. Task 1.2
5. Task 2.1

Reason:
- It creates the missing control-plane skeleton
- It establishes the non-hardcoded-literal rule early
- It gives us typed boundaries before touching behavior
- It avoids mixing store, RPC, and agent cleanup in the first implementation pass
