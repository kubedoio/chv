# ADR-003 — Node State Machine

## Status
Accepted

## Date
2026-04-13

## Context
The control plane must not infer readiness from loose health checks. A node needs explicit states for scheduling, failure handling, and recovery.

## Decision
Each node uses an explicit state machine.

## States
- `Discovered`
- `Bootstrapping`
- `HostReady`
- `StorageReady`
- `NetworkReady`
- `TenantReady`
- `Degraded`
- `Draining`
- `Maintenance`
- `Failed`

## Readiness rules
- `HostReady`: base host verified, local cache available, `chv-agent` running
- `StorageReady`: `chv-stord` is healthy and serving
- `NetworkReady`: `chv-nwd` is healthy and required topology is ready
- `TenantReady`: both storage and network readiness satisfied

## Scheduling rules
- only `TenantReady` nodes are schedulable for new tenant workloads
- `Degraded` nodes are unschedulable by default
- existing workloads may continue in `Degraded`
- `Draining` blocks new placements and evacuates per policy
- `Maintenance` blocks scheduling and allows operator workflows

## Transitions
- `Discovered -> Bootstrapping`
- `Bootstrapping -> HostReady`
- `HostReady -> StorageReady` when `chv-stord` is ready
- `HostReady -> NetworkReady` when `chv-nwd` is ready
- `StorageReady + NetworkReady -> TenantReady`
- `TenantReady -> Degraded` if critical service readiness is lost
- `Degraded -> TenantReady` after successful recovery and reconcile
- `Degraded -> Failed` if safety thresholds or recovery policy require it
- `TenantReady/Degraded -> Draining` when operator or control plane requests drain
- `* -> Maintenance` when maintenance mode is entered

## Consequences
Pros:
- deterministic scheduling behavior
- clearer failure handling
- safer automation and maintenance workflows

Cons:
- requires disciplined state reporting
- requires explicit sub-state handling in implementation

## Related ADRs
- **ADR-006** defines partition and autonomy behavior during control-plane outages. A partitioned node that remains locally healthy does not automatically enter `Degraded`; the transition depends on whether critical service readiness is lost. If the node preserves runtime state and local services remain healthy, it may stay `TenantReady` (but still deny new placements per ADR-006).
- **ADR-005** and **ADR-007** define `chv-nwd` drain-and-replace upgrades. Service-level draining (upgrading `chv-nwd` on a host) is distinct from node-level `Draining` (evacuating all VMs). Service-level drain may be performed without moving the node to `Draining` if tenant continuity can be preserved.
