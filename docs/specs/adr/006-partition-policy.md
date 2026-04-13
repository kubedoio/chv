# ADR-006 — Partition and Autonomy Policy

## Status
Accepted

## Context
The node must continue safely during control-plane outages without drifting into unbounded local authority.

## Decision
During control-plane outage:
- preserve current runtime state
- allow local infra self-heal
- allow stop/reboot of existing VMs if local policy permits
- deny new VM creation
- deny migrations
- deny destructive topology mutations unless explicitly forced through operator procedure

The control plane remains authoritative for long-lived desired state. The node may preserve reality temporarily, but must converge back after reconnection.

## Consequences
Pros:
- reduces split-brain risk
- preserves tenant continuity
- makes outage behavior predictable

Cons:
- fewer autonomous repair options during prolonged partition
- forces clear operator procedures for destructive actions

## Guardrails
- no new long-lived desired state may be invented on the node
- all deferred reports must be flushed after reconnect
- stale generations must not be applied
