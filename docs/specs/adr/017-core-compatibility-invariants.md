# ADR-017: Core Compatibility Invariants

## Status

Accepted

## Date

2026-07-21

## Context

ADR-015 combined stable Core safety decisions with unproven cloud-platform integration choices. The stable decisions should not wait for OpenStack, CloudStack, or libvirt experiments, while platform paths must remain evidence-driven.

This ADR extracts the compatibility invariants that are already defensible and required to prevent unsafe implementation shortcuts.

## Decision

1. The native CellHV Core API is the canonical contract for CellHV-controlled clients and adapters.
2. Cloud Hypervisor is the only active VMM target for Core 1.0.
3. CellHV MUST NOT advertise Cloud Hypervisor as `qemu:///system`, QEMU, or QMP-compatible.
4. VMM, hypervisor-interface, network, storage, cloud-platform, workload, and version compatibility are separate claim axes.
5. No compatibility claim may be inferred from a URI, schema, mock, registration, or successful connection alone.
6. Capabilities and statistics MUST reflect executable, measured behavior.
7. Network and storage support MUST be qualified independently from VM lifecycle.
8. Platform-specific code remains outside Core and uses public Core contracts.
9. Core remains the single VM mutation and recovery authority.
10. Other VMM backends are outside the active Core 1.0 programme. Any future proposal requires a separate business case, ADR, implementation plan, and qualification profile.
11. Protocol or identity emulation around Cloud Hypervisor is not an accepted route to ecosystem compatibility.

## Rationale

These rules are independent of which OpenStack, CloudStack, OpenNebula, or libvirt path eventually succeeds. Accepting them now prevents false compatibility claims, duplicate authorities, and a large emulation surface from entering the implementation.

## Consequences

### Positive

- implementation can begin with stable safety boundaries;
- cloud discovery remains free to select the smallest maintainable path;
- support claims become precise and auditable;
- future prompts cannot pull additional VMMs into the active scope;
- provider tests remain independent from VMM tests.

### Negative

- no universal compatibility URI is promised;
- some platforms may require maintained adapters;
- cloud-platform support requires real T5 laboratories and evidence;
- unsupported features must remain visible rather than being approximated.

## Rejected alternatives

### Delay all compatibility decisions until cloud experiments finish

Rejected because truthful identity and separate compatibility axes are safety constraints, not platform-specific choices.

### Treat `ch:///system` as universal compatibility

Rejected because libvirt recognition does not prove cloud-platform behavior.

### Expose Cloud Hypervisor through QEMU identity

Rejected because it would promise semantics that are not implemented.

### Keep another VMM in the active Core 1.0 topology

Rejected because it creates expectation and scope debt before the primary runtime is stable.

## Acceptance conditions

This ADR is accepted immediately. Compliance is enforced through:

- static identity and dependency guards;
- compatibility-claim schema validation;
- real VMM identity checks;
- independent network/storage profiles;
- platform-specific T5 qualification;
- review of every advertised capability and support tuple.
