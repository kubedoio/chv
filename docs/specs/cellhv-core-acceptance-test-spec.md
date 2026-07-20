# CellHV Core Acceptance Test Specification

**Status:** Proposed  
**Date:** 2026-07-20

## 1. Purpose

This specification prevents implementation progress from being confused with product compatibility.

Mocks and schemas can prove contracts. Only real KVM, provider, and platform tests can prove infrastructure claims.

## 2. Test tiers

| Tier | Environment | Proves |
|---|---|---|
| T0 | static CI | boundaries, schemas, forbidden identities, claim consistency |
| T1 | unit/property | state machines, idempotency, mappings, validation |
| T2 | privileged disposable Linux | services, SQLite, permissions, sockets, providers |
| T3 | real KVM host | VM lifecycle, recovery, VMM behavior, leaks |
| T4 | provider/multi-host lab | storage, networking, migration where advertised |
| T5 | real external platform | OpenStack, CloudStack, OpenNebula, O3K |
| T6 | release lab | upgrade, rollback, security, soak, packaging |

Lower tiers cannot qualify higher-tier claims.

## 3. Profiles

- `native-api`;
- `minimal-core`;
- `recovery`;
- `vmm-cloud-hypervisor`;
- `libvirt-ch-experimental`;
- `network-provider`;
- `storage-provider`;
- `openstack`;
- `cloudstack`;
- `opennebula`;
- `managed-endpoint`;
- `core-1.0`.

A future QEMU backend receives its own `vmm-qemu` and `libvirt-qemu` profiles after a separate ADR.

## 4. Core north-star scenarios

| ID | Requirement | Tier |
|---|---|---|
| CORE-INSTALL-001 | Core installs and becomes healthy without manager, libvirt, or external DB. | T2 |
| CORE-VM-001 | One qualified Linux VM runs through the native API. | T3 |
| CORE-ATTACH-001 | One pre-existing disk and network endpoint attach correctly. | T3 |
| CORE-IDEMP-001 | Repeated requests do not duplicate resources. | T3 |
| CORE-RECOVERY-001 | Killing `cellhvd` does not stop the VM; it is re-adopted. | T3 |
| CORE-RECOVERY-002 | Host reboot preserves identity and requested state. | T3 |
| CORE-STORE-001 | Database corruption fails closed without empty replacement. | T2 |
| CORE-OPS-001 | Crash after commit does not duplicate a VM. | T3 |
| CORE-LEAK-001 | 100 lifecycle cycles leave no resource leaks. | T3 |
| CORE-AUTH-001 | Conflicting ownership blocks destructive mutation. | T3 |

## 5. VMM identity and capability scenarios

| ID | Requirement | Tier |
|---|---|---|
| VMM-ID-001 | Cloud Hypervisor backend never reports or exposes `qemu:///system`. | T0/T2 |
| VMM-ID-002 | Reported VMM and capabilities match the running process. | T2/T3 |
| VMM-ID-003 | Unsupported QEMU/QMP operations fail explicitly. | T1/T3 |
| VMM-ID-004 | VMM process, socket, and systemd ownership are auditable. | T2/T3 |
| VMM-ID-005 | A future QEMU profile cannot pass unless actual QEMU processes run. | T0/T3 |

## 6. Compatibility-axis scenarios

| ID | Requirement | Tier |
|---|---|---|
| CLAIM-001 | Every published claim validates against the compatibility-claims schema. | T0 |
| CLAIM-002 | Platform, VMM, network, and storage results are recorded independently. | T0/T5 |
| CLAIM-003 | A successful URI connection cannot mark a platform supported. | T0 |
| CLAIM-004 | Unsupported features and known deviations are included in release artifacts. | T0/T6 |
| CLAIM-005 | Evidence digest resolves to real scenario results. | T0/T6 |

## 7. Optional `ch:///system` profile

These scenarios apply only when CellHV advertises the profile.

| ID | Requirement | Tier |
|---|---|---|
| CH-001 | `virConnectOpen("ch:///system")` succeeds. | T2 |
| CH-002 | Capabilities contain no unsupported QEMU claims. | T2 |
| CH-003 | Supported lifecycle enters the Core operation journal. | T3 |
| CH-004 | UUID and name survive Core/libvirt/host restart. | T3 |
| CH-005 | Supported disk and NIC operations use qualified attachment paths. | T3 |
| CH-006 | Events and statistics are accurate or explicitly unavailable. | T3 |
| CH-007 | Unsupported XML fails before mutation. | T1/T3 |
| CH-008 | Compatibility code cannot access VMM sockets or host mutation APIs directly. | T0/T2 |
| CH-009 | Passing this profile does not set any platform-supported result automatically. | T0 |

## 8. Network provider contract

Every advertised network path tests:

- validate;
- prepare or consume;
- attach;
- guest connectivity;
- inspect;
- daemon and host restart recovery;
- detach;
- repeated cleanup;
- unrelated-rule preservation;
- leak detection.

Required negative tests include deleting in-use networks, duplicate MAC/TAP ownership, and modification of unrelated nftables or bridge state.

## 9. Storage provider contract

Every advertised storage path tests:

- validate;
- provision or consume;
- attach;
- data write/read;
- exclusivity and locking where applicable;
- daemon and host restart recovery;
- detach;
- repeated cleanup;
- data integrity;
- leak detection.

A VMM-profile pass cannot substitute for a storage-profile pass.

## 10. OpenStack qualification

The discovery run compares all viable paths:

- generic `ch` libvirt path;
- generic upstream changes;
- official CellHV Nova driver.

Required scenarios:

| ID | Requirement | Tier |
|---|---|---|
| OS-001 | Accurate host inventory and Placement reporting. | T5 |
| OS-002 | Spawn, inspect, power, reboot, and destroy one instance. | T5 |
| OS-003 | Neutron NIC/MAC mapping passes its network profile. | T5 |
| OS-004 | Cinder block attachment passes its storage profile. | T5 |
| OS-005 | Retry and nova-compute restart do not duplicate or stop VMs. | T5 |
| OS-006 | Selected integration path and maintenance owner are published. | T0/T5 |
| OS-007 | QEMU-specific assumptions are recorded for rejected paths. | T5 |
| OS-008 | No path bypasses Core authority. | T0/T5 |

OpenStack support requires all required scenarios for one selected path. It does not require the `ch` path to win.

## 11. CloudStack qualification

| ID | Requirement | Tier |
|---|---|---|
| CS-001 | Connection, hook, QEMU-tooling, and URI assumptions are inventoried. | T5 |
| CS-002 | Selected path deploys, inspects, powers, reboots, and deletes one instance. | T5 |
| CS-003 | Network and storage paths pass their independent profiles. | T5 |
| CS-004 | Agent/management restart does not duplicate or stop the VM. | T5 |
| CS-005 | Selected integration path and maintenance owner are published. | T0/T5 |
| CS-006 | No Cloud Hypervisor path claims `qemu:///system`. | T0/T5 |
| CS-007 | No path bypasses Core authority. | T0/T5 |

A complete gap report is acceptable before CloudStack support is implemented, but it is not a support claim.

## 12. OpenNebula qualification

The same method compares generic libvirt, generic VMM changes, and a native CellHV VMM driver.

Lifecycle, monitoring, network, datastore, restart, and Core-authority tests are mandatory for a supported claim.

## 13. Fault strategy

Use:

1. property tests for operation state and idempotency;
2. reusable provider contracts;
3. representative real-host failpoints;
4. platform retry/restart tests;
5. identity and capability-negative tests.

Do not multiply every theoretical fault across every integration unless risk or defects justify it.

## 14. Forbidden outcomes

- Cloud Hypervisor exposed as QEMU;
- fabricated capabilities or statistics;
- platform support inferred from URI connection;
- network or storage support inferred from VM lifecycle alone;
- VM identity loss;
- empty replacement database;
- duplicate VM after retry;
- two authorities for one VM;
- management outage stopping workloads;
- silent unsupported XML;
- leaked units, processes, TAPs, namespaces, mappings, volumes, files, or records.

## 15. Core 1.0 gate

Core 1.0 requires:

- minimal Core and recovery profiles;
- Cloud Hypervisor VMM profile;
- at least one supported OpenStack integration path;
- a CloudStack discovery report and selected path;
- all advertised network and storage profiles;
- truthful compatibility claim tuples;
- upgrade, rollback, package, security, and soak qualification.

The `ch:///system` profile is required only when advertised. A QEMU profile is not part of Core 1.0 unless separately approved.
