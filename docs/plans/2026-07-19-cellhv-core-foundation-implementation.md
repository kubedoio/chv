# CellHV Core Foundation Implementation Plan

**Status:** Proposed  
**Date:** 2026-07-20

## 1. Objective

Extract a small autonomous CellHV Core from the current control-plane-led implementation, qualify Cloud Hypervisor honestly, and choose cloud-platform integration paths from measured evidence.

The implementation MUST avoid:

- a flag-day rewrite;
- false QEMU identity;
- making libvirt mandatory for standalone Core;
- embedding cloud-platform models in Core;
- assuming one URI solves networking, storage, and platform integration.

## 2. Repository mapping

| Existing area | Direction |
|---|---|
| `chv-agent-runtime-ch` | source for Cloud Hypervisor VMM adapter |
| `chv-agent-core` | source for lifecycle, console, telemetry, recovery |
| `chv-stord-*` | source for storage providers |
| `chv-nwd-*` | source for network providers |
| `chv-common`, `chv-errors`, `chv-observability` | reuse |
| node JSON cache | migration input only, never authoritative |
| existing gRPC | compatibility/internal path during transition |
| Controller, UI, Designer | remain above Core |

Target structure:

```text
api/openapi/cellhv-core-v1.yaml
cmd/cellhvd/
cmd/cellhv-hostd/
crates/cellhv-core-*/
crates/cellhv-vmm-cloud-hypervisor/
crates/cellhv-vmm-api/
crates/cellhv-network-provider-*/
crates/cellhv-storage-provider-*/
integrations/libvirt-ch/
integrations/openstack/
integrations/cloudstack/
integrations/opennebula/
tests/qualification/
```

## 3. Phase plan

### Phase 0 — decision and claim lock

- approve authority invariants;
- approve ADR-015;
- publish compatibility-claims contract;
- define initial host/VMM/guest matrix;
- prohibit QEMU identity for Cloud Hypervisor;
- create dependency guards.

Exit: product and claim boundaries are machine-checkable.

### Phase 1 — Core M0

- Core domain types;
- SQLite schema and migrations;
- operation journal;
- idempotency and resource versions;
- native OpenAPI skeleton;
- generated client;
- VMM adapter trait.

Exit: state and contract tests pass.

### Phase 2 — Core M1

- minimal `cellhvd`;
- Cloud Hypervisor adapter;
- one Linux VM;
- pre-existing disk and bridge/TAP;
- create, inspect, start, stop, delete;
- compatibility path from current agent.

Exit: real-KVM minimal scenarios pass.

### Phase 3 — Core M2

- daemon re-adoption;
- host reboot;
- fail-closed database;
- crash recovery;
- leak testing;
- ownership markers;
- truthful capability reporting.

Exit: recovery and VMM identity profiles pass.

### Phase 4 — compatibility discovery

Run independent workstreams:

#### Hypervisor interface

- inventory upstream libvirt `ch` support;
- test `virsh` and language bindings;
- identify delegation or coexistence options;
- do not assume support is worth implementing.

#### OpenStack

- run Nova LibvirtDriver against `ch`;
- enumerate QEMU assumptions;
- estimate a native CellHV ComputeDriver;
- compare maintenance and reliability.

#### CloudStack

- inventory URI, hook, QEMU-tool, storage, network, and agent assumptions;
- evaluate extension and native-plugin paths.

#### OpenNebula

- compare generic libvirt, generic VMM changes, and native driver.

#### Network and storage

- map Neutron/Cinder, CloudStack, and OpenNebula outputs into Core attachment contracts separately.

Exit: evidence-based path recommendation for each platform.

### Phase 5 — first OpenStack integration

Implement the selected path:

- generic `ch` path;
- generic upstream changes;
- or official CellHV Nova driver.

Requirements:

- Core authority remains unchanged;
- Neutron and Cinder paths pass independent provider profiles;
- retry, restart, upgrade, and rollback tests;
- maintenance owner and version matrix.

Exit: OpenStack supported-profile tests pass.

### Phase 6 — CloudStack integration decision

Using the discovery report:

- implement the selected generic or native integration;
- or publish unsupported/preview status with exact blockers;
- never masquerade Cloud Hypervisor as QEMU.

Exit: support profile passes or a reviewed roadmap decision exists.

### Phase 7 — OpenNebula integration

Implement only the selected smallest maintainable path and qualify datastore/network behavior separately.

### Phase 8 — standard providers and managed endpoint

- privileged helper;
- bridge/VLAN/NAT providers;
- raw/LVM/RBD providers;
- HTTPS/mTLS and enrollment;
- Controller and O3K native API integration.

### Phase 9 — optional compatibility profiles

- bounded `ch:///system` profile if it has real consumers;
- Terraform and Kubernetes integrations;
- additional cloud platforms.

### Phase 10 — future QEMU backend decision

Only after an explicit business and compatibility case:

- write separate ADR;
- define real QEMU adapter;
- decide libvirt ownership model;
- qualify complete required semantics;
- ensure Cloud Hypervisor and QEMU identities never mix.

This phase is not required for Core 1.0.

## 4. Initial PR sequence

1. ADR-015 and compatibility-claims contract.
2. Core dependency and identity guards.
3. Core types, SQLite, and operation journal.
4. Native API and generated client.
5. VMM adapter contract.
6. Minimal Cloud Hypervisor runtime.
7. Recovery and ownership tests.
8. Network attachment contract.
9. Storage attachment contract.
10. Libvirt `ch` support-matrix discovery.
11. OpenStack path-comparison lab.
12. Selected OpenStack implementation ADR and code.
13. CloudStack path-comparison lab.
14. Selected CloudStack decision.
15. OpenNebula path-comparison lab.
16. Standard provider qualification.

## 5. Coding-agent rules

Every task names:

- phase;
- VMM backend;
- authority boundary;
- network and storage paths;
- platform integration path if applicable;
- acceptance IDs;
- unsupported behavior;
- rollback.

Agents MUST NOT:

- expose Cloud Hypervisor as `qemu:///system`;
- add QMP emulation;
- infer platform support from a connection test;
- infer network/storage support from VM lifecycle;
- write cloud-platform state into Core;
- bypass public Core APIs;
- fabricate capabilities;
- silently accept unsupported XML/devices;
- claim compatibility from mocks;
- implement a future QEMU backend without an ADR.

## 6. Open decisions

- first OpenStack integration path;
- first CloudStack integration path;
- whether bounded `ch` support has enough consumer value;
- standard libvirt network/storage coexistence;
- exact provider process boundaries;
- support distributions and versions;
- future actual QEMU backend business case;
- integration maintenance ownership.
