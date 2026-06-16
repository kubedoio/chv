# ADR-005-Designer Separate Desired Topology Designer from Live Fleet Topology

Date: 2026-06-13
Status: Accepted (2026-06-16, Phase 7)

## Context

CHV already has a live topology view. The new designer is not the same thing. One represents actual state; the other represents desired state.

## Decision

Maintain two separate surfaces:

```text
Architecture Designer
  Desired topology
  Editable
  Saved as CHVArchitecture YAML
  Can be validated, planned and deployed

Fleet Overview / Live Topology
  Actual topology
  Read-only or operational
  Shows current health, resources, events and drift
```

## Rationale

Mixing desired and actual state in one view causes confusion. Operators need to know whether they are editing intent or operating live infrastructure.

## Consequences

- Architecture Designer uses Svelte Flow.
- Existing topology canvas remains current-state oriented.
- Drift view can compare both.
- After deployment, a topology may link to live resources.

## UX rule

Use clear labels:

- `Desired topology`
- `Current fleet state`
- `Drift detected`
- `Not yet deployed`
- `Applied version`
