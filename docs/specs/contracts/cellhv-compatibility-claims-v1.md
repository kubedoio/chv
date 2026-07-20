# CellHV Compatibility Claims Contract v1

**Status:** Proposed  
**Authority:** ADR-017  
**Purpose:** prevent ambiguous or inflated compatibility claims

## 1. Rule

CellHV MUST NOT publish an unqualified statement such as:

> compatible with OpenStack

A compatibility claim MUST identify:

- `chv-agent`/CellHV Core version;
- VMM backend and version;
- hypervisor management interface;
- network path;
- storage path;
- platform and platform version;
- supported workload profile;
- passed acceptance profile;
- unsupported features and known deviations;
- evidence digest and maintenance owner.

## 2. Core 1.0 claim tuple

```yaml
cellhv_core:
  runtime_service: chv-agent
  version:
vmm:
  backend: cloud-hypervisor
  version:
hypervisor_interface:
  type: native-api|libvirt-ch|platform-adapter
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
maintenance_owner:
unsupported: []
known_deviations: []
```

The v1 schema intentionally accepts only Cloud Hypervisor as the VMM backend. Any other backend requires a new contract version and ADR; it must not be pre-declared in the active schema.

## 3. VMM identity rules

- `cloud-hypervisor` is the only CellHV Core 1.0 VMM backend.
- `libvirt-ch` may be claimed only when the bounded Cloud Hypervisor libvirt profile passes.
- Cloud Hypervisor MUST NOT be exposed as `qemu:///system`, QEMU, or QMP-compatible.
- A URI connection test is not a platform compatibility test.
- Reported capabilities and statistics MUST match executable behavior.
- `chv-agent` remains the runtime authority for CellHV-managed VMs.

## 4. Network claims

Network support is qualified independently from the VMM.

A claim identifies whether the VM NIC is provided by:

- a pre-existing bridge or TAP;
- a qualified CellHV/`chv-nwd` provider;
- standard libvirt networking;
- an external SDN or platform integration.

Testing MUST cover ownership, creation or consumption, attachment, detach, restart recovery, cleanup, unrelated-state preservation, and leak behavior.

## 5. Storage claims

Storage support is qualified independently from the VMM.

A claim identifies whether the VM disk is provided by:

- a pre-existing file or block path;
- a qualified CellHV/`chv-stord` provider;
- standard libvirt storage;
- an external platform/storage adapter.

Testing MUST cover ownership, attachment, detach, exclusivity where applicable, restart recovery, cleanup, and data integrity.

## 6. Platform claims

Allowed integration labels:

- `generic-libvirt` — works through documented generic libvirt configuration without CellHV-specific platform code;
- `generic-upstream-change` — requires a non-CellHV-specific improvement accepted or explicitly maintained in the relevant project;
- `native-adapter` — uses an official CellHV platform adapter maintained and qualified by CellHV;
- `unsupported`.

A platform adapter is not inferior to a generic path. Selection is based on reliability, maintenance cost, security, upstream viability, and preservation of Core authority.

OpenStack is the only external platform in the active Core 1.0 claim programme. CloudStack, OpenNebula, Kubernetes, Terraform, and Designer require separate future profiles before any support claim.

## 7. Claim levels

### Experimental

- discovery or partial conformance;
- not recommended for production;
- known gaps published.

### Preview

- core lifecycle and required provider paths pass;
- exact version tuple is published;
- upgrade or extended failure testing may remain incomplete;
- maintenance owner is named.

### Supported

- all required platform, network, storage, recovery, upgrade, security, and soak scenarios pass;
- packages and version matrix are published;
- maintenance ownership and evidence digest are published.

## 8. Forbidden claims

- “drop-in QEMU replacement” while using Cloud Hypervisor;
- “OpenStack compatible” based only on Nova connecting to libvirt;
- “CloudStack compatible” based only on registration or lifecycle smoke tests;
- “libvirt compatible” without naming the bounded profile;
- “network supported” without naming the network path;
- “storage supported” without naming the storage path;
- “no adapter required” before real platform qualification;
- any support claim that omits exact versions, unsupported features, evidence, or maintenance ownership.
