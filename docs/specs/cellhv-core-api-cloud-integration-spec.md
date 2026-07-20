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

Libvirt is the primary compatibility strategy for established cloud-management ecosystems. The preferred public libvirt connection remains the existing upstream `ch:///system` URI. Platform-specific CellHV drivers and a separate `cellhv:///system` URI are fallback integrations requiring separate ADRs.

## 2. Why two surfaces

The native API protects the small modern Core from the full historical libvirt surface.

The libvirt surface provides the best chance to reuse existing integrations in OpenStack, CloudStack, OpenNebula, `virsh`, and libvirt language bindings.

Both surfaces MUST converge before any host mutation:

```text
native client -------------------------┐
                                      v
                                 Core service
                                      ^
libvirt client -> ch driver ----------┘
                   CellHV delegation mode
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

The preferred compatibility connection is:

```text
ch:///system
```

CellHV first attempts to extend or package the existing libvirt Cloud Hypervisor driver with a trusted host-local **CellHV delegation mode**.

In delegation mode, the driver:

- preserves the upstream `ch` URI and driver identity;
- parses the supported domain XML subset;
- converts mutating libvirt calls into native Core requests;
- maps Core events and states back to libvirt;
- reports unsupported APIs explicitly;
- uses Core UUIDs as domain UUIDs;
- does not open Cloud Hypervisor sockets;
- does not mutate Linux networking or storage directly;
- contains no independent persistent VM authority;
- exposes diagnostics proving delegation mode is active.

The host-local mode selection mechanism is not yet normative. It may be implemented through libvirt driver configuration, a packaged backend selection, or another trusted local mechanism. It MUST NOT be controlled by guest input or arbitrary cloud request data.

A client opening `ch:///system` sees the existing libvirt Cloud Hypervisor identity. The client does not select direct versus delegated mode.

## 5. Existing `ch` driver and mode separation

Libvirt already provides `ch:///session` and `ch:///system` for Cloud Hypervisor. The existing driver is valuable as:

- the preferred public connection identity;
- a reference for Cloud Hypervisor mapping;
- a source of reusable implementation experience;
- an early compatibility experiment;
- a support-matrix baseline.

Two modes must remain distinct:

### Direct mode

- existing upstream behavior;
- libvirt manages Cloud Hypervisor directly;
- intended for non-CellHV use;
- MUST remain compatible unless an upstream change is explicitly approved.

### CellHV delegation mode

- libvirt preserves the `ch:///system` identity;
- CellHV Core owns VM identity, persistence, operations, runtime recovery, and host mutations;
- the driver translates and projects only;
- direct Cloud Hypervisor management from the driver is forbidden.

Direct and delegated mode MUST NOT manage the same VM UUID, process, API socket, systemd unit, or runtime directory.

A spike MUST decide whether a generic delegation backend is acceptable upstream. If not, the project may maintain a downstream libvirt package. A separate `cellhv:///system` driver remains a last-resort fallback requiring a new ADR.

## 6. Compatibility policy

The published compatibility claim is profile-based:

- **Native API compatible** — passes the native API contract.
- **CellHV libvirt profile v1 compatible** — passes the contract through `ch:///system` with CellHV delegation mode enabled.
- **Platform compatible through libvirt** — a real supported platform passes without a CellHV-specific platform driver.
- **Platform compatible through generic upstream changes** — passes with non-CellHV-specific generalisations accepted by the platform or libvirt project.
- **Platform compatible through fallback adapter** — allowed only after an ADR and separate conformance profile.

CellHV MUST NOT claim complete libvirt compatibility or zero-configuration compatibility.

## 7. OpenStack strategy

Default target:

```text
Nova LibvirtDriver
        |
connection_uri = ch:///system
        |
libvirt Cloud Hypervisor driver
CellHV delegation mode
        |
CellHV Core
```

The first experiment MUST use the upstream Nova LibvirtDriver without a CellHV-specific ComputeDriver.

The experiment records:

- required standard configuration changes;
- whether Nova accepts and preserves the `ch` hypervisor identity;
- generated domain XML;
- generic and QEMU-specific libvirt calls;
- `virt_type`, capability, statistics, image, console, and migration assumptions;
- network and volume assumptions;
- lifecycle, events, and recovery gaps.

Preferred outcomes, in order:

1. works through documented configuration only;
2. small generic upstream Nova or libvirt changes remove a backend assumption;
3. bounded CellHV-specific adapter only after an ADR proves the first two unsafe or impractical.

Using `ch:///system` avoids a new URI identity, but does not guarantee Nova compatibility.

## 8. CloudStack strategy

Default target:

```text
CloudStack KVM agent
        |
libvirt API using ch:///system where configurable
        |
libvirt Cloud Hypervisor driver
CellHV delegation mode
        |
CellHV Core
```

CloudStack has QEMU/KVM-specific behavior, including connection assumptions, hooks, host preparation, image tooling, storage pools, and lifecycle behavior. The first deliverable is a measured gap matrix, not a claim of automatic compatibility.

Preferred outcomes:

1. documented configuration and packaging only;
2. generic CloudStack changes that support non-QEMU libvirt drivers;
3. fallback CellHV extension/plugin only after an ADR.

If the KVM agent cannot configure or use `ch:///system`, the exact hard-coded or semantic blocker must be recorded.

## 9. OpenNebula strategy

The first target is the existing KVM/VMM path with `LIBVIRT_URI = ch:///system`. QEMU-specific template and driver assumptions are measured and generalised where practical.

A dedicated OpenNebula driver is a fallback.

## 10. O3K, Controller, Kubernetes, Terraform, and Designer

These systems SHOULD use the native API because they are new CellHV-controlled integrations and do not benefit from importing libvirt semantics.

O3K is a lightweight OpenStack-compatible control plane and is separate from standalone OpenStack Nova.

Designer targets Controller or O3K, never Core private state.

## 11. Single-writer and mixed-client rules

- Native and libvirt writes are allowed only through the same Core operation service.
- Every delegated libvirt call that mutates state receives a stable idempotency mapping.
- Conflicting native and libvirt writes return an explicit conflict.
- Platform request IDs are preserved as correlation metadata.
- A direct-mode `ch:///system` writer and CellHV delegation mode MUST NOT manage the same VM or runtime resources.
- Core and the libvirt driver fail closed when conflicting ownership is detected.
- Changing driver mode while CellHV or direct-mode domains exist requires an explicit migration or empty-host procedure.
- Mode diagnostics are included in support bundles and qualification evidence.

## 12. Fallback decision gates

### Platform-specific adapter

A platform-specific adapter may be approved only when:

- the libvirt compatibility profile is implemented;
- a real platform test has produced a reproducible gap;
- upstream-generalisation options were evaluated;
- the adapter is smaller and safer than extending compatibility;
- maintenance ownership is named;
- the Core remains platform-neutral;
- a new ADR and acceptance profile are included.

### Separate `cellhv:///system` driver

A separate driver or URI may be approved only when:

- the `ch:///system` delegation prototype has been attempted;
- upstream maintainers reject the architecture or maintaining it would materially endanger direct mode;
- downstream patch maintenance is compared with a separate driver;
- platform configuration and compatibility consequences are documented;
- a new ADR supersedes the preferred-URI part of ADR-015;
- all libvirt and platform acceptance scenarios are updated.

## 13. Release artifacts

A compatibility release publishes:

- native API version and OpenAPI digest;
- Core version and database schema;
- Cloud Hypervisor version;
- libvirt version range;
- libvirt Cloud Hypervisor driver version or downstream package version;
- active backend-mode configuration contract;
- libvirt function support matrix;
- domain XML support matrix;
- direct-mode regression results where the driver is changed;
- platform qualification matrix;
- unsupported features and known deviations;
- upgrade, mode-transition, and rollback instructions.
