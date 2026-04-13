# ADR-003 — Node State Machine

## Status
Accepted

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
