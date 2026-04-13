# CHV Repository Layout Spec

## Goal
Provide a clean repository layout that preserves the architecture boundaries defined in the CHV ADR and spec pack.

## Core principles
- `chv-agent`, `chv-stord`, and `chv-nwd` remain separate runtime components.
- shared code is allowed only for small, stable, low-level utilities.
- Cloud Hypervisor integration stays inside `chv-agent` only.
- protobuf contracts are the source of truth for inter-service APIs.
- tests must mirror the runtime split.

## Recommended top-level layout

```text
/chv
  /cmd
    /chv-agent
    /chv-stord
    /chv-nwd
    /chvctl

  /proto
    /controlplane
      control-plane-node.proto
    /node
      chv-stord-api.proto
      chv-nwd-api.proto

  /gen
    /rust
      ... generated protobuf/grpc bindings ...

  /crates
    /chv-common
    /chv-errors
    /chv-observability
    /chv-config
    /chv-state
    /chv-agent-core
    /chv-agent-runtime-ch
    /chv-stord-core
    /chv-stord-backends
    /chv-nwd-core
    /chvctl-core

  /specs
    /adr
    /proto
    /component
    /ops

  /deploy
    /systemd
    /tmpfiles.d
    /sysusers.d
    /example-config

  /scripts
    /dev
    /ci

  /tests
    /integration
    /contracts
    /fixtures

  Cargo.toml
  Cargo.lock
  Makefile
  README.md
```

## Ownership boundaries

### `/cmd/chv-agent`
Thin binary entrypoint for the node orchestrator.
Should only wire config, logging, startup, and service composition.

### `/cmd/chv-stord`
Thin binary entrypoint for the storage daemon.
Should not include control-plane logic.

### `/cmd/chv-nwd`
Thin binary entrypoint for the network daemon.
Should not include storage or VMM lifecycle logic.

### `/cmd/chvctl`
Local operator/debug CLI.
Read-first by default.
Mutating commands must be gated.

## Crate guidance

### `chv-common`
Only truly shared primitives:
- IDs
- request metadata
- time helpers
- tiny utility traits

Do not turn this into a dumping ground.

### `chv-errors`
Shared typed error enums and conversion helpers.
Must define the stable CHV error space.

### `chv-observability`
Shared logging, metrics, correlation-id helpers.
Should provide:
- structured logging initialization
- operation-id propagation helpers
- metric registration wrappers

### `chv-config`
Typed config loading and validation.
Split by component when needed.

### `chv-state`
Local durable cache model and serialization.
May include:
- observed state snapshots
- desired state fragment cache
- service version cache
- recovery metadata

Must not become a replacement for the control plane.

### `chv-agent-core`
Reconciliation engine, node state machine, control-plane client, and lifecycle orchestration.

### `chv-agent-runtime-ch`
Cloud Hypervisor-specific adapter layer.
This crate is the only place where Cloud Hypervisor API/socket details should live.

### `chv-stord-core`
Storage daemon server, session model, request handlers, health model.

### `chv-stord-backends`
Backend adapters for:
- qcow2/raw file
- local block/LVM
- iSCSI-backed block
- Ceph RBD

Keep backend-specific code out of the core handler layer.

### `chv-nwd-core`
Network daemon server, topology model, nftables integration hooks, namespace orchestration.

### `chvctl-core`
Shared CLI inspection logic and typed output rendering.

## Proto and generated code

### `/proto`
Human-edited source-of-truth contract files.

### `/gen/rust`
Generated bindings.
Do not hand-edit.
If generation is deterministic, CI should verify that generated output matches checked-in source.

## Tests

### `/tests/contracts`
Contract tests for:
- protobuf/gRPC compatibility expectations
- idempotency of ensure/apply operations
- stale generation rejection

### `/tests/integration`
Black-box integration tests between components.
At minimum:
- `chv-agent` + `chv-stord`
- `chv-agent` + `chv-nwd`
- VM create flow with mocked Cloud Hypervisor adapter

### `/tests/fixtures`
Static JSON/YAML fixtures, generated desired-state fragments, backend locator examples.

## Deployment layout

### `/deploy/systemd`
Service units for:
- `chv-agent.service`
- `chv-stord.service`
- `chv-nwd.service`
- optional `chvctl` completion/install helper

### `/deploy/sysusers.d`
Dedicated service users.

### `/deploy/tmpfiles.d`
Runtime directories and socket dir preparation.

## Suggested runtime paths

```text
/run/chv/
  agent/
  stord/
  nwd/
  vmm/

/var/lib/chv/
  cache/
  state/
  images/
  volumes/
  runtime/

/etc/chv/
  agent.toml
  stord.toml
  nwd.toml
```

## Non-negotiables
- no direct control-plane access to Cloud Hypervisor
- no storage logic merged into `chv-agent`
- no network logic merged into `chv-agent`
- no service-VM implementation introduced for MVP-1
- contracts remain typed and versioned

## First implementation scope recommendation
1. repository skeleton
2. protobuf generation pipeline
3. `chv-stord` daemon MVP
4. `chv-nwd` daemon MVP
5. `chv-agent` runtime orchestration MVP
6. `chvctl` inspection MVP
