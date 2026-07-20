# ADR-015: Libvirt-First Ecosystem Compatibility

## Status

Proposed

## Date

2026-07-20

## Context

CellHV Core is intended to become a small Linux-native compute runtime that can be used below OpenStack, CloudStack, OpenNebula, O3K, Kubernetes, and CellHV Controller.

Maintaining a separate CellHV integration in every established cloud project creates a serious adoption and maintenance risk. OpenStack, CloudStack, and OpenNebula already have mature libvirt-based paths. Libvirt also already contains a Cloud Hypervisor driver exposed through `ch:///session` and `ch:///system`.

A new `cellhv:///system` URI would require a new libvirt driver identity, packaging, configuration, upstream acceptance, and platform qualification. It would add an avoidable compatibility difference before solving the deeper QEMU/KVM assumptions in cloud platforms.

However, allowing the existing Cloud Hypervisor driver to manage Cloud Hypervisor processes directly would bypass CellHV's durable local authority, operation journal, recovery logic, and provider contracts.

The project therefore needs to preserve the existing libvirt identity while ensuring that CellHV Core remains the only authority for CellHV-managed VMs.

## Decision

1. The native CellHV REST/OpenAPI API remains the canonical Core API.
2. Libvirt is the primary compatibility strategy for established cloud platforms.
3. The preferred public compatibility URI is the existing upstream `ch:///system` URI.
4. The existing libvirt Cloud Hypervisor driver identity and normal libvirt client behavior are preserved wherever technically possible.
5. CellHV will first attempt to add a host-local **CellHV delegation mode** to the libvirt Cloud Hypervisor driver.
6. In CellHV delegation mode, every mutating libvirt call delegates to CellHV Core and creates or reuses a Core operation.
7. In CellHV delegation mode, the libvirt driver does not launch Cloud Hypervisor directly, does not mutate Linux networking or storage directly, and does not own a second VM database.
8. Upstream direct mode remains a separate behavior for non-CellHV Cloud Hypervisor users and MUST NOT be silently changed.
9. Direct mode and CellHV delegation mode MUST NOT manage the same VM or runtime namespace concurrently.
10. The exact host-local mode-selection mechanism is a candidate implementation. It MUST NOT require a new URI scheme for the preferred path.
11. OpenStack, CloudStack, and OpenNebula are first tested through their existing libvirt paths using `ch:///system`, without CellHV-specific platform adapters.
12. Small generic upstream generalisations are preferred over downstream platform plugins.
13. A platform-specific adapter requires a separate ADR after a real libvirt-path gap is proven.
14. A separate `cellhv:///system` driver is a last-resort fallback and requires a new ADR demonstrating that preserving `ch:///system` is unsafe, unacceptable upstream, or technically impossible.
15. XenAPI is not a Core 1.0 compatibility target.

## Rationale

This approach concentrates compatibility maintenance in one ecosystem boundary instead of many cloud-specific drivers while avoiding a new libvirt URI and driver identity.

A superficial fake libvirt REST API is rejected because libvirt consumers expect the actual libvirt library, connection URIs, domain XML, events, statistics, errors, and driver semantics.

Using upstream `ch:///system` in its current direct mode as the CellHV production path is also rejected because direct mode manages Cloud Hypervisor outside CellHV Core.

The preferred solution is therefore not to replace the URI, but to introduce an explicit backend/delegation mode behind the existing URI. This preserves compatibility at the connection layer without creating two runtime authorities.

The project is honest that preserving the URI does not guarantee platform compatibility. OpenStack, CloudStack, and OpenNebula may still contain QEMU-specific assumptions that require configuration or generic upstream changes.

## Consequences

### Positive

- Existing libvirt clients continue to use the upstream `ch:///system` identity.
- No new URI scheme is required for the preferred compatibility path.
- `virsh` and language bindings can reuse the existing libvirt connection model.
- OpenStack and OpenNebula may require only standard URI configuration plus backend-generalisation fixes.
- One compatibility profile can serve multiple ecosystems.
- CellHV remains free to expose a smaller native API.
- Unsupported legacy features remain outside Core.

### Negative

- Extending an existing upstream driver with a delegating backend may be difficult to justify or accept upstream.
- A downstream libvirt package may be required if the upstream project rejects the delegation model.
- The full libvirt surface is not feasible; a bounded profile is required.
- Established platforms may contain QEMU-specific assumptions unrelated to the URI.
- Mode selection, packaging, upgrades, and diagnostics become part of the support contract.
- Mixed native/libvirt concurrency requires strict conflict handling.
- A separate `cellhv:///system` driver may still be required as a fallback.

## Mode and authority rules

- `ch:///system` direct mode and CellHV delegation mode are explicit and mutually exclusive for CellHV-owned VM identities.
- Delegation mode MUST be enabled by host-local trusted configuration or packaging, not by untrusted guest or cloud request data.
- A client opening `ch:///system` does not choose whether Core is bypassed.
- In delegation mode, the driver MUST fail closed when CellHV Core is unavailable for a mutating request.
- Existing running VMs MUST remain running when Core, libvirt, or the management platform restarts.
- Read operations MAY use a bounded driver-side projection, but authoritative identity and configuration come from Core.
- The driver MUST expose enough diagnostic metadata to prove which backend mode is active without changing the public URI.

## Alternatives considered

### Native API plus one adapter per cloud

Rejected as the default because it multiplies maintenance and weakens adoption. Retained as a fallback.

### Create `cellhv:///system` immediately

Rejected as the preferred path because it creates a new driver identity and avoidable configuration/upstream work. Retained only as a separately approved fallback.

### Use upstream `ch:///system` direct mode unchanged

Rejected as the CellHV production architecture because it bypasses Core authority. Retained as a baseline, compatibility experiment, and non-CellHV direct mode.

### Reimplement libvirt RPC

Rejected because of scope, compatibility burden, and security risk.

### Adopt XenAPI

Rejected because it imports Xen pool and object semantics and has less alignment with the Linux/KVM ecosystem targeted here.

### Make libvirt authoritative and remove native Core authority

Rejected because it weakens CellHV's recovery, operation, and standalone product model.

## Acceptance conditions

This ADR can move to Accepted when:

- the v1 compatibility contract is reviewed;
- upstream `ch:///system` behavior and support are inventoried;
- a prototype opens `ch:///system` in CellHV delegation mode;
- `virsh` lifecycle mutations reach Core's operation journal;
- direct Cloud Hypervisor access is absent from the driver in delegation mode;
- direct mode behavior is not regressed for non-CellHV users;
- mode selection and ownership-conflict behavior are demonstrated;
- conflict behavior between native and libvirt clients is demonstrated;
- a first OpenStack or OpenNebula experiment uses `ch:///system`;
- ownership for upstream or downstream libvirt maintenance is named.

## Follow-up decisions

- exact CellHV delegation-mode selection and configuration;
- whether the change can be accepted upstream as a generic backend mode;
- downstream package strategy if upstreaming fails;
- supported libvirt version range;
- code sharing and change scope inside the existing `ch` driver;
- service and package topology;
- diagnostics for direct versus delegated mode;
- first platform fallback decision, if any;
- separate `cellhv:///system` fallback ADR, only if required.
