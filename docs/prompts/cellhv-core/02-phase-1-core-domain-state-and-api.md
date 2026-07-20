# Prompt 02 — Phase 1: Core Domain, Durable State, and Native API

Implement CellHV Core M0: authority, persistence, operations, and the native contract. Do not launch real VMs in this phase.

## Preconditions

- Phase 0 is merged and its architecture guards are green.
- Use branch `agent/cellhv-core-p01-state-api`.
- Read `00-execution-policy.md` and all normative Core documents.

## Goal

Create the smallest durable Core authority that can accept validated VM intent, persist it transactionally, represent every mutation as an operation, and expose a versioned local API without performing host-side effects.

## Required work

### 1. Core domain model

Define platform-neutral types for:

- HostIdentity and HostCapabilities;
- VmId, VmName, VmSpec, RequestedPowerState, ObservedPowerState;
- supported boot definition;
- vCPU and memory;
- storage attachment references;
- network attachment references;
- Operation, OperationStep, OperationStatus;
- Event;
- resource version and idempotency identity;
- ownership and ambiguity status.

Do not include tenant, project, quota, scheduler, Neutron, Cinder, CloudStack, OpenNebula, Kubernetes, libvirt XML, or UI types.

Use explicit newtypes and validated constructors for identifiers and paths. Reject invalid values at the boundary.

### 2. SQLite durable store

Implement a local SQLite store with explicit migrations for at least:

- schema version;
- host identity;
- VM definitions;
- requested and observed state;
- storage and network attachment records;
- operations and operation steps;
- idempotency mappings;
- events;
- ownership markers.

Requirements:

- WAL mode where appropriate;
- foreign keys enabled;
- transactional mutations;
- no silent database recreation after corruption;
- atomic migration behavior;
- migration downgrade/rollback policy documented;
- deterministic test fixtures.

The existing JSON node cache may be read only by a later migration adapter; it is never the authoritative store.

### 3. Operation engine

Implement the generic state machine only, without Linux or VMM execution:

```text
Accepted -> Prepared -> Executing -> Succeeded
                         |            |
                         +-> Failed <-+
                         +-> Interrupted -> RecoveryRequired|Retryable|Failed
```

The exact states may differ if justified, but must support:

- durable acceptance before side effects;
- stable idempotency keys;
- request fingerprint conflict detection;
- resource-version conflict detection;
- restart classification;
- bounded retries;
- cancellation rules;
- structured errors;
- operation/event correlation.

Use property or model-based tests for transition legality and retry/idempotency behavior.

### 4. Native API contract

Create `api/openapi/cellhv-core-v1.yaml` or the repository-approved equivalent.

Minimum resources:

```text
GET  /v1/system
GET  /v1/host
GET  /v1/host/capabilities
POST /v1/vms
GET  /v1/vms
GET  /v1/vms/{id}
PATCH /v1/vms/{id}
DELETE /v1/vms/{id}
POST /v1/vms/{id}/actions/start
POST /v1/vms/{id}/actions/stop
POST /v1/vms/{id}/actions/reboot
POST /v1/vms/{id}/actions/pause
POST /v1/vms/{id}/actions/resume
GET  /v1/operations/{id}
GET  /v1/events
```

Mutation requirements:

- `202 Accepted` with operation reference;
- `Idempotency-Key` support;
- `ETag` and `If-Match` for mutable resources;
- `application/problem+json` errors;
- correlation ID;
- no cloud-platform-specific fields;
- capability fields default to false/absent until executable.

Generate or implement the first Rust client and reproducibility check. Generated artifacts must be deterministic.

### 5. Minimal `cellhvd` API process

Create a non-privileged service that:

- opens the SQLite store;
- applies migrations;
- exposes the local HTTP API over a Unix socket;
- accepts and records operations;
- returns truthful capabilities showing no real VM execution yet;
- shuts down cleanly;
- emits structured tracing and minimal metrics.

It must not run as root, launch Cloud Hypervisor, manipulate Linux networking/storage, or require Controller.

## Acceptance criteria

- `API-001` OpenAPI validation and reproducible client generation.
- `API-002` breaking `/v1` contract changes are rejected in CI.
- `API-003` mutations create durable idempotent operations.
- `API-004` stale resource versions fail before side effects.
- `API-005` capabilities report only implemented behavior.
- `API-006` Core schema contains no cloud-platform model.
- `CORE-STORE-001` corruption fails closed and does not create an empty store.
- Operation model/property tests cover commit boundaries, retries, duplicate keys, and conflicting fingerprints.

## Forbidden outcomes

- real VM or Linux mutations;
- root service execution;
- platform-specific domain fields;
- a second persistence implementation;
- automatic database deletion or replacement;
- reporting VM lifecycle capability before Phase 2;
- coupling API DTOs directly to SQL row types.

## Deliverables

- Core domain crates/modules;
- SQLite schema and migration tests;
- operation engine and model tests;
- OpenAPI contract and Rust client;
- minimal local `cellhvd`;
- API and store documentation;
- upgrade/rollback notes;
- phase evidence report.

## Exit gate

Phase 1 passes when `cellhvd` can be installed and started without a manager, accepts a VM create request into durable state, returns a durable operation, survives restart, and still performs zero VMM or privileged host actions.
