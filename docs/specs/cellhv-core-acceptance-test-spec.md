# CellHV Core Acceptance Test Specification

**Status:** Proposed  
**Date:** 2026-07-21  
**Authority:** ADR-015 and ADR-016

## 1. Purpose

This specification prevents implementation progress from being confused with runtime safety or ecosystem compatibility.

`chv-agent` is the CellHV Core implementation. Tests must prove an in-place migration to local authority, not the coexistence of two runtime daemons.

Mocks and schemas can prove contracts. Only real KVM, provider, and platform tests can prove infrastructure claims.

## 2. Test tiers

| Tier | Environment | Proves |
|---|---|---|
| T0 | static CI | boundaries, schemas, forbidden identities, claim consistency |
| T1 | unit/property | state machines, idempotency, mappings, validation |
| T2 | privileged disposable Linux | services, SQLite, permissions, sockets, provider contracts |
| T3 | real KVM host | VM lifecycle, recovery, VMM behavior, leaks |
| T4 | provider/multi-host lab | advertised storage/network behavior |
| T5 | real external platform | OpenStack and later cloud integrations |
| T6 | release lab | upgrade, rollback, security, soak, packaging |

Lower tiers cannot qualify higher-tier claims.

## 3. Active profiles

- `agent-core-migration`;
- `native-api`;
- `minimal-core`;
- `recovery`;
- `vmm-cloud-hypervisor`;
- `libvirt-ch-experimental`;
- `network-provider`;
- `storage-provider`;
- `openstack-discovery`;
- `openstack`;
- `managed-endpoint`;
- `core-1.0`.

CloudStack and OpenNebula profiles remain defined as follow-on work but do not gate the active Core 1.0 programme.

## 4. `chv-agent` migration scenarios

| ID | Requirement | Tier |
|---|---|---|
| AGENT-CORE-001 | No parallel `cellhvd` binary, service, store, or VM authority exists. | T0/T2 |
| AGENT-CORE-002 | Legacy control-plane gRPC and native local API mutations enter the same durable operation engine. | T1/T2 |
| AGENT-CORE-003 | Existing `chv-agent` VM/runtime identifiers map deterministically into durable Core identity. | T1/T2 |
| AGENT-CORE-004 | During migration, one VM cannot be controlled through independent old and new paths. | T2/T3 |
| AGENT-CORE-005 | Controller removal does not stop running workloads or erase local identity. | T3 |
| AGENT-CORE-006 | A future binary rename cannot activate two runtime services simultaneously. | T0/T2 |

## 5. Core north-star scenarios

| ID | Requirement | Tier |
|---|---|---|
| CORE-INSTALL-001 | `chv-agent` Core mode installs and becomes healthy without manager, libvirt, or external DB. | T2 |
| CORE-VM-001 | One qualified Linux VM runs through the native API. | T3 |
| CORE-ATTACH-001 | One pre-existing disk and network endpoint attach correctly. | T3 |
| CORE-IDEMP-001 | Repeated requests do not duplicate resources. | T3 |
| CORE-RECOVERY-001 | Killing `chv-agent` does not stop the VM; the restarted agent re-adopts it. | T3 |
| CORE-RECOVERY-002 | Host reboot preserves identity and requested-state policy. | T3 |
| CORE-STORE-001 | Database corruption fails closed without empty replacement. | T2 |
| CORE-OPS-001 | Crash after commit does not duplicate a VM. | T3 |
| CORE-LEAK-001 | 100 lifecycle cycles leave no resource leaks. | T3 |
| CORE-AUTH-001 | Conflicting ownership blocks destructive mutation. | T3 |

## 6. VMM identity and capability scenarios

| ID | Requirement | Tier |
|---|---|---|
| VMM-ID-001 | Cloud Hypervisor backend never reports or exposes `qemu:///system`. | T0/T2 |
| VMM-ID-002 | Reported VMM and capabilities match the running process. | T2/T3 |
| VMM-ID-003 | Unsupported QEMU/QMP operations fail explicitly. | T1/T3 |
| VMM-ID-004 | VMM process, socket, systemd unit, and agent ownership are auditable. | T2/T3 |

Other VMM backends are outside the active test programme.

## 7. Compatibility-claim scenarios

| ID | Requirement | Tier |
|---|---|---|
| CLAIM-001 | Every published claim validates against the compatibility-claims schema. | T0 |
| CLAIM-002 | Platform, VMM, network, and storage results are recorded independently. | T0/T5 |
| CLAIM-003 | A successful URI connection cannot mark a platform supported. | T0 |
| CLAIM-004 | Unsupported features and known deviations are included in release artifacts. | T0/T6 |
| CLAIM-005 | Evidence digest resolves to real scenario results. | T0/T6 |

## 8. Optional `ch:///system` profile

These scenarios apply only when CellHV advertises the profile.

| ID | Requirement | Tier |
|---|---|---|
| CH-001 | `virConnectOpen("ch:///system")` succeeds. | T2 |
| CH-002 | Capabilities contain no unsupported QEMU claims. | T2 |
| CH-003 | Supported lifecycle enters the `chv-agent` Core operation journal. | T3 |
| CH-004 | UUID and name survive agent/libvirt/host restart. | T3 |
| CH-005 | Supported disk and NIC operations use qualified attachment paths. | T3 |
| CH-006 | Events and statistics are accurate or explicitly unavailable. | T3 |
| CH-007 | Unsupported XML fails before mutation. | T1/T3 |
| CH-008 | Compatibility code cannot access VMM sockets or host mutation APIs directly. | T0/T2 |
| CH-009 | Passing this profile does not set any platform-supported result automatically. | T0 |

## 9. Network provider contract

Every advertised network path tests:

- validate and ownership;
- prepare or consume;
- attach and guest connectivity;
- inspect;
- agent and host restart recovery;
- detach and repeated cleanup;
- unrelated-rule preservation;
- leak detection.

Required negative tests include deleting in-use networks, duplicate MAC/TAP ownership, and modification of unrelated nftables or bridge state.

## 10. Storage provider contract

Every advertised storage path tests:

- validate and ownership;
- provision or consume;
- attach and guest data write/read;
- exclusivity and locking where applicable;
- agent and host restart recovery;
- detach and repeated cleanup;
- data integrity;
- leak detection.

A VMM-profile pass cannot substitute for a storage-profile pass.

## 11. OpenStack discovery gate

The discovery spike is time-boxed and compares:

- Nova LibvirtDriver with upstream `ch:///system`;
- small generic upstream improvements;
- an official CellHV Nova driver using the native API.

Discovery scenarios:

| ID | Requirement | Tier |
|---|---|---|
| OSD-001 | DevStack/Nova reaches the selected libvirt connection and records the first exact failure. | T5 |
| OSD-002 | QEMU-specific Nova/libvirt assumptions are catalogued with code/config references. | T5 |
| OSD-003 | Neutron and Cinder expectations are catalogued separately from VM lifecycle. | T5 |
| OSD-004 | Native Nova driver effort, security boundary, and maintenance cost are estimated. | T0/T5 |
| OSD-005 | A recommendation selects Path A, B, or C with evidence and residual risk. | T0/T5 |

Passing discovery does not claim OpenStack support.

## 12. OpenStack qualification

Once a path is selected, support requires:

| ID | Requirement | Tier |
|---|---|---|
| OS-001 | Accurate host inventory and Placement reporting. | T5 |
| OS-002 | Spawn, inspect, power, reboot, and destroy one instance. | T5 |
| OS-003 | Neutron NIC/MAC mapping passes its network profile. | T5 |
| OS-004 | Cinder block attachment passes its storage profile. | T5 |
| OS-005 | Retry and nova-compute restart do not duplicate or stop VMs. | T5 |
| OS-006 | Selected integration path, versions, unsupported features, and maintenance owner are published. | T0/T5 |
| OS-007 | Rejected paths and QEMU-specific assumptions are retained as evidence. | T5 |
| OS-008 | No path bypasses `chv-agent` Core authority. | T0/T5 |

The `ch` path is not required to win.

## 13. Deferred platform profiles

CloudStack and OpenNebula remain strategic targets. Their discovery and qualification scenarios are retained in the compatibility plan, but implementation begins only after the Core authority and first OpenStack path are stable.

No Core 1.0 claim may imply CloudStack or OpenNebula support without separate T5 evidence.

## 14. Fault strategy

Use:

1. property tests for operation state and idempotency;
2. reusable provider contracts;
3. representative real-host failpoints;
4. platform retry/restart tests;
5. identity and capability-negative tests.

Do not multiply every theoretical fault across every integration unless risk or defects justify it.

## 15. Forbidden outcomes

- a parallel `cellhvd` runtime;
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

## 16. Core 1.0 gate

Core 1.0 requires:

- `agent-core-migration`, minimal Core, and recovery profiles;
- Cloud Hypervisor VMM profile;
- one OpenStack integration at least at the published Preview level;
- all advertised network and storage profiles;
- truthful compatibility claim tuples;
- upgrade, rollback, package, security, and soak qualification.

The `ch:///system` profile is required only when advertised. CloudStack, OpenNebula, and other VMMs are not Core 1.0 gates.
