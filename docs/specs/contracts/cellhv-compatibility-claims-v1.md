# CellHV Compatibility Claims Contract v1

**Status:** Proposed  
**Authority:** ADR-015  
**Purpose:** prevent ambiguous or inflated compatibility claims

## 1. Rule

CellHV MUST NOT publish an unqualified statement such as:

> compatible with OpenStack

A compatibility claim MUST identify:

- CellHV Core version;
- VMM backend and version;
- hypervisor management interface;
- network path;
- storage path;
- platform and platform version;
- supported workload profile;
- passed acceptance profile;
- unsupported features and known deviations.

## 2. Claim tuple

```yaml
cellhv_core:
vmm:
  backend: cloud-hypervisor|qemu|other
  version:
hypervisor_interface:
  type: native-api|libvirt-ch|libvirt-qemu|platform-adapter
  version:
network:
  type: existing-bridge|cellhv-provider|libvirt-network|external-sdn
  profile:
storage:
  type: existing-file-block|cellhv-provider|libvirt-storage|external-storage
  profile:
platform:
  name:
  version:
  integration: generic-libvirt|native-adapter|generic-upstream-change|none
workload:
  guest_os:
  architecture:
  firmware:
qualification:
  profile:
  evidence_digest:
unsupported: []
known_deviations: []
```

## 3. VMM identity rules

- `cloud-hypervisor` is the default CellHV Core 1.0 VMM backend.
- `libvirt-ch` may be claimed only when the published Cloud Hypervisor libvirt profile passes.
- `libvirt-qemu` may be claimed only when CellHV is using an actual qualified QEMU backend.
- Cloud Hypervisor MUST NOT be exposed as `qemu:///system`.
- A URI connection test is not a platform compatibility test.
- Reported capabilities MUST match executable behavior.

## 4. Network claims

Network support is qualified independently from the VMM.

A claim identifies whether the VM NIC is provided by:

- a pre-existing bridge or TAP;
- a CellHV Linux provider;
- standard libvirt networking;
- an external SDN or platform integration.

Testing MUST cover creation or consumption, attachment, detach, restart recovery, cleanup, and leak behavior.

## 5. Storage claims

Storage support is qualified independently from the VMM.

A claim identifies whether the VM disk is provided by:

- a pre-existing file or block path;
- a CellHV storage provider;
- standard libvirt storage;
- an external platform/storage adapter.

Testing MUST cover attachment, detach, exclusivity where applicable, restart recovery, cleanup, and data integrity.

## 6. Platform claims

Allowed platform integration labels:

- `generic-libvirt` — works through a documented generic libvirt configuration without CellHV-specific platform code;
- `generic-upstream-change` — requires a non-CellHV-specific platform or libvirt improvement accepted by the relevant project;
- `native-adapter` — uses an official CellHV platform adapter maintained and qualified by CellHV;
- `future-qemu-backend` — uses an actual qualified QEMU backend, never QEMU emulation around Cloud Hypervisor;
- `unsupported`.

A platform adapter is not considered inferior to a generic path. The selected path is judged by reliability, maintenance cost, security, and upstream viability.

## 7. Claim levels

### Experimental

- discovery or partial conformance;
- not recommended for production;
- known gaps published.

### Preview

- core lifecycle and required provider paths pass;
- upgrade and failure testing may remain incomplete;
- limited support matrix.

### Supported

- all required platform, network, storage, recovery, upgrade, and security scenarios pass;
- packages and version matrix are published;
- maintenance ownership is named.

## 8. Forbidden claims

- “drop-in QEMU replacement” while using Cloud Hypervisor;
- “OpenStack compatible” based only on Nova connecting to libvirt;
- “CloudStack compatible” based only on a KVM-agent registration;
- “libvirt compatible” without naming the bounded profile;
- “network supported” without naming the network path;
- “storage supported” without naming the storage path;
- “no adapter required” before real platform qualification.
