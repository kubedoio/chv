# CellHV Libvirt Compatibility Profile v1

**Status:** Proposed  
**Date:** 2026-07-20  
**Authority:** ADR-015  
**Purpose:** define the minimum real libvirt contract CellHV must satisfy before using libvirt as an ecosystem compatibility claim

## 1. Scope

This profile is intentionally smaller than the complete libvirt API support matrix. A function is supported only when listed here and covered by acceptance tests.

Preferred target connection:

```text
ch:///system
```

The connection uses the existing libvirt Cloud Hypervisor driver identity with **CellHV delegation mode** enabled by trusted host-local configuration or packaging.

The public URI MUST remain `ch:///system` for the preferred v1 profile. A separate `cellhv:///system` connection is outside this contract and requires another ADR.

Remote transports may be added later through standard libvirt remote mechanisms. They are not required for v1.

## 2. Driver mode contract

The libvirt Cloud Hypervisor driver has two conceptually distinct modes:

- **direct mode** — existing upstream behavior; libvirt manages Cloud Hypervisor directly;
- **CellHV delegation mode** — libvirt translates supported calls into CellHV Core operations.

For this compatibility profile:

- CellHV delegation mode MUST be active;
- the client MUST NOT select the mode through domain XML, URI query data, or cloud request input;
- the active mode MUST be visible through trusted diagnostics and qualification evidence;
- mode selection MUST be host-local and privileged;
- direct mode and CellHV delegation mode MUST NOT manage the same VM identity or runtime resources;
- changing mode with existing domains requires a documented migration or empty-host procedure;
- modifying the driver for delegation MUST NOT silently regress the qualified upstream direct-mode profile.

## 3. Authority contract

- The libvirt driver is stateless except for bounded libvirt-required runtime bookkeeping and caches.
- CellHV Core owns persistent VM identity and accepted configuration.
- Every mutating libvirt call creates or reuses a Core operation.
- Domain UUID equals Core VM UUID.
- Driver restart rebuilds its projection from Core.
- The driver does not access the Core database.
- The driver does not open Cloud Hypervisor sockets in CellHV delegation mode.
- The driver does not mutate networking or storage except through Core attachment requests.
- The driver does not launch, stop, or signal Cloud Hypervisor directly in CellHV delegation mode.
- Conflicting native and libvirt mutations fail explicitly.
- Core unavailability blocks new mutations but MUST NOT stop existing VMs.

## 4. Connection and capability APIs

Required:

- `virConnectOpen` / `virConnectOpenAuth` as applicable using `ch:///system`;
- `virConnectClose`;
- `virConnectGetType` preserving the upstream Cloud Hypervisor driver identity;
- `virConnectGetVersion`;
- `virConnectGetLibVersion`;
- `virConnectGetCapabilities`;
- `virConnectGetDomainCapabilities`;
- `virConnectIsAlive`;
- `virConnectListAllDomains`;
- `virConnectGetMaxVcpus`;
- basic node information and free-memory reporting where accurate.

The exact upstream return value for `virConnectGetType` is recorded during the support-matrix inventory and MUST NOT be changed merely to advertise CellHV branding.

Capabilities MUST advertise only executable behavior. CellHV delegation mode MAY be reported through a namespaced capability or diagnostic field where this can be done without breaking normal libvirt consumers.

## 5. Domain identity and lookup

Required:

- lookup by UUID;
- lookup by name;
- lookup by ID for running domains;
- persistent inactive domain listing;
- stable UUID and name across libvirt-driver, Core, and host restart.

Duplicate UUID or conflicting names return standard libvirt errors.

## 6. Supported domain XML subset

Required elements:

- `<name>`;
- `<uuid>`;
- `<memory>` and `<currentMemory>`;
- `<vcpu>`;
- `<os><type>hvm</type></os>`;
- supported firmware/kernel boot references;
- `<disk type='file'>` with raw virtio disk;
- `<disk type='block'>` with virtio disk;
- `<interface type='bridge'>` with virtio model;
- `<interface type='ethernet'>` or pre-created TAP where qualified;
- `<serial>` and `<console>` for the qualified console method;
- opaque metadata under a CellHV namespace where safe.

The driver MUST reject unsupported XML before creating a Core operation that mutates host state. It MUST NOT silently ignore unsupported devices.

The first profile does not require IDE, SATA, SCSI emulation, USB, sound, graphics stacks, legacy BIOS devices, arbitrary PCI topology, QEMU command-line injection, or arbitrary emulator paths.

## 7. Domain lifecycle APIs

Required:

- define;
- undefine;
- create/start;
- graceful shutdown;
- force destroy;
- reboot;
- suspend/pause;
- resume;
- get state;
- autostart flag only after host-reboot semantics are qualified.

Lifecycle events are required for started, stopped, suspended, resumed, crashed, and undefined transitions where observable.

Every mutating lifecycle request MUST carry or derive a stable Core idempotency identity and MUST be correlatable with the resulting Core operation.

## 8. Device operations

Required after the corresponding Core provider is qualified:

- attach/detach supported virtio file or block disk;
- attach/detach supported virtio NIC;
- live versus config flags must be handled explicitly;
- unsupported flag combinations return `VIR_ERR_NO_SUPPORT` or the narrowest valid error.

No storage-pool or virtual-network implementation is required in the CellHV hypervisor path. Standard libvirt storage/network drivers may coexist when their output is passed to Core as a pre-existing attachment and ownership is explicit.

## 9. Statistics and observability

Required:

- domain state;
- vCPU count;
- configured and observed memory;
- process/CPU time where accurately available;
- basic block and interface statistics where accurately available;
- lifecycle events;
- stable error mapping;
- correlation between libvirt call and Core operation ID in logs;
- diagnostic evidence that CellHV delegation mode is active.

A value MUST NOT be fabricated to satisfy a caller.

## 10. Direct-mode preservation

Because the preferred implementation modifies or packages the existing libvirt Cloud Hypervisor driver, qualification MUST include a bounded regression profile for upstream direct mode.

The regression profile verifies at least:

- `ch:///system` still opens in explicitly configured direct mode;
- one upstream-supported direct-mode VM lifecycle remains functional;
- direct mode does not contact CellHV Core unless explicitly configured;
- delegation mode does not open Cloud Hypervisor sockets directly;
- mode-specific runtime directories and ownership markers do not overlap;
- a host cannot silently switch modes while domains exist.

CellHV's compatibility claim does not extend to all upstream direct-mode functions, but CellHV changes MUST NOT knowingly break the inventoried upstream baseline.

## 11. Unsupported in v1

- live migration;
- snapshots and checkpoints;
- managed save;
- QEMU monitor commands;
- QEMU guest-agent commands;
- memory ballooning unless qualified;
- CPU hotplug;
- memory hotplug;
- host device and mediated-device passthrough;
- graphics protocols other than the qualified console;
- secret APIs implemented by the hypervisor driver;
- network-filter semantics;
- arbitrary domain XML;
- Xen or VMware compatibility semantics;
- selecting backend mode from domain XML or untrusted client input;
- a `cellhv:///system` URI in the preferred v1 profile.

## 12. Error behavior

- Unsupported APIs return a standard explicit libvirt unsupported error.
- Invalid XML returns configuration-invalid before host mutation.
- Missing resources return not-found.
- Core operation conflicts map to operation-invalid or resource-busy.
- Transient Core unavailability maps to a retryable system/operation error without losing domain identity.
- Ambiguous process ownership blocks destructive calls.
- Direct/delegated mode conflict fails closed with an operator-visible diagnostic.
- Existing VMs continue running when the libvirt driver or Core API is temporarily unavailable.

## 13. Versioning

Profile revisions are additive within v1 where possible. Removing a supported function requires:

- deprecation notice;
- release-note entry;
- affected platform analysis;
- replacement or migration path;
- compatibility test update.

A compatibility release publishes the tested tuple of:

- CellHV Core version;
- native API version;
- Cloud Hypervisor version;
- libvirt version;
- libvirt `ch` driver or downstream package version;
- delegation-mode configuration version;
- supported platform versions.

## 14. Qualification consumers

The initial conformance consumers are:

1. `virsh`;
2. Python libvirt bindings;
3. Go libvirt client/bindings selected by the project;
4. OpenStack Nova LibvirtDriver;
5. CloudStack KVM agent;
6. OpenNebula KVM/VMM driver.

Passing `virsh` alone does not prove platform compatibility. Preserving the existing `ch:///system` URI reduces compatibility friction but does not prove that a platform's QEMU-specific assumptions have been removed.
