# ADR-007 — Upgrade and Rollback Policy

## Status
Accepted

## Date
2026-04-13

## Context
Multiple moving parts exist on each node: control plane, `chv-agent`, storage
services, `chv-nwd`, Cloud Hypervisor, and host helper tooling. Version skew
must be controlled. ADR-028 selects Volvisor as the target storage authority;
`chv-stord` remains in the matrix while legacy-owned volumes are supported.

## Decision

### Upgrade model
- bundle-tested node upgrades are the default
- selective independent component upgrades are allowed only inside a compatibility matrix

### Compatibility scope
The matrix must track:
- control plane version range
- `chv-agent` version
- Volvisor API and daemon version (once integrated)
- `chv-stord` version
- `chv-nwd` version
- Cloud Hypervisor version
- host helper bundle version

### Rollback promise
- one-step rollback to the previous tested node bundle

### Service-specific policies
- Volvisor and `chv-agent` must be upgraded as a tested compatible pair unless
  the matrix explicitly allows otherwise
- `chv-stord` may be upgraded independently if the matrix allows it while
  legacy-owned volumes remain
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

## Related ADRs
- **ADR-002** defines desired-state generation strings. During rollback, the control plane must ensure that generation strings remain monotonic and that stale generations are rejected, even across version skew.
- **ADR-001** defines `chv-agent` supervision of node services. Independent
  component upgrades (permitted inside the compatibility matrix) must
  coordinate with supervision so that the agent does not incorrectly restart
  or fail a subordinate undergoing a controlled upgrade.
