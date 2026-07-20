# CellHV Core API and Cloud Integration Specification

**Status:** Proposed  
**Date:** 2026-07-20  
**Depends on:** ADR-015 and the compatibility-claims contract

## 1. Canonical API

The native CellHV API is the authoritative integration contract.

- HTTP/JSON;
- OpenAPI 3.1;
- local Unix-socket transport;
- optional HTTPS/mTLS;
- durable asynchronous operations;
- idempotency keys;
- resource versions;
- structured problem responses;
- event/watch interface.

Representative resources:

```text
GET    /v1/system
GET    /v1/host
GET    /v1/host/capabilities
POST   /v1/vms
GET    /v1/vms/{id}
PATCH  /v1/vms/{id}
DELETE /v1/vms/{id}
POST   /v1/vms/{id}/actions/{start|stop|reboot|pause|resume}
POST   /v1/vms/{id}/disks
POST   /v1/vms/{id}/nics
GET    /v1/operations/{id}
GET    /v1/events
```

Every integration path converges on the same Core operation service.

## 2. VMM backend policy

Cloud Hypervisor is the first backend.

The internal VMM adapter exposes only Core-required operations. It does not expose the complete Cloud Hypervisor or QEMU feature surface.

A future QEMU backend:

- requires a separate ADR;
- runs actual QEMU;
- may use libvirt QEMU APIs where appropriate;
- must preserve Core authority and recovery;
- receives a separate support and test matrix.

CellHV MUST NOT route Cloud Hypervisor through `qemu:///system` or emulate QMP.

## 3. Libvirt policy

The upstream `ch:///system` driver is evaluated as an optional compatibility profile.

Possible outcomes:

1. useful for `virsh`, SDKs, or a cloud platform through configuration;
2. useful after generic upstream changes;
3. too limited or costly for a specific platform;
4. not implemented as a supported CellHV profile.

A successful libvirt connection proves only the hypervisor-interface axis. Network, storage, platform, and recovery claims require separate tests.

## 4. Network integration

Core network attachments accept concrete endpoints such as:

- existing bridge;
- existing TAP;
- managed CellHV network endpoint;
- external SDN-prepared endpoint.

Network orchestration may come from:

- CellHV providers;
- standard libvirt network services;
- Neutron or another cloud SDN;
- CloudStack/OpenNebula network orchestration;
- Kubernetes networking integrations.

Ownership, cleanup, and restart recovery are explicit.

## 5. Storage integration

Core storage attachments accept concrete endpoints such as:

- file path;
- block device;
- read-only block device;
- provider handle.

Provisioning may come from:

- CellHV providers;
- standard libvirt storage;
- Cinder;
- CloudStack primary storage;
- OpenNebula datastore drivers;
- external storage systems.

Core does not infer storage ownership from the VMM URI.

## 6. OpenStack strategy

OpenStack is the first cloud target.

The discovery programme evaluates:

### Path A — generic libvirt Cloud Hypervisor

- upstream Nova LibvirtDriver;
- `ch:///system`;
- no CellHV-specific ComputeDriver;
- measure QEMU-specific assumptions.

### Path B — generic upstream generalisation

- small non-CellHV-specific improvements to Nova/libvirt;
- maintainable by the relevant communities;
- still uses the generic libvirt path.

### Path C — official CellHV Nova driver

- a bounded Nova `ComputeDriver`;
- uses the native CellHV API;
- maintained and conformance-tested by CellHV;
- selected when it is safer or smaller than forcing libvirt compatibility.

The selected supported path is based on evidence, not preference. A native adapter is acceptable.

OpenStack networking and storage are qualified through Neutron and Cinder mappings independently from VM lifecycle.

## 7. CloudStack strategy

CloudStack's standard KVM agent is strongly QEMU-oriented. Discovery must measure:

- connection URI configurability;
- QEMU hooks;
- QEMU image tooling;
- storage-pool assumptions;
- CPU/device XML assumptions;
- migration and snapshot behavior;
- network scripts and bridge handling.

Candidate supported paths:

- generic non-QEMU libvirt support;
- CloudStack extension framework;
- native CellHV hypervisor plugin;
- future actual QEMU backend.

CellHV MUST NOT claim CloudStack compatibility merely because the agent can connect to libvirt.

## 8. OpenNebula strategy

Evaluate:

- existing KVM/VMM path with `ch:///system`;
- generic VMM generalisation;
- official CellHV VMM driver using the native API.

Network and datastore paths are qualified independently.

## 9. O3K, Controller, Kubernetes, Terraform, Designer

These are CellHV-controlled integrations and SHOULD use the native API.

- O3K is a lightweight OpenStack-compatible control plane.
- Controller manages fleets through the public API.
- Kubernetes uses an operator or controller.
- Terraform/OpenTofu use a provider generated around the native API.
- Designer targets Controller or O3K, never Core private state.

## 10. Platform adapter rules

A platform adapter:

- remains outside Core;
- uses public Core APIs;
- has named maintenance ownership;
- publishes a version matrix;
- maps platform idempotency and identity into Core;
- cannot bypass network/storage ownership rules;
- has real platform conformance tests;
- is preferred over false VMM identity or unsafe protocol emulation.

## 11. Compatibility decision record

Every platform evaluation publishes:

```yaml
platform:
platform_version:
vmm_backend:
hypervisor_interface:
network_path:
storage_path:
integration_candidate:
configuration_changes:
platform_code_changes:
generic_upstream_option:
qemu_specific_assumptions:
security_risk:
maintenance_cost:
recommended_path:
evidence:
```

## 12. Release artifacts

A supported integration release publishes:

- compatibility claim tuple;
- API and package versions;
- Core and VMM versions;
- network/storage profiles;
- platform matrix;
- known unsupported features;
- installation, upgrade, rollback, and troubleshooting guides.
