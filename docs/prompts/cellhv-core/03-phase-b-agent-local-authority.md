# Prompt 03 — Phase B: Local Authority Inside `chv-agent`

Evolve the existing `chv-agent` into the durable CellHV Core authority. Do not create a new runtime daemon.

## Preconditions

- Phase A1 migration inventory is reviewed.
- Phase A2 OpenStack discovery evidence exists, even if the final path is undecided.
- Use branch `agent/cellhv-core-pb-local-authority`.

## Estimated effort

4–6 engineering weeks. Split this work into several narrow PRs rather than one large change.

Recommended slices:

1. domain types and SQLite store;
2. operation journal and idempotency;
3. legacy gRPC routing into the operation engine;
4. native local API skeleton;
5. NodeCache migration/compatibility path.

## Goal

Make `chv-agent` authoritative for local VM identity and accepted configuration while preserving current control-plane compatibility and avoiding any duplicate runtime path.

## Required work

### 1. Platform-neutral Core domain

Define types for:

- host identity and capabilities;
- VM identity and supported specification;
- requested and observed power state;
- boot, vCPU, memory, and supported device references;
- network and storage attachment references;
- operations, steps, events, resource versions, and idempotency;
- ownership and recovery classification.

Do not include tenant, project, quota, scheduler, Neutron, Cinder, CloudStack, OpenNebula, Kubernetes, Designer, UI, or libvirt XML types.

### 2. Durable SQLite state

Implement explicit migrations for:

- schema version;
- host identity;
- VM definition and requested/observed state;
- attachment records;
- operation and step records;
- idempotency mappings;
- events;
- ownership markers.

Requirements:

- transactional mutations;
- foreign keys enabled;
- WAL where appropriate;
- corruption fails closed;
- no automatic empty database replacement;
- deterministic migration tests;
- backup/restore and rollback documentation;
- stable identifiers across restart.

### 3. Single operation engine

Every mutation from the current control-plane gRPC path and the new local API must call the same application service and operation journal.

Support:

- durable acceptance before side effects;
- request fingerprint conflict detection;
- resource-version conflicts;
- bounded retries;
- restart classification;
- operation/event correlation;
- explicit unsupported behavior;
- no direct legacy bypass to VMM or providers once the relevant mutation is migrated.

Use model/property tests for state transitions and idempotency.

### 4. Native local API

Implement the smallest API necessary for standalone operation, initially over a Unix socket.

Minimum semantics:

- host and capability inspection;
- create/list/get/update/delete VM definitions;
- start/stop/reboot action requests;
- operation inspection;
- event inspection/watch where feasible.

Requirements:

- asynchronous mutation operations;
- idempotency keys;
- resource versions/conditional updates;
- structured errors;
- no cloud-platform fields;
- capabilities default to false or absent until executable;
- deterministic contract/client generation if OpenAPI is retained after prototype validation.

Do not add remote HTTPS/mTLS yet.

### 5. NodeCache migration

Design and implement a controlled migration or compatibility path:

- existing NodeCache data is never silently discarded;
- durable database becomes authoritative after a recorded cutover;
- repeated migration is idempotent;
- rollback does not create divergent identities;
- malformed cache data fails explicitly;
- old and new paths cannot independently mutate one VM.

### 6. `chv-agent` service behavior

`chv-agent` must:

- start without Controller;
- open and validate its durable store;
- expose the local API;
- retain legacy gRPC compatibility where required;
- report truthful capabilities;
- shut down cleanly;
- remain the same binary/service package.

This phase may record operations without executing new VMM behavior beyond the existing safe path.

## Acceptance criteria

- `AGENT-CORE-001`: no parallel `cellhvd` exists.
- `AGENT-CORE-002`: legacy and native mutations enter one operation journal.
- `AGENT-CORE-003`: current VM identifiers map deterministically into Core identity.
- `AGENT-CORE-004`: old and new paths cannot control the same VM independently.
- `API-001`: native contract validates and generated artifacts are reproducible if applicable.
- `API-002`: breaking contract changes are detected.
- `API-003`: mutations create durable idempotent operations.
- `API-004`: stale resource versions fail before side effects.
- `API-005`: capabilities are truthful.
- `CORE-STORE-001`: corruption fails closed without empty replacement.

## Forbidden outcomes

- new `cellhvd` binary/service;
- second SQLite store or operation journal;
- flag-day removal of the current gRPC path;
- cloud-platform models in Core;
- root execution solely to expose the API;
- direct API-to-SQL coupling;
- automatic identity regeneration;
- fabricated VM lifecycle capability;
- unrelated provider or UI work.

## Deliverables

- domain and store implementation;
- operation engine and model tests;
- legacy/native routing into one authority;
- native local API skeleton;
- NodeCache migration/rollback path;
- schema and API documentation;
- acceptance evidence and residual-risk report.

## Exit gate

Phase B passes when `chv-agent` starts without Controller, accepts a VM definition through the native API, accepts an equivalent legacy request through the existing protocol, records both through one operation engine, survives restart without losing identity, and has no second runtime service.
