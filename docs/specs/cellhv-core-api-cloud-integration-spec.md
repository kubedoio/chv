# CellHV Core API and Cloud Integration Specification

**Status:** Proposed  
**Date:** 2026-07-21  
**Depends on:** ADR-015, ADR-016, ADR-017, and the compatibility-claims contract

## 1. Runtime identity

The existing `chv-agent` evolves into CellHV Core and remains the single runtime authority.

- no parallel `cellhvd` service is introduced;
- current control-plane gRPC compatibility and the native local API enter one operation engine;
- integrations never access the agent database, VMM sockets, or provider private APIs;
- Controller and cloud platforms remain optional clients.

## 2. Canonical API semantics

The native CellHV API is the canonical contract for CellHV-controlled clients and platform adapters.

Current default:

- HTTP/JSON;
- OpenAPI 3.1;
- local Unix-socket transport;
- optional HTTPS/mTLS after standalone recovery is proven;
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

The exact transport remains subject to prototype validation, but the semantics above and the single-operation-authority rule are required.

## 3. VMM policy

Cloud Hypervisor is the only active Core 1.0 VMM target.

The internal VMM boundary exposes only required Core operations. CellHV MUST NOT:

- route Cloud Hypervisor through `qemu:///system`;
- emulate QMP or QEMU monitor behavior;
- advertise QEMU capabilities;
- include another VMM in the active Core 1.0 implementation plan.

Other VMMs are outside this specification and require a separate future programme.

## 4. Libvirt policy

The upstream `ch:///system` path is an optional bounded experiment and compatibility profile.

Possible discovery outcomes:

1. useful without CellHV-specific platform code;
2. useful after small generic upstream changes;
3. useful only for `virsh` or SDK clients;
4. too limited or costly to productise.

A successful connection proves only the libvirt-interface profile. It does not prove OpenStack, network, storage, recovery, or platform support.

## 5. Network integration

Core network attachments consume concrete, owned endpoints such as:

- existing bridge;
- existing TAP;
- selected `chv-nwd` managed endpoint;
- external SDN-prepared endpoint.

For the active programme, only the minimum path required by the selected OpenStack integration is implemented and qualified.

Ownership, recovery, detach, cleanup, and unrelated-host-state preservation are mandatory.

## 6. Storage integration

Core storage attachments consume concrete, owned endpoints such as:

- file path;
- block device;
- read-only block device;
- selected `chv-stord` provider handle;
- external platform-prepared handle.

For the active programme, only the minimum path required by the selected OpenStack integration is implemented and qualified.

Core never infers storage ownership from the VMM URI.

## 7. OpenStack discovery and integration

OpenStack is the first external cloud target.

### Discovery candidates

#### Path A — generic libvirt Cloud Hypervisor

- upstream Nova `LibvirtDriver`;
- `ch:///system`;
- no CellHV-specific Nova driver;
- measure QEMU-specific assumptions and Core-authority impact.

#### Path B — generic upstream generalisation

- small non-CellHV-specific Nova/libvirt changes;
- realistic upstream maintenance path;
- no Core authority bypass.

#### Path C — official CellHV Nova driver

- bounded Nova `ComputeDriver`;
- uses the public `chv-agent` Core API;
- maintained and conformance-tested by CellHV;
- selected when safer and smaller than reproducing libvirt/QEMU behavior.

The time-boxed discovery produces evidence before selecting a path.

### Qualification requirements

The selected path must qualify independently:

- Nova lifecycle and Placement reporting;
- Neutron network mapping;
- Cinder storage mapping;
- retries and nova-compute restart;
- `chv-agent` restart and host reboot;
- manager outage without workload loss;
- exact versions and unsupported features;
- maintenance ownership.

## 8. CellHV Controller and O3K

These are CellHV-controlled integrations and use the native Core authority path.

- Controller stores fleet projections, not the only VM record.
- O3K is a lightweight OpenStack-compatible control plane and is distinct from standalone OpenStack Nova integration.
- Both must tolerate `chv-agent` and management-plane restarts without duplicate VM actions.
- Neither may access private Core state or VMM/provider sockets.

## 9. Deferred integrations

The following are strategic targets but outside the active Core 1.0 implementation sequence:

- CloudStack;
- OpenNebula;
- Kubernetes;
- Terraform/OpenTofu;
- Designer execution integration;
- broad libvirt productisation;
- additional VMMs.

Each deferred target requires its own discovery evidence, resource commitment, prompts, acceptance profile, and maintenance owner.

## 10. Platform adapter rules

A platform adapter:

- remains outside `chv-agent` Core;
- uses public Core APIs;
- maps platform idempotency and identity into Core;
- has named maintenance ownership;
- publishes an exact version matrix;
- cannot bypass provider ownership rules;
- has real platform conformance tests;
- is preferred over false VMM identity or unsafe protocol emulation.

## 11. Compatibility decision record

Every platform evaluation publishes:

```yaml
platform:
platform_version:
core_version:
vmm_backend:
hypervisor_interface:
network_path:
storage_path:
integration_candidate:
configuration_changes:
platform_code_changes:
generic_upstream_option:
qemu_specific_assumptions:
core_authority_impact:
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
- installation, upgrade, rollback, and troubleshooting guides;
- maintenance owner and evidence digest.
