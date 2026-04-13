# ADR-007 — Upgrade and Rollback Policy

## Status
Accepted

## Context
Multiple moving parts exist on each node: control plane, `chv-agent`, `chv-stord`, `chv-nwd`, Cloud Hypervisor, and host helper tooling. Version skew must be controlled.

## Decision

### Upgrade model
- bundle-tested node upgrades are the default
- selective independent component upgrades are allowed only inside a compatibility matrix

### Compatibility scope
The matrix must track:
- control plane version range
- `chv-agent` version
- `chv-stord` version
- `chv-nwd` version
- Cloud Hypervisor version
- host helper bundle version

### Rollback promise
- one-step rollback to the previous tested node bundle

### Service-specific policies
- `chv-stord` may be upgraded independently if the matrix allows it
- `chv-nwd` uses drain-and-replace in MVP-1

## Consequences
Pros:
- predictable rollouts
- safer incident response
- cleaner support boundaries

Cons:
- compatibility testing is mandatory
- more release discipline is required

## Rules
- rollback targets must exist in the compatibility matrix
- component upgrades outside the matrix are unsupported
