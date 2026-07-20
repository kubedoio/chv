# ADR-015: Libvirt-First Ecosystem Compatibility

## Status

Proposed

## Date

2026-07-20

## Context

CellHV Core is intended to become a small Linux-native compute runtime that can be used below OpenStack, CloudStack, OpenNebula, O3K, Kubernetes, and CellHV Controller.

Maintaining a separate CellHV integration in every established cloud project creates a serious adoption and maintenance risk. OpenStack, CloudStack, and OpenNebula already have mature libvirt-based paths. Libvirt also already contains a Cloud Hypervisor driver.

However, allowing libvirt or the existing Cloud Hypervisor driver to manage Cloud Hypervisor processes directly would bypass CellHV's durable local authority, operation journal, recovery logic, and provider contracts.

The project therefore needs one ecosystem bridge without creating two runtime authorities.

## Decision

1. The native CellHV REST/OpenAPI API remains the canonical Core API.
2. Libvirt is the primary compatibility strategy for existing cloud platforms.
3. CellHV will target a real libvirt hypervisor driver, provisionally exposed as `cellhv:///system`.
4. The driver delegates all mutations to CellHV Core.
5. The driver does not launch Cloud Hypervisor directly and does not own a second VM database.
6. The existing libvirt `ch` driver is used as a reference, code-sharing candidate, and experimental baseline.
7. OpenStack, CloudStack, and OpenNebula are first tested through their existing libvirt paths without CellHV-specific platform adapters.
8. Small upstream generalisations are preferred over downstream platform plugins.
9. A platform-specific adapter requires a separate ADR after a real libvirt gap is proven.
10. XenAPI is not a Core 1.0 compatibility target.

## Rationale

This approach concentrates compatibility maintenance in one ecosystem boundary instead of many cloud-specific drivers. It also preserves CellHV's modern native API and local authority.

A superficial fake libvirt REST API is rejected because libvirt consumers expect the actual libvirt library, connection URIs, domain XML, events, statistics, and error behavior.

Using the existing `ch:///system` driver unchanged as the production path is also rejected because it would manage Cloud Hypervisor outside CellHV Core.

## Consequences

### Positive

- Existing libvirt clients can be reused.
- OpenStack, CloudStack, and OpenNebula may require configuration or small generic changes rather than new CellHV drivers.
- One compatibility profile can serve multiple ecosystems.
- CellHV remains free to expose a smaller native API.
- Unsupported legacy features remain outside Core.

### Negative

- A libvirt driver is still a substantial maintained integration.
- The full libvirt surface is not feasible; a bounded profile is required.
- Established platforms may contain QEMU-specific assumptions.
- Upstream acceptance is not guaranteed.
- A downstream incubation package may be necessary.
- Mixed native/libvirt concurrency requires strict conflict handling.

## Alternatives considered

### Native API plus one adapter per cloud

Rejected as the default because it multiplies maintenance and weakens adoption. Retained as a fallback.

### Use the existing libvirt `ch` driver directly

Rejected as the final architecture because it bypasses Core authority. Retained for experiments and potential code sharing.

### Reimplement libvirt RPC

Rejected because of scope, compatibility burden, and security risk.

### Adopt XenAPI

Rejected because it imports Xen pool and object semantics and has less alignment with the Linux/KVM ecosystem targeted here.

### Make libvirt authoritative and remove native Core authority

Rejected because it weakens CellHV's recovery, operation, and standalone product model.

## Acceptance conditions

This ADR can move to Accepted when:

- the v1 compatibility contract is reviewed;
- the authority path is proven with a prototype;
- `virsh` lifecycle operations reach Core's operation journal;
- direct Cloud Hypervisor access is absent from the compatibility driver;
- conflict behavior between native and libvirt clients is demonstrated;
- a first OpenStack or OpenNebula experiment runs through the libvirt path;
- ownership for upstream/downstream driver maintenance is named.

## Follow-up decisions

- upstream driver name and URI;
- code sharing with libvirt `ch`;
- supported libvirt version range;
- upstreaming strategy;
- packaging and service topology;
- first platform fallback decision, if any.
