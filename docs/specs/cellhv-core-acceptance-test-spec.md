# CellHV Core Acceptance Test Specification

**Status:** Proposed  
**Date:** 2026-07-20  
**Depends on:**

- `docs/specs/cellhv-core-foundation-spec.md`
- `docs/specs/adr/015-libvirt-first-ecosystem-compatibility.md`
- `docs/specs/contracts/cellhv-libvirt-compatibility-profile-v1.md`
- `docs/specs/cellhv-core-api-cloud-integration-spec.md`

## 1. Purpose

This specification defines the evidence required to claim that CellHV is a trustworthy standalone runtime, a real libvirt-compatible backend, or compatible with an existing cloud platform.

No API, platform, or provider claim is valid from schema, mock, or unit tests alone.

## 2. Test tiers

| Tier | Environment | May prove |
|---|---|---|
| T0 | static CI | dependency boundaries, schemas, forbidden imports, support-matrix consistency |
| T1 | unit/property | state machines, mappings, idempotency, XML validation, errors |
| T2 | privileged disposable Linux | sockets, SQLite, permissions, daemon restart, libvirt daemon integration |
| T3 | qualified real KVM host | real VM lifecycle, recovery, attachments, events, statistics, leaks |
| T4 | multi-host/provider lab | shared providers and migration when advertised |
| T5 | real external platform | OpenStack, CloudStack, OpenNebula, O3K, Controller compatibility |
| T6 | release qualification | upgrade, rollback, soak, package, security, support matrix |

Lower-tier evidence cannot qualify higher-tier claims.

## 3. Profiles and gates

Profiles:

- `native-api`;
- `minimal-core`;
- `recovery`;
- `libvirt-v1`;
- `virsh`;
- `openstack-libvirt`;
- `cloudstack-libvirt`;
- `opennebula-libvirt`;
- `provider`;
- `managed-endpoint`;
- `core-1.0`.

Gates:

- `pull-request`;
- `milestone-m0`;
- `milestone-m1`;
- `milestone-m2`;
- `milestone-m3`;
- `milestone-m4`;
- `beta`;
- `provider`;
- `release-candidate`;
- `core-1.0`.

A scenario definition includes stable ID, claim, tier, gate, profiles, preconditions, actions, faults, observations, forbidden outcomes, evidence profile, cleanup assertions, and timeout.

## 4. Minimal Core north-star scenarios

| ID | Requirement | Tier | Gate |
|---|---|---|---|
| CORE-INSTALL-001 | Install and become healthy without Controller, libvirt, cloud platform, or external DB. | T2 | `milestone-m1` |
| CORE-VM-001 | Create and start one qualified Linux VM through native API. | T3 | `milestone-m1` |
| CORE-ATTACH-001 | Use one pre-existing network and storage endpoint. | T3 | `milestone-m1` |
| CORE-IDEMP-001 | Repeated lifecycle requests do not duplicate resources. | T3 | `milestone-m1` |
| CORE-RECOVERY-001 | Killing `cellhvd` leaves VM running and it is re-adopted. | T3 | `milestone-m2` |
| CORE-RECOVERY-002 | Host reboot preserves identity and requested state. | T3 | `milestone-m2` |
| CORE-STORE-001 | Corrupt DB fails closed and never creates an empty replacement. | T2 | `milestone-m2` |
| CORE-OPS-001 | Crash after commit does not create a duplicate VM. | T3 | `milestone-m2` |
| CORE-LEAK-001 | 100 lifecycle cycles leave no units, sockets, TAPs, files, or records. | T3 | `milestone-m2` |
| CORE-AUTH-001 | Conflicting ownership is detected and destructive mutation is blocked. | T3 | `milestone-m2` |

## 5. Native API scenarios

| ID | Requirement | Tier |
|---|---|---|
| API-001 | OpenAPI is valid and generated artifacts are reproducible. | T0 |
| API-002 | Breaking `/v1` changes are rejected. | T0 |
| API-003 | Mutations create durable operations and honor idempotency. | T1/T2 |
| API-004 | Resource-version conflicts are rejected before host mutation. | T1/T2 |
| API-005 | Capabilities describe only executable behavior. | T2/T3 |
| API-006 | Core schema contains no cloud-platform model. | T0 |

## 6. Libvirt profile scenarios

| ID | Requirement | Tier | Gate |
|---|---|---|---|
| LIBVIRT-001 | `virConnectOpen("cellhv:///system")` succeeds and reports CellHV type/version. | T2 | `milestone-m3` |
| LIBVIRT-002 | Capability and domain-capability XML match the published profile. | T2 | `milestone-m3` |
| LIBVIRT-003 | Define, list, lookup, start, shutdown, destroy, reboot, pause, resume, undefine. | T3 | `milestone-m3` |
| LIBVIRT-004 | Persistent UUID/name survive libvirt-driver, Core, and host restart. | T3 | `milestone-m3` |
| LIBVIRT-005 | Supported raw/block virtio disk attach and detach use Core operations. | T3 | `milestone-m3` |
| LIBVIRT-006 | Supported virtio NIC attach and detach use Core operations. | T3 | `milestone-m3` |
| LIBVIRT-007 | Lifecycle events are emitted once and in valid order. | T3 | `milestone-m3` |
| LIBVIRT-008 | Basic state and statistics are accurate or explicitly unavailable. | T3 | `milestone-m3` |
| LIBVIRT-009 | Unsupported XML and APIs fail explicitly before mutation. | T1/T3 | `milestone-m3` |
| LIBVIRT-010 | Driver restart rebuilds projection from Core without workload change. | T3 | `milestone-m3` |
| LIBVIRT-011 | Every libvirt mutation has a corresponding Core operation/correlation ID. | T2/T3 | `milestone-m3` |
| LIBVIRT-012 | Driver has no Core DB, host-helper, Linux mutation, or CH-socket access. | T0/T2 | `milestone-m3` |
| LIBVIRT-013 | Conflicting native and libvirt writes are serialized or rejected. | T3 | `milestone-m3` |
| LIBVIRT-014 | Direct `ch:///system` and CellHV ownership of the same VM fails closed. | T3 | `milestone-m3` |
| LIBVIRT-015 | `virsh` and selected Python/Go bindings pass the profile without patches. | T3 | `milestone-m3` |

## 7. Existing-platform compatibility experiments

These begin as experiments. A platform is called supported only after all required scenarios pass on a published version matrix.

### 7.1 OpenStack

The first run uses the upstream Nova LibvirtDriver and no CellHV-specific ComputeDriver.

| ID | Requirement | Tier |
|---|---|---|
| OS-LIBVIRT-001 | Nova connects through the configured libvirt URI. | T5 |
| OS-LIBVIRT-002 | Host inventory and Placement reporting are accurate. | T5 |
| OS-LIBVIRT-003 | Spawn, inspect, power, reboot, and destroy one qualified instance. | T5 |
| OS-LIBVIRT-004 | Assigned NIC/MAC and block attachment map correctly. | T5 |
| OS-LIBVIRT-005 | Retry or nova-compute restart does not duplicate or stop the VM. | T5 |
| OS-LIBVIRT-006 | No CellHV-specific Nova driver/package is installed. | T0/T5 |
| OS-LIBVIRT-007 | Every failure is classified as configuration, missing libvirt profile, generic Nova assumption, or irreducible platform gap. | T5 |

### 7.2 CloudStack

The first run uses the standard KVM agent with no CellHV extension.

| ID | Requirement | Tier |
|---|---|---|
| CS-LIBVIRT-001 | KVM agent connects to the CellHV libvirt URI or records the exact blocker. | T5 |
| CS-LIBVIRT-002 | Deploy, inspect, power, reboot, and delete one qualified instance. | T5 |
| CS-LIBVIRT-003 | Network, MAC/VLAN, and storage mappings are correct. | T5 |
| CS-LIBVIRT-004 | Agent/management restart does not stop or duplicate the VM. | T5 |
| CS-LIBVIRT-005 | QEMU-hook and QEMU-specific assumptions are recorded in a gap matrix. | T5 |
| CS-LIBVIRT-006 | No CellHV-specific CloudStack extension/plugin is installed. | T0/T5 |

### 7.3 OpenNebula

| ID | Requirement | Tier |
|---|---|---|
| ONE-LIBVIRT-001 | Existing KVM/VMM path connects through configured `LIBVIRT_URI`. | T5 |
| ONE-LIBVIRT-002 | Deploy, monitor, power, and delete one qualified instance. | T5 |
| ONE-LIBVIRT-003 | QEMU-specific template/driver gaps are captured. | T5 |
| ONE-LIBVIRT-004 | No CellHV-specific VMM driver is installed. | T0/T5 |

## 8. Gap and fallback acceptance

Every failed platform scenario produces a gap record containing:

- exact upstream version and configuration;
- API call or XML causing failure;
- expected and observed behavior;
- whether the gap belongs to CellHV profile, libvirt, or platform;
- proposed generic upstream fix;
- security and maintenance impact;
- fallback-adapter estimate.

A platform-specific adapter may not start until the fallback decision gate in the API specification passes.

## 9. Fault strategy

Use:

1. property/model tests for generic operation and mapping logic;
2. reusable driver-contract tests for every supported libvirt API;
3. representative real-host faults around create, start, stop, attach, detach, and delete;
4. platform service restart, duplicate request, API timeout, and projection-rebuild tests.

Do not multiply every fault point across every consumer unless risk or a defect justifies it.

## 10. Forbidden outcomes

- VM identity loss;
- empty replacement database after corruption;
- libvirt bypass of Core operation journal;
- direct platform access to CH sockets or host helper;
- two authorities managing one VM;
- fabricated capability or statistic;
- silent XML/device omission;
- duplicate VM after retry;
- management-plane outage stopping a VM;
- leaked units, processes, TAPs, namespaces, mappings, files, or records.

## 11. Evidence

### Minimal

- result and environment versions;
- Core/libvirt/platform package versions;
- operations/events;
- leak result;
- relevant capability documents.

### Failure

- minimal evidence;
- Core/libvirt/platform logs;
- requested and accepted XML/API payloads with secrets redacted;
- process/network/storage inventories;
- correlation timeline;
- gap record.

### Qualification

- complete support matrix;
- package digests;
- test results and logs;
- upgrade/rollback result;
- signed evidence manifest;
- known unsupported functions.

## 12. Core 1.0 gate

Core 1.0 requires:

- minimal Core and recovery profiles;
- libvirt profile v1;
- `virsh` and language-binding profile;
- one existing cloud platform passing without a CellHV-specific adapter;
- measured OpenStack and CloudStack gap reports or passing profiles;
- advertised providers;
- upgrade, rollback, security, package, and soak qualification.
