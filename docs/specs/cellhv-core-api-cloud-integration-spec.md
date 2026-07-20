# CellHV Core API and Ecosystem Compatibility Specification

**Status:** Proposed  
**Date:** 2026-07-20  
**Depends on:**

- `docs/specs/cellhv-core-foundation-spec.md`
- `docs/specs/adr/015-libvirt-first-ecosystem-compatibility.md`
- `docs/specs/contracts/cellhv-libvirt-compatibility-profile-v1.md`

## 1. Decision

CellHV exposes:

1. a **native REST/OpenAPI API** that is authoritative and optimised for CellHV-native clients;
2. a **libvirt compatibility facade** that translates a bounded libvirt profile into the same Core resources and operation journal.

Libvirt is the primary compatibility strategy for established cloud-management ecosystems. Platform-specific CellHV drivers are fallback integrations and require a separate ADR.

## 2. Why two surfaces

The native API protects the small modern Core from the full historical libvirt surface.

The libvirt surface provides the best chance to reuse existing integrations in OpenStack, CloudStack, OpenNebula, `virsh`, and libvirt language bindings.

Both surfaces MUST converge before any host mutation:

```text
native client ----------------┐
                              v
                         Core service
                              ^
libvirt client -> driver -----┘
```

There must never be two independent process managers.

## 3. Native API contract

The native API uses HTTP/JSON and OpenAPI 3.1.

Transports:

- local HTTP over Unix-domain socket;
- optional HTTPS/mTLS for managed access.

Core resources:

```text
GET    /v1/system
GET    /v1/host
GET    /v1/host/capabilities

POST   /v1/vms
GET    /v1/vms
GET    /v1/vms/{vm_id}
PATCH  /v1/vms/{vm_id}
DELETE /v1/vms/{vm_id}

POST   /v1/vms/{vm_id}/actions/start
POST   /v1/vms/{vm_id}/actions/stop
POST   /v1/vms/{vm_id}/actions/reboot
POST   /v1/vms/{vm_id}/actions/pause
POST   /v1/vms/{vm_id}/actions/resume

POST   /v1/vms/{vm_id}/disks
DELETE /v1/vms/{vm_id}/disks/{attachment_id}
POST   /v1/vms/{vm_id}/nics
DELETE /v1/vms/{vm_id}/nics/{attachment_id}

GET    /v1/operations/{operation_id}
GET    /v1/events
GET    /v1/events/watch
```

Mutations use:

- durable operation resources;
- `202 Accepted`;
- `Idempotency-Key`;
- `ETag` and `If-Match`;
- `application/problem+json`;
- stable correlation IDs.

## 4. Libvirt architecture decision

CellHV MUST NOT reimplement libvirt's public library or XDR remote protocol.

The target is a real libvirt hypervisor driver, provisionally:

```text
cellhv:///system
```

The driver:

- parses the supported domain XML subset;
- converts libvirt calls into native Core requests;
- maps Core events and states back to libvirt;
- reports unsupported APIs explicitly;
- uses Core UUIDs as domain UUIDs;
- does not open Cloud Hypervisor sockets;
- does not mutate Linux networking or storage directly;
- contains no independent persistent VM authority.

The driver may initially incubate as a downstream package. Upstreaming to libvirt is the intended long-term path.

## 5. Existing `ch` driver

Libvirt already provides `ch:///session` and `ch:///system` for Cloud Hypervisor. It is valuable as:

- a reference for Cloud Hypervisor mapping;
- a source of reusable implementation experience;
- an early compatibility experiment;
- a support-matrix baseline.

It is not sufficient as the final CellHV compatibility layer if it bypasses Core state, recovery, operations, and providers.

A spike MUST decide whether code can be shared or the existing driver can gain a backend mode without compromising generic libvirt design. If not, a separate `cellhv` driver is required.

## 6. Compatibility policy

The published compatibility claim is profile-based:

- **Native API compatible** — passes the native API contract.
- **CellHV libvirt profile v1 compatible** — passes the contract in `cellhv-libvirt-compatibility-profile-v1.md`.
- **Platform compatible through libvirt** — a real supported platform passes without a CellHV-specific platform driver.
- **Platform compatible through fallback adapter** — allowed only after an ADR and separate conformance profile.

CellHV MUST NOT claim complete libvirt compatibility.

## 7. OpenStack strategy

Default target:

```text
Nova LibvirtDriver
        |
libvirt connection configured for CellHV
        |
cellhv libvirt driver
        |
CellHV Core
```

The first experiment MUST use the upstream Nova LibvirtDriver without a CellHV-specific ComputeDriver.

The experiment records:

- configuration-only changes;
- generated domain XML;
- generic and QEMU-specific libvirt calls;
- capability and statistics expectations;
- network and volume assumptions;
- lifecycle, events, console, and recovery gaps.

Preferred outcomes, in order:

1. works through configuration only;
2. small generic upstream Nova changes remove a backend assumption;
3. bounded CellHV-specific adapter only after an ADR proves the first two unsafe or impractical.

## 8. CloudStack strategy

Default target:

```text
CloudStack KVM agent
        |
libvirt API
        |
cellhv libvirt driver
        |
CellHV Core
```

CloudStack has QEMU/KVM-specific behavior, including hooks and host preparation. The first deliverable is a measured gap matrix, not a claim of automatic compatibility.

Preferred outcomes:

1. configuration and packaging only;
2. generic CloudStack changes that support non-QEMU libvirt drivers;
3. fallback CellHV extension/plugin only after an ADR.

## 9. OpenNebula strategy

The first target is the existing KVM/VMM path with `LIBVIRT_URI` set to the CellHV connection. QEMU-specific template and driver assumptions are measured and generalised where practical.

A dedicated OpenNebula driver is a fallback.

## 10. O3K, Controller, Kubernetes, and Terraform

These systems SHOULD use the native API because they are new CellHV-controlled integrations and do not benefit from importing libvirt semantics.

Designer targets Controller or O3K, never Core private state.

## 11. Single-writer and mixed-client rules

- Native and libvirt writes are allowed only through the same Core operation service.
- Every libvirt call that mutates state receives a stable idempotency mapping.
- Conflicting native and libvirt writes return an explicit conflict.
- Platform request IDs are preserved as correlation metadata.
- A direct `ch:///system` writer and `cellhvd` MUST NOT manage the same VM.
- Core startup fails closed when conflicting ownership is detected.

## 12. Fallback-adapter decision gate

A platform-specific adapter may be approved only when:

- the libvirt compatibility profile is implemented;
- a real platform test has produced a reproducible gap;
- upstream-generalisation options were evaluated;
- the adapter is smaller and safer than extending compatibility;
- maintenance ownership is named;
- the Core remains platform-neutral;
- a new ADR and acceptance profile are included.

## 13. Release artifacts

A compatibility release publishes:

- native API version and OpenAPI digest;
- Core version and database schema;
- Cloud Hypervisor version;
- libvirt version range;
- CellHV libvirt driver version;
- libvirt function support matrix;
- domain XML support matrix;
- platform qualification matrix;
- unsupported features and known deviations;
- upgrade and rollback instructions.
