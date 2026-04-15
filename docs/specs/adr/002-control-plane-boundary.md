# ADR-002 — Control Plane to Node Boundary

## Status
Accepted

## Context
The control plane must remain authoritative for long-lived desired state, while the node must preserve runtime state during outages and reboots. Cloud Hypervisor should not be remotely reachable.

## Decision
The control plane communicates only with `chv-agent`.

### External transport
- gRPC over mTLS
- bootstrap token for initial enrollment
- certificate-based steady-state trust after enrollment

### Local VMM boundary
- only `chv-agent` may call Cloud Hypervisor
- Cloud Hypervisor is accessed only over local Unix API sockets
- Cloud Hypervisor APIs must never be exposed remotely

### Desired state authority
- control plane owns long-lived desired state
- node reports observed state
- node may preserve local runtime state during partition
- node must not invent new long-lived desired state independently

## Consequences
Pros:
- single trusted entrypoint on the node
- easier auditing and authorization
- clean operational and security boundary

Cons:
- `chv-agent` becomes operationally critical
- the local durable cache must be implemented carefully

## Rules
- all remote mutations flow through `chv-agent`
- desired-state generations are monotonic decimal strings
- stale desired-state generations must be rejected cleanly
- non-numeric generation values must be rejected as invalid contract input
- retries should be safe and idempotent where possible
