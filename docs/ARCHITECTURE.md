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

#### Drain Evacuation Flow

When a node enters `Draining` (via `chvctl node drain` or the BFF API):

1. Scheduling is paused on the node (no new VMs placed).
2. The agent reconcile loop detects `Draining` state and iterates running VMs.
3. For each VM, a migration request is issued to the control plane (tracked in `drain_requested_vms` to avoid duplicates).
4. When `vm_count` reaches 0 (all VMs evacuated or stopped), the node transitions to `Maintenance`.
5. After maintenance, an operator marks the node `TenantReady` to resume scheduling.

Implementation: `ReconcileEngine` in `crates/chv-agent-core/src/reconcile.rs` handles the `NodeState::Draining` arm.

See ADR-003: [Node State Machine](./specs/adr/003-node-state-machine.md)

### Task State

Every mutating action creates a task record with states:

`queued` → `running` → (`succeeded` | `failed` | `cancelled`)

Tasks are first-class UI objects; users can inspect progress, cancel queued tasks, and view history.

## Storage Datapath

MVP-1 uses a host-side `chv-stord` daemon. Supported storage classes:

- Local raw / qcow2 files
- LVM thin pools
- iSCSI (planned adapter; not a complete production backend)
- Ceph RBD (planned adapter; not a complete production backend)

The storage-VM / NBD model was explicitly rejected for MVP-1.

### Migration Security Model

Storage migration between nodes is secured with mandatory mTLS:

- **mTLS enforcement**: `MigrationSender` rejects plaintext connections. If `tls_config` is not provided, `start_migration()` returns `FAILED_PRECONDITION`. There is no fallback to `http://`.
- **Certificate validation**: The sender presents the node certificate issued by the CP CA and validates the destination's certificate against the same CA.
- **Backpressure**: The receiver can send `Backpressure` messages with a `slow_down_factor`. The sender inserts throttle sleeps proportional to this factor between chunk sends.
- **Flow control**: A sliding send window (default 16 in-flight chunks) prevents memory exhaustion on either side.
- **MigrationReaper**: A background task (`crates/chv-controlplane-service/src/migration_reaper.rs`) scans every 60s for migrations stuck beyond 2 hours and force-transitions them to `Failed`.

Current status: migration orchestration is partial. Control-plane phases, mTLS, flow control, backpressure, rollback paths, and stale-operation reaping exist, but dirty sync rounds, stord-to-control-plane convergence reporting, and paused final dirty flush remain incomplete.

Implementation: `crates/chv-stord-core/src/migration/sender.rs`

See ADR-004: [Storage Datapath Model](./specs/adr/004-storage-datapath.md)

## Network Service Model

MVP-1 uses Linux bridge + netns + veth + nftables via a host-side `chv-nwd` daemon. Advanced features:

- Kernel VXLAN overlay networking with explicit FDB cleanup on VM detach
- eBPF policy and rate-limit enforcement only; eBPF is not the VXLAN datapath
- VXLAN teardown via `delete_topology` (cleans up VXLAN interfaces)

See ADR-005: [Network Service Model](./specs/adr/005-network-service-model.md)

## Partition and Autonomy

During control-plane outages, nodes preserve runtime state and allow limited local operations (self-heal, VM stop/reboot). They deny new VM creation, migrations, and destructive topology mutations. Upon reconnection, nodes converge back to the control-plane desired state.

### Partition Reconnect Flush

When an agent detects that it has reconnected to the control plane after a partition (state transitions from `Disconnected` to `Connected`), it flushes all pending messages queued during the outage. The flush is ordered and atomic per-message: if a dispatch fails, remaining messages stay queued for the next attempt.

Implementation: `ControlPlaneClient::flush_pending_messages()` in `crates/chv-agent-core/src/control_plane.rs`. Pending messages are stored in `NodeCache::pending_control_plane_messages`.

See ADR-006: [Partition and Autonomy Policy](./specs/adr/006-partition-policy.md)

## Upgrade and Rollback

The default upgrade path is a bundle-tested node release. One-step rollback to the previous tested bundle is supported. The system tracks versions for:

- Control plane
- `chv-agent`, `chv-stord`, `chv-nwd`
- Cloud Hypervisor
- Host helper tools

### Upgrade Orchestration Flow

Rolling upgrades are driven by `UpgradeOrchestrator` (trait-based, strategy pattern):

```
UpgradeOrchestrator
    │ plan(target_version, strategy, nodes)
    ▼
For each node (rolling, one-at-a-time):
    1. Run pre-checks (VersionCompatible, DiskSpace, NoActiveMigrations, HealthCheck)
    2. Drain node (pause scheduling, wait for VMs to evacuate)
    3. Write upgrade intent to node_desired_state (Maintenance + target_version)
    4. Agent observes desired state → performs binary swap + systemd restart
    5. Poll node_observed_state for TenantReady (health check)
    6. If health check fails → rollback_node (restore previous binary)
    7. Un-drain node (resume scheduling)
```

The concrete implementation is `SystemdNodeUpgrader` (`crates/chv-controlplane-service/src/systemd_upgrader.rs`), which interacts with the SQLite state store and the node gRPC client pool.

### Compatibility Matrix Boot Gate

Before any upgrade proceeds, the `CompatibilityMatrix` (`crates/chv-controlplane-service/src/compat.rs`) validates that the target version falls within the allowed range for each component. Incompatible versions are rejected before draining begins. The matrix is loaded from `/etc/chv/compat-matrix.toml`.

See ADR-007: [Upgrade and Rollback Policy](./specs/adr/007-upgrade-rollback.md)

## Resilience

### Circuit Breaker

Node communication from the control plane is protected by a circuit breaker (`crates/chv-controlplane-service/src/circuit_breaker.rs`). States: `Closed` (normal) → `Open` (reject immediately after N failures) → `HalfOpen` (probe). Defaults: 5 failures to trip, 30s recovery timeout, 3 successful probes to close.

The `with_circuit_breaker()` helper wraps any async operation and automatically records success/failure. When open, calls return `ChvError::BackendUnavailable` without attempting the RPC.

### Deep Health Checks

The `/health/deep` endpoint (`GET /health/deep`) reports component-level health:

- **database**: SQLite connectivity with latency measurement
- **agent_socket_dir**: Agent runtime directory exists and is readable
- **agent_connectivity**: Can establish a Unix socket connection to at least one agent

Status values: `healthy` (all pass), `degraded` (DB pass but agent issues), `unhealthy` (DB fail). Degraded returns HTTP 200 (still serving); unhealthy returns 503.

## Current Implementation Phase

**Phase:** Early-to-MVP transitioning to stability  
**Gap Analysis:** [`../PHASED_IMPLEMENTATION_PLAN.md`](../PHASED_IMPLEMENTATION_PLAN.md)

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
- Rolling upgrade orchestration with `SystemdNodeUpgrader` and compatibility matrix
- Storage migration with mTLS enforcement, backpressure, and flow control; dirty sync rounds and paused final dirty flush remain incomplete
- Circuit breaker on node communication
- Deep health checks (database, agent socket, agent connectivity)
- Migration reaper (auto-fails stuck migrations after 2h)
- Drain evacuation (automatic VM migration on node drain)
- Partition reconnect flush (pending messages delivered on reconnect)
- eBPF policy scope defined for policy/rate limiting; kernel VXLAN remains the overlay datapath
- FDB cleanup on VM detach
- VXLAN teardown on topology delete

### Remaining Gaps

| Area | Gap | Priority |
|------|-----|----------|
| Backend | Backup/DR execution engine, off-host shipping, restore validation, and runbook automation incomplete | P2 |
| Backend | Disk migration dirty sync rounds, convergence reporting, and paused final dirty flush incomplete | P1 |
| Backend | iSCSI and Ceph RBD storage backend adapters planned, not production-complete | P2 |
| UI | Production-readiness Tailwind-first refactor not started | P2 |
| UI | Command palette TODO | P2 |

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
- [Phased Implementation Plan](../PHASED_IMPLEMENTATION_PLAN.md)
- [Deployment Guide](./DEPLOYMENT.md)
- [Operations Guide](./OPERATIONS.md)
- [Design System](../DESIGN.md)
