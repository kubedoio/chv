# CellHV Core Acceptance Test Specification

**Status:** Proposed  
**Date:** 2026-07-20  
**Depends on:**

- `docs/specs/cellhv-core-foundation-spec.md`
- `docs/specs/adr/015-libvirt-first-ecosystem-compatibility.md`
- `docs/specs/contracts/cellhv-libvirt-compatibility-profile-v1.md`
- `docs/specs/cellhv-core-api-cloud-integration-spec.md`

## 1. Purpose

This specification defines the evidence required to claim that CellHV is a trustworthy standalone runtime, a real libvirt-compatible Cloud Hypervisor backend, or compatible with an existing cloud platform.

No API, platform, or provider claim is valid from schema, mock, or unit tests alone.

Preserving the existing `ch:///system` URI reduces connection-level integration work. It does not by itself prove OpenStack, CloudStack, OpenNebula, or another platform compatible.

## 2. Test tiers

| Tier | Environment | May prove |
|---|---|---|
| T0 | static CI | dependency boundaries, schemas, forbidden imports, support-matrix consistency |
| T1 | unit/property | state machines, mappings, idempotency, XML validation, errors, mode selection |
| T2 | privileged disposable Linux | sockets, SQLite, permissions, daemon restart, libvirt daemon integration, backend mode isolation |
| T3 | qualified real KVM host | real VM lifecycle, recovery, attachments, events, statistics, direct/delegated-mode behavior, leaks |
| T4 | multi-host/provider lab | shared providers and migration when advertised |
| T5 | real external platform | OpenStack, CloudStack, OpenNebula, O3K, Controller compatibility |
| T6 | release qualification | upgrade, rollback, soak, package, security, support matrix |

Lower-tier evidence cannot qualify higher-tier claims.

## 3. Profiles and gates

Profiles:

- `native-api`;
- `minimal-core`;
- `recovery`;
- `libvirt-ch-delegation-v1`;
- `libvirt-ch-direct-regression`;
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

## 6. Libvirt `ch:///system` delegation scenarios

| ID | Requirement | Tier | Gate |
|---|---|---|---|
| LIBVIRT-001 | `virConnectOpen("ch:///system")` succeeds with CellHV delegation mode enabled and preserves the upstream driver identity. | T2 | `milestone-m3` |
| LIBVIRT-002 | Capability and domain-capability XML match the published profile and do not claim unsupported QEMU behavior. | T2 | `milestone-m3` |
| LIBVIRT-003 | Define, list, lookup, start, shutdown, destroy, reboot, pause, resume, and undefine work through Core. | T3 | `milestone-m3` |
| LIBVIRT-004 | Persistent UUID/name survive libvirt-driver, Core, and host restart. | T3 | `milestone-m3` |
| LIBVIRT-005 | Supported raw/block virtio disk attach and detach use Core operations. | T3 | `milestone-m3` |
| LIBVIRT-006 | Supported virtio NIC attach and detach use Core operations. | T3 | `milestone-m3` |
| LIBVIRT-007 | Lifecycle events are emitted once and in valid order. | T3 | `milestone-m3` |
| LIBVIRT-008 | Basic state and statistics are accurate or explicitly unavailable. | T3 | `milestone-m3` |
| LIBVIRT-009 | Unsupported XML and APIs fail explicitly before host mutation. | T1/T3 | `milestone-m3` |
| LIBVIRT-010 | Driver restart rebuilds projection from Core without workload change. | T3 | `milestone-m3` |
| LIBVIRT-011 | Every mutating libvirt call has a corresponding Core operation and correlation ID. | T2/T3 | `milestone-m3` |
| LIBVIRT-012 | Delegation mode has no Core DB, host-helper, Linux mutation, or Cloud Hypervisor socket access. | T0/T2 | `milestone-m3` |
| LIBVIRT-013 | Conflicting native and libvirt writes are serialized or rejected. | T3 | `milestone-m3` |
| LIBVIRT-014 | `virsh` and selected Python/Go bindings pass the profile without CellHV-specific client patches. | T3 | `milestone-m3` |
| LIBVIRT-015 | Active delegation mode is observable in trusted diagnostics without changing the public URI. | T2/T3 | `milestone-m3` |

## 7. Driver-mode and upstream-regression scenarios

| ID | Requirement | Tier | Gate |
|---|---|---|---|
| CH-MODE-001 | Direct mode and CellHV delegation mode are selected only through trusted host-local configuration. | T1/T2 | `milestone-m3` |
| CH-MODE-002 | Client URI, domain XML, or untrusted cloud input cannot switch backend mode. | T1/T2 | `milestone-m3` |
| CH-MODE-003 | Direct mode and delegation mode cannot own the same VM UUID, process, socket, unit, or runtime directory. | T3 | `milestone-m3` |
| CH-MODE-004 | Switching mode while domains exist fails closed and provides migration/cleanup guidance. | T2/T3 | `milestone-m3` |
| CH-MODE-005 | One inventoried upstream direct-mode lifecycle still works when direct mode is explicitly configured. | T3 | `milestone-m3` |
| CH-MODE-006 | Direct mode does not contact CellHV Core unless delegation is explicitly configured. | T2/T3 | `milestone-m3` |
| CH-MODE-007 | Delegation mode never opens Cloud Hypervisor API sockets or launches VMs directly. | T2/T3 | `milestone-m3` |
| CH-MODE-008 | Core or libvirt restart does not change the selected mode or stop existing workloads. | T3 | `milestone-m3` |

## 8. Existing-platform compatibility experiments

These begin as experiments. A platform is called supported only after all required scenarios pass on a published version matrix.

The unchanged-path profile permits documented standard configuration changes, including setting a libvirt connection URI. It does not permit a CellHV-specific platform driver, extension, or plugin.

### 8.1 OpenStack

The first run uses the upstream Nova LibvirtDriver, with `connection_uri = ch:///system` or the equivalent supported configuration, and no CellHV-specific ComputeDriver.

| ID | Requirement | Tier |
|---|---|---|
| OS-LIBVIRT-001 | Nova connects through `ch:///system` using documented configuration only. | T5 |
| OS-LIBVIRT-002 | Host inventory and Placement reporting are accurate. | T5 |
| OS-LIBVIRT-003 | Spawn, inspect, power, reboot, and destroy one qualified instance. | T5 |
| OS-LIBVIRT-004 | Assigned NIC/MAC and block attachment map correctly. | T5 |
| OS-LIBVIRT-005 | Retry or nova-compute restart does not duplicate or stop the VM. | T5 |
| OS-LIBVIRT-006 | No CellHV-specific Nova driver/package is installed. | T0/T5 |
| OS-LIBVIRT-007 | Every failure is classified as configuration, missing libvirt profile, generic Nova assumption, QEMU-specific assumption, or irreducible platform gap. | T5 |
| OS-LIBVIRT-008 | Nova cannot bypass delegation mode or reach Cloud Hypervisor/Core-private interfaces. | T0/T5 |

### 8.2 CloudStack

The first run uses the standard KVM agent with no CellHV extension. It attempts `ch:///system` through supported configuration; if this is impossible, the exact blocker is part of the result.

| ID | Requirement | Tier |
|---|---|---|
| CS-LIBVIRT-001 | KVM agent connects through `ch:///system` or records the exact hard-coded/configuration blocker. | T5 |
| CS-LIBVIRT-002 | Deploy, inspect, power, reboot, and delete one qualified instance when connection succeeds. | T5 |
| CS-LIBVIRT-003 | Network, MAC/VLAN, and storage mappings are correct. | T5 |
| CS-LIBVIRT-004 | Agent/management restart does not stop or duplicate the VM. | T5 |
| CS-LIBVIRT-005 | QEMU hooks, tools, URI assumptions, and host preparation are recorded in a gap matrix. | T5 |
| CS-LIBVIRT-006 | No CellHV-specific CloudStack extension/plugin is installed. | T0/T5 |
| CS-LIBVIRT-007 | CloudStack cannot bypass delegation mode or reach Cloud Hypervisor/Core-private interfaces. | T0/T5 |

### 8.3 OpenNebula

The first run uses the existing KVM/VMM path with `LIBVIRT_URI = ch:///system` and no CellHV-specific VMM driver.

| ID | Requirement | Tier |
|---|---|---|
| ONE-LIBVIRT-001 | Existing KVM/VMM path connects through `ch:///system`. | T5 |
| ONE-LIBVIRT-002 | Deploy, monitor, power, and delete one qualified instance. | T5 |
| ONE-LIBVIRT-003 | QEMU-specific template/driver gaps are captured. | T5 |
| ONE-LIBVIRT-004 | No CellHV-specific VMM driver is installed. | T0/T5 |
| ONE-LIBVIRT-005 | OpenNebula cannot bypass delegation mode or reach Cloud Hypervisor/Core-private interfaces. | T0/T5 |

## 9. Gap and fallback acceptance

Every failed platform scenario produces a gap record containing:

- exact upstream version and configuration;
- requested libvirt URI;
- active backend mode;
- API call or XML causing failure;
- expected and observed behavior;
- whether the gap belongs to Core, CellHV delegation, libvirt, platform configuration, or QEMU-specific platform behavior;
- proposed generic upstream fix;
- security and maintenance impact;
- fallback-adapter estimate;
- separate-URI estimate when relevant.

A platform-specific adapter may not start until the fallback decision gate in the API specification passes.

A separate `cellhv:///system` driver may not start until the separate-URI fallback gate passes and a new ADR is approved.

## 10. Fault strategy

Use:

1. property/model tests for generic operation, idempotency, mode-selection, and mapping logic;
2. reusable driver-contract tests for every supported libvirt API;
3. representative real-host faults around create, start, stop, attach, detach, and delete;
4. libvirt/Core restart and mode-conflict tests;
5. platform service restart, duplicate request, API timeout, and projection-rebuild tests.

Do not multiply every fault point across every consumer unless risk or a defect justifies it.

## 11. Forbidden outcomes

- VM identity loss;
- empty replacement database after corruption;
- libvirt bypass of Core operation journal in delegation mode;
- direct platform access to Cloud Hypervisor sockets or host helper;
- two authorities managing one VM;
- client-controlled backend-mode selection;
- silent mode switch while domains exist;
- upstream direct-mode regression hidden by CellHV tests;
- fabricated capability or statistic;
- silent XML/device omission;
- duplicate VM after retry;
- management-plane outage stopping a VM;
- leaked units, processes, TAPs, namespaces, mappings, files, or records.

## 12. Evidence

### Minimal

- result and environment versions;
- Core/libvirt/platform package versions;
- requested URI and active backend mode;
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
- mode-selection configuration and version;
- direct-mode regression result;
- test results and logs;
- upgrade/rollback result;
- signed evidence manifest;
- known unsupported functions.

## 13. Core 1.0 gate

Core 1.0 requires:

- minimal Core and recovery profiles;
- `ch:///system` CellHV delegation profile v1;
- bounded upstream direct-mode regression profile;
- `virsh` and language-binding profile;
- one existing cloud platform passing without a CellHV-specific adapter;
- measured OpenStack and CloudStack gap reports or passing profiles;
- advertised providers;
- upgrade, rollback, security, package, and soak qualification.
