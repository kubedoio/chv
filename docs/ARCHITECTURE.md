# CHV Architecture

This document describes the high-level architecture of CHV, the boundaries between components, and the current implementation phase.

## System Overview

CHV is a Linux-first, cloud-image-first virtualization platform. It targets sovereign private cloud and edge environments where operators need full control over the hypervisor stack without the operational complexity of OpenStack or the licensing cost of VMware vSphere.

The system is built around four binaries:

| Binary | Responsibility | Runtime |
|--------|---------------|---------|
| `chv-controlplane` | Orchestration, desired-state management, node enrollment, Web UI BFF | Control-plane host |
| `chv-agent` | VM lifecycle, Cloud Hypervisor runtime, serial console, local telemetry | Hypervisor host |
| `chv-stord` | Volume management, storage pools, images, snapshots | Hypervisor host (sidecar to agent) |
| `chv-nwd` | Network topology, bridges, firewall/NAT, DHCP, DNS | Hypervisor host (sidecar to agent) |

## Architectural Boundaries

### Control Plane ↔ Agent

- **Only `chv-agent` is reachable from the control plane.** All communication is gRPC over mTLS.
- The control plane owns **desired state**; the agent owns **observed state** and converges toward desired state.
- Cloud Hypervisor is accessed **only** via local Unix sockets from `chv-agent`.

See ADR-002: [Control Plane to Node Boundary](./specs/adr/002-control-plane-boundary.md)

### Agent ↔ Storage / Network

- `chv-agent` communicates with `chv-stord` and `chv-nwd` via local gRPC (Unix socket or loopback).
- These daemons may be upgraded independently inside a compatibility matrix, but the default is a bundle-tested node release.

See ADR-001: [Node Runtime Split](./specs/adr/001-node-runtime-split.md)

### Web UI ↔ Backend

- The browser talks **only** to the control-plane BFF HTTP service (`chv-webui-bff`).
- Direct browser access to `chv-agent`, `chv-stord`, `chv-nwd`, or Cloud Hypervisor APIs is forbidden.

See ADR-002-WebUI: [WebUI Architecture Boundary](./specs/adr/002-webui-architecture-boundary.md)

## Data Flow

### VM Creation (Happy Path)

```
User (Web UI)
    │ POST /api/v1/vms
    ▼
chv-controlplane (BFF)
    │ validate, quota check, assign node
    ▼
SQLite (desired_state, operation_journal)
    │
    ▼
Reconcile loop
    │ gRPC CreateVm
    ▼
chv-agent
    │ 1. call chv-stord (prepare volume)
    │ 2. call chv-nwd   (ensure network)
    │ 3. call cloud-hypervisor (vm.create)
    ▼
Observed state streamed back ──► SQLite ──► Web UI polling
```

### Serial Console

```
Browser ──► WebSocket /ws/vms/{id} ──► BFF ──► gRPC ──► chv-agent ──► PTY ──► CHV API
```

Console access is gated by short-lived JWT tokens with one-time-use replay prevention.

## State Machines

### Node State

Nodes progress through explicit states before they are schedulable:

`Discovered` → `Bootstrapping` → `HostReady` → `StorageReady` → `NetworkReady` → `TenantReady`

Only `TenantReady` nodes receive new VMs. Nodes may also enter `Degraded`, `Draining`, `Maintenance`, or `Failed`.

See ADR-003: [Node State Machine](./specs/adr/003-node-state-machine.md)

### Task State

Every mutating action creates a task record with states:

`queued` → `running` → (`succeeded` | `failed` | `cancelled`)

Tasks are first-class UI objects; users can inspect progress, cancel queued tasks, and view history.

## Storage Datapath

MVP-1 uses a host-side `chv-stord` daemon. Supported storage classes:

- Local raw / qcow2 files
- LVM thin pools
- iSCSI (planned)
- Ceph RBD (planned)

The storage-VM / NBD model was explicitly rejected for MVP-1.

See ADR-004: [Storage Datapath Model](./specs/adr/004-storage-datapath.md)

## Network Service Model

MVP-1 uses Linux bridge + netns + veth + nftables via a host-side `chv-nwd` daemon. Advanced features deferred:

- eBPF-based data plane
- Distributed overlay networking
- Full flow-state replication

See ADR-005: [Network Service Model](./specs/adr/005-network-service-model.md)

## Partition and Autonomy

During control-plane outages, nodes preserve runtime state and allow limited local operations (self-heal, VM stop/reboot). They deny new VM creation, migrations, and destructive topology mutations. Upon reconnection, nodes converge back to the control-plane desired state.

See ADR-006: [Partition and Autonomy Policy](./specs/adr/006-partition-policy.md)

## Upgrade and Rollback

The default upgrade path is a bundle-tested node release. One-step rollback to the previous tested bundle is supported. The system tracks versions for:

- Control plane
- `chv-agent`, `chv-stord`, `chv-nwd`
- Cloud Hypervisor
- Host helper tools

See ADR-007: [Upgrade and Rollback Policy](./specs/adr/007-upgrade-rollback.md)

## Current Implementation Phase

**Phase:** Early-to-MVP transitioning to stability  
**Gap Analysis:** [`plans/2026-04-24-gap-analysis-and-implementation-plan.md`](./plans/2026-04-24-gap-analysis-and-implementation-plan.md)

### What Works

- VM lifecycle (create, start, stop, reboot, delete) via desired-state reconciliation
- Node enrollment with mTLS and bootstrap tokens
- Certificate authority with optional CA-backed issuer
- SQLite repositories with desired/observed state tracking
- Operation journal with idempotency
- Prometheus metrics endpoint
- Web UI dashboard, VM list/detail, events, images, networks, storage pools
- Serial console backend (WebSocket → PTY → CHV)
- Hypervisor settings DB + BFF (orchestrator merge partially wired)
- Basic CI (GitHub Actions)

### Active Gaps (Sprints 11–15)

| Area | Gap | Priority |
|------|-----|----------|
| Backend | Network mutations (`mutate_network` returns `NotImplemented`) | P1 |
| Backend | Quota enforcement at VM-create time | P1 |
| Backend | Backup jobs & history stubbed | P2 |
| Agent | Stord/NWD mock stubs cover many RPCs; real clients incomplete | P1 |
| Agent | Console token replay prevention hardening | P1 |
| UI | Production-readiness Tailwind-first refactor not started | P1 |
| UI | Command palette TODO | P2 |
| Infra | DB ownership on deploy; agent console port collision | P0 |
| Infra | Nginx WebSocket proxy partially missing | P1 |

## Technology Choices

| Layer | Technology | Rationale |
|-------|-----------|-----------|
| Backend language | Rust | Memory safety, async performance, single-binary deployments |
| Database | SQLite | Zero-config, embedded, sufficient for early phase; migration path to PostgreSQL documented |
| Frontend | SvelteKit + TailwindCSS | Compile-time optimizations, minimal runtime, design-token-friendly |
| RPC | gRPC / protobuf | Strong contracts, streaming, generated bindings |
| BFF HTTP | axum | Rust-native, async, integrates with tonic stacks |
| Metrics | Prometheus | Industry standard, pull-based, low overhead |
| VMM | Cloud Hypervisor | Modern, Rust-based, KVM-only, minimal attack surface |

## Related Documents

- [Architecture Decision Records](./specs/adr/)
- [Component Specifications](./specs/component/)
- [Gap Analysis & Implementation Plan](./plans/2026-04-24-gap-analysis-and-implementation-plan.md)
- [Deployment Guide](./DEPLOYMENT.md)
- [Design System](../DESIGN.md)
