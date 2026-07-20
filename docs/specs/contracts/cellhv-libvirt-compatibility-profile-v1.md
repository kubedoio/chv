# CellHV Libvirt Compatibility Profile v1

**Status:** Proposed  
**Date:** 2026-07-20  
**Authority:** ADR-015  
**Purpose:** define the minimum real libvirt contract CellHV must satisfy before using libvirt as an ecosystem compatibility claim

## 1. Scope

This profile is intentionally smaller than the complete libvirt API support matrix. A function is supported only when listed here and covered by acceptance tests.

Target connection:

```text
cellhv:///system
```

Remote transports may be added later through standard libvirt remote mechanisms. They are not required for v1.

## 2. Authority contract

- The libvirt driver is stateless except for libvirt-required runtime bookkeeping.
- CellHV Core owns persistent VM identity and configuration.
- Every mutation creates or reuses a Core operation.
- Domain UUID equals Core VM UUID.
- Driver restart rebuilds its projection from Core.
- The driver does not access the Core database.
- The driver does not open Cloud Hypervisor sockets.
- The driver does not mutate networking or storage except through Core attachment requests.
- Conflicting native and libvirt mutations fail explicitly.

## 3. Connection and capability APIs

Required:

- `virConnectOpen` / `virConnectOpenAuth` as applicable;
- `virConnectClose`;
- `virConnectGetType`;
- `virConnectGetVersion`;
- `virConnectGetLibVersion`;
- `virConnectGetCapabilities`;
- `virConnectGetDomainCapabilities`;
- `virConnectIsAlive`;
- `virConnectListAllDomains`;
- `virConnectGetMaxVcpus`;
- basic node information and free-memory reporting where accurate.

Capabilities MUST advertise only executable behavior.

## 4. Domain identity and lookup

Required:

- lookup by UUID;
- lookup by name;
- lookup by ID for running domains;
- persistent inactive domain listing;
- stable UUID and name across driver and host restart.

Duplicate UUID or conflicting names return standard libvirt errors.

## 5. Supported domain XML subset

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

The driver MUST reject unsupported XML before host mutation. It MUST NOT silently ignore unsupported devices.

The first profile does not require IDE, SATA, SCSI emulation, USB, sound, graphics stacks, legacy BIOS devices, arbitrary PCI topology, QEMU command-line injection, or arbitrary emulator paths.

## 6. Domain lifecycle APIs

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

## 7. Device operations

Required after the corresponding Core provider is qualified:

- attach/detach supported virtio file or block disk;
- attach/detach supported virtio NIC;
- live versus config flags must be handled explicitly;
- unsupported flag combinations return `VIR_ERR_NO_SUPPORT` or the narrowest valid error.

No storage-pool or virtual-network implementation is required in the CellHV hypervisor driver. Standard libvirt storage/network drivers may coexist when their output is passed to Core as a pre-existing attachment.

## 8. Statistics and observability

Required:

- domain state;
- vCPU count;
- configured and observed memory;
- process/CPU time where accurately available;
- basic block and interface statistics where accurately available;
- lifecycle events;
- stable error mapping;
- correlation between libvirt call and Core operation ID in logs.

A value MUST NOT be fabricated to satisfy a caller.

## 9. Unsupported in v1

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
- Xen or VMware compatibility semantics.

## 10. Error behavior

- Unsupported APIs return a standard explicit libvirt unsupported error.
- Invalid XML returns configuration-invalid before mutation.
- Missing resources return not-found.
- Core operation conflicts map to operation-invalid or resource-busy.
- Transient Core unavailability maps to a retryable system/operation error without losing domain identity.
- Ambiguous process ownership blocks destructive calls.

## 11. Versioning

Profile revisions are additive within v1 where possible. Removing a supported function requires:

- deprecation notice;
- release-note entry;
- affected platform analysis;
- replacement or migration path;
- compatibility test update.

## 12. Qualification consumers

The initial conformance consumers are:

1. `virsh`;
2. Python libvirt bindings;
3. Go libvirt client/bindings selected by the project;
4. OpenStack Nova LibvirtDriver;
5. CloudStack KVM agent;
6. OpenNebula KVM/VMM driver.

Passing `virsh` alone does not prove platform compatibility.
