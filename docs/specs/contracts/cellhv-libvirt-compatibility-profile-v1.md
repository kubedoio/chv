# CellHV Cloud Hypervisor Libvirt Compatibility Profile v1

**Status:** Proposed  
**Authority:** ADR-015  
**Purpose:** define an optional bounded libvirt profile for the Cloud Hypervisor backend

## 1. Scope

This profile evaluates and, if implemented, qualifies CellHV through the upstream Cloud Hypervisor libvirt identity:

```text
ch:///system
```

This profile is optional for Core 1.0 unless CellHV advertises it. It is not a universal cloud-platform compatibility contract.

It does not authorize:

- `qemu:///system`;
- QEMU capability claims;
- complete libvirt support;
- automatic OpenStack, CloudStack, or OpenNebula compatibility.

## 2. Authority

When CellHV delegation is enabled:

- CellHV Core owns persistent VM identity and accepted configuration.
- Every mutation creates or reuses a Core operation.
- Domain UUID equals Core VM UUID.
- The compatibility layer does not access the Core database.
- It does not open Cloud Hypervisor sockets directly.
- It does not launch, stop, or signal Cloud Hypervisor directly.
- It does not mutate networking or storage except through published Core attachment APIs.
- Driver restart rebuilds its projection from Core.
- Core unavailability blocks new mutations but does not stop existing VMs.

Direct upstream `ch` mode and CellHV delegation mode MUST NOT manage the same resource namespace.

## 3. Required connection APIs

- `virConnectOpen` / `virConnectOpenAuth`;
- `virConnectClose`;
- `virConnectGetType`;
- `virConnectGetVersion`;
- `virConnectGetLibVersion`;
- `virConnectGetCapabilities`;
- `virConnectGetDomainCapabilities`;
- `virConnectIsAlive`;
- `virConnectListAllDomains`;
- basic node information where accurate.

The upstream Cloud Hypervisor identity is preserved. CellHV branding MUST NOT replace the reported hypervisor type.

## 4. Required VM model

- stable name and UUID;
- memory and vCPU;
- modern HVM boot;
- supported firmware or kernel boot;
- raw file or block virtio disk;
- bridge or pre-created TAP virtio NIC;
- serial console;
- explicit rejection of unsupported XML.

The profile excludes legacy device emulation, arbitrary PCI topology, QEMU command-line injection, QMP, and arbitrary emulator paths.

## 5. Required lifecycle

- define and undefine;
- start;
- graceful shutdown;
- force stop;
- reboot;
- pause and resume;
- state inspection;
- lifecycle events where observable.

Every mutating call is correlatable with a Core operation and stable idempotency identity.

## 6. Device operations

After the relevant provider profile passes:

- attach/detach supported virtio file or block disk;
- attach/detach supported virtio NIC;
- explicit live/config flag handling;
- explicit unsupported errors.

Libvirt network and storage drivers may coexist, but their outputs are consumed by Core as attachments. Their qualification is separate from this hypervisor profile.

## 7. Statistics

Only accurate values are returned:

- state;
- vCPU count;
- configured and observed memory;
- process CPU time where available;
- basic block and interface statistics where available.

Unavailable values are reported as unsupported or unavailable, never fabricated.

## 8. Unsupported in v1

- live migration;
- snapshots and checkpoints;
- managed save;
- QMP and QEMU monitor calls;
- QEMU guest-agent calls;
- QEMU block jobs;
- QEMU migration semantics;
- CPU or memory hotplug;
- arbitrary host-device passthrough;
- complete graphics stack;
- arbitrary domain XML;
- platform compatibility claims;
- `qemu:///system`.

## 9. Qualification

The profile passes only when:

- real `virsh` and selected language bindings pass;
- all mutations enter the Core operation journal;
- restart and host-reboot behavior preserve identity;
- unsupported XML fails before mutation;
- no Cloud Hypervisor socket or direct host mutation is performed by the compatibility layer;
- network and storage paths used by the test have their own passing profiles;
- direct upstream `ch` behavior affected by a downstream patch has a bounded regression result.

Passing this profile does not prove OpenStack, CloudStack, or OpenNebula compatibility.
