# ADR-015: Layered Ecosystem Compatibility and Honest VMM Identity

## Status

Proposed

## Date

2026-07-20

## Context

CellHV Core is intended to be a small, autonomous Linux-native compute runtime beneath CellHV Controller, O3K, OpenStack, CloudStack, OpenNebula, Kubernetes, and other management systems.

The major Linux cloud platforms have mature QEMU/KVM integrations, usually through libvirt. The upstream libvirt Cloud Hypervisor driver exposes `ch:///system`, but this URI is not widely qualified as a production backend by those platforms. It has a much smaller feature surface than the libvirt QEMU driver, and platform code frequently contains QEMU-specific assumptions that are independent of the URI.

Using `qemu:///system` would improve superficial recognition, but the URI selects the libvirt QEMU hypervisor driver and implies QEMU process, QMP, device, migration, block-job, guest-agent, capability, and error semantics. Advertising that identity while actually running Cloud Hypervisor would create false compatibility and unsafe failure modes.

Networking and storage are separate compatibility concerns. Libvirt has distinct hypervisor, network, storage, interface, node-device, secret, and filtering drivers. CellHV's network and storage capabilities therefore do not justify identifying the VMM as QEMU.

The project needs an evidence-driven compatibility strategy that preserves a small Core without making false compatibility claims.

## Decision

1. The native CellHV REST/OpenAPI API is the canonical Core contract.
2. Cloud Hypervisor remains the primary VMM for CellHV Core 1.0.
3. CellHV MUST NOT impersonate `qemu:///system` while running Cloud Hypervisor.
4. The existing `ch:///system` path is an optional libvirt compatibility profile and experiment, not a universal or mandatory cloud-platform integration strategy.
5. `ch:///system` compatibility is claimed only for the explicitly published CellHV Cloud Hypervisor libvirt profile.
6. OpenStack, CloudStack, and OpenNebula compatibility is qualified independently from libvirt URI compatibility.
7. Platform-specific adapters are permitted when real conformance testing proves that generic libvirt integration is insufficient or unsafe.
8. Platform-specific code remains outside Core and communicates through public Core APIs.
9. Network compatibility, storage compatibility, VMM compatibility, and cloud-platform compatibility are separate claim axes.
10. A future actual QEMU backend may be added behind the same Core operation and recovery model, but only through a separate ADR, implementation profile, and complete qualification.
11. A future QEMU backend MUST use real QEMU/libvirt QEMU semantics. It is not a protocol-emulation shim around Cloud Hypervisor.
12. XenAPI and XAPI compatibility are not Core 1.0 targets.
13. No compatibility claim may be inferred from a URI, schema, mock, or successful connection alone.

## Rationale

CellHV's differentiator is a small autonomous compute runtime with durable local authority and modern Linux integration. False QEMU identity would exchange that clarity for fragile compatibility.

`ch:///system` remains useful for:

- `virsh` and libvirt language bindings;
- understanding the upstream Cloud Hypervisor/libvirt gap;
- platforms that already support configurable non-QEMU libvirt backends;
- contributing generic Cloud Hypervisor improvements upstream.

It is not treated as the sole route to cloud adoption.

Platform adapters are not considered architectural failure. A small maintained adapter can be safer and cheaper than emulating QEMU or importing the complete libvirt QEMU surface.

## Compatibility axes

Every published compatibility claim MUST identify all applicable axes:

### VMM backend

- Cloud Hypervisor;
- future qualified QEMU backend;
- other future backend approved by ADR.

### Hypervisor management interface

- native CellHV API;
- bounded `ch:///system` libvirt profile;
- future real `qemu:///system` profile only with the actual QEMU backend;
- platform-specific adapter.

### Network path

- pre-existing bridge/TAP;
- CellHV Linux network provider;
- standard libvirt network driver;
- external SDN integration.

### Storage path

- pre-existing file or block device;
- CellHV storage provider;
- standard libvirt storage driver;
- external storage integration.

### Cloud-platform path

- native CellHV adapter;
- generic libvirt path;
- generic upstream platform change;
- future actual QEMU backend;
- unsupported.

A platform is compatible only for a published tuple of these axes.

## Consequences

### Positive

- Core stays small and VMM identity remains truthful.
- Cloud Hypervisor differentiation is preserved.
- Network and storage work can evolve independently.
- Platform-specific integrations are allowed when they are the lowest-risk solution.
- A future QEMU backend remains possible without contaminating the Cloud Hypervisor path.
- Compatibility claims become measurable and supportable.

### Negative

- No single URI guarantees broad cloud-platform compatibility.
- OpenStack, CloudStack, and OpenNebula may each require separate work.
- More than one integration package may eventually be maintained.
- Cloud Hypervisor adoption depends partly on upstream platform generalisation.
- A future QEMU backend would add a second runtime qualification burden.

## Rejected alternatives

### Treat `ch:///system` as a universal compatibility standard

Rejected because upstream libvirt recognition does not equal platform qualification, and major platforms contain QEMU-specific assumptions.

### Present Cloud Hypervisor through `qemu:///system`

Rejected because it would falsely promise QEMU process and feature semantics and require either invasive libvirt changes or a large QEMU/QMP emulation layer.

### Reimplement QEMU/QMP compatibility around Cloud Hypervisor

Rejected because it would become a second VMM project and contradict the small-Core goal.

### Require one CellHV adapter for every platform from the start

Rejected as a default roadmap. Adapters are implemented only after measured gap analysis and maintenance ownership.

### Make libvirt authoritative

Rejected because CellHV Core must own VM identity, operations, recovery, and process adoption.

## Acceptance conditions

This ADR can move to Accepted when:

- the compatibility-claims contract is reviewed;
- the native Core authority path is demonstrated;
- the upstream `ch` support matrix is measured;
- one real OpenStack and one real CloudStack discovery run produce gap reports;
- the project selects the first supported integration path for each target;
- no code path advertises QEMU identity while using Cloud Hypervisor;
- network and storage compatibility are tested separately from VM lifecycle;
- maintenance ownership is named for every advertised integration.

## Follow-up decisions

- whether to implement and maintain the bounded `ch:///system` profile;
- OpenStack integration path;
- CloudStack integration path;
- OpenNebula integration path;
- future actual QEMU backend business case;
- libvirt network/storage coexistence model;
- supported version matrices;
- integration package ownership and upstreaming strategy.
