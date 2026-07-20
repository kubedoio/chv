# CellHV Core Acceptance Test Harness Implementation Plan

**Status:** Proposed  
**Date:** 2026-07-21  
**Authority:** ADR-016 and ADR-017

## 1. Objective

Build the smallest harness that proves:

- `chv-agent` evolves into one standalone Core authority;
- Cloud Hypervisor identity and capabilities are truthful;
- local VM lifecycle and recovery work on real KVM;
- advertised network and storage paths pass independent contracts;
- the selected OpenStack integration works on a real platform;
- published compatibility claims resolve to evidence.

The harness observes public interfaces and host state. It never repairs private product state or implements missing product behavior.

## 2. Active commands

```text
cellhv-test verify-host
cellhv-test list
cellhv-test run <scenario-id>
cellhv-test collect <run-id>
cellhv-test report <run-id>
cellhv-test gap-report <run-id>
cellhv-test claim <run-id>
```

Implement commands incrementally. Do not build a report portal or distributed scheduler as part of Core 1.0.

## 3. Active environment classes

| Environment | Purpose |
|---|---|
| normal CI | schemas, dependencies, identity, property tests |
| privileged disposable Linux | `chv-agent`, SQLite, sockets, permissions, provider contracts |
| Cloud Hypervisor KVM host | real lifecycle, recovery, ownership, and leaks |
| libvirt discovery host | bounded upstream `ch` inventory only |
| OpenStack lab | Path A/B/C discovery and selected-path qualification |
| provider lab | minimum network and storage paths required by OpenStack |
| release lab | package, upgrade, rollback, security, and soak |

CloudStack and OpenNebula labs are deferred and are not built by this plan.

## 4. Minimal harness modules

### Initial modules

- scenario registry and validator;
- test-host safety guard;
- public `chv-agent` Core client;
- legacy gRPC client where migration comparison is required;
- process/systemd/socket inventory;
- VMM identity verifier;
- operation/event collector;
- leak checker;
- JSON/JUnit report output.

### Added only when required

- network provider runner;
- storage provider runner;
- libvirt discovery runner;
- OpenStack discovery/qualification runner;
- upgrade/rollback runner;
- compatibility-claim generator.

The test harness may drive a product adapter but MUST NOT contain lifecycle, recovery, or provider logic that belongs in the product.

## 5. Agent/Core migration verification

The harness records:

- active runtime binary and service;
- durable database identity;
- legacy and native request correlation;
- VM UUID and operation IDs;
- Cloud Hypervisor process, unit, socket, and runtime directory;
- Controller presence/absence;
- ownership conflicts.

It fails when:

- a parallel `cellhvd` service exists;
- old and new request paths create separate operations for one intended mutation;
- two services own one VM;
- a future rename activates both old and new services;
- Controller removal erases local VM identity.

## 6. VMM identity verification

Record:

- expected and actual VMM backend/version;
- process executable;
- reported hypervisor type;
- API sockets;
- systemd units and ownership markers;
- advertised capabilities.

Fail when:

- Cloud Hypervisor is reported as QEMU;
- QEMU/QMP functions are advertised;
- capabilities cannot be executed;
- two authorities own the same VMM process.

Other VMMs are outside the active harness scope.

## 7. Network and storage contracts

Provider runners are independent from VM lifecycle tests.

### Network evidence

- endpoint source, type, and owner;
- bridge/TAP/VLAN/namespace state for the advertised path;
- guest connectivity;
- firewall changes;
- agent/provider/host restart behavior;
- cleanup and leak result;
- proof unrelated host state is unchanged.

### Storage evidence

- endpoint source, type, and owner;
- file/block/provider identity;
- lock/exclusivity state;
- guest data integrity;
- agent/provider/host restart behavior;
- cleanup and leak result;
- proof unowned data is not deleted.

Only providers required by the first OpenStack path are implemented in the active programme.

## 8. OpenStack discovery runner

The discovery runner compares:

- Nova `LibvirtDriver` with upstream `ch`;
- small generic upstream changes;
- native CellHV Nova driver.

Required report fields:

```yaml
openstack_version:
nova_version:
libvirt_version:
cloud_hypervisor_version:
host_kernel:
candidate:
configuration:
first_success:
first_failure:
qemu_specific_assumptions:
network_expectation:
storage_expectation:
core_authority_impact:
generic_upstream_option:
native_driver_effort:
security_risk:
maintenance_risk:
result:
recommended_path:
```

The discovery runner must stop at the time-box boundary and report partial evidence honestly.

## 9. Selected OpenStack path qualification

Once a focused ADR selects a path, the harness proves:

- Placement/resource reporting;
- spawn, inspect, power, reboot, and destroy;
- Neutron mapping through an independent network profile;
- Cinder mapping through an independent storage profile;
- duplicate-request and timeout behavior;
- nova-compute, `chv-agent`, and host restart;
- OpenStack management outage without workload loss;
- exact version and unsupported-feature matrix;
- no Core-authority bypass.

## 10. Evidence profiles

### Minimal successful run

- scenario result;
- exact versions;
- active runtime service;
- VMM identity;
- Core operations/events;
- network/storage path identifiers where applicable;
- leak result.

### Failure run

- minimal evidence;
- relevant logs;
- API/XML payloads;
- process/network/storage inventory;
- correlation timeline;
- gap report;
- database integrity result where relevant.

### Qualification run

- complete claim tuple;
- package/configuration manifest;
- all required scenarios;
- upgrade/rollback result;
- security result;
- soak result;
- signed evidence digest;
- unsupported matrix.

## 11. Safety

Destructive tests require:

- explicit test-host marker;
- reserved resource prefixes;
- disposable VM/network/storage;
- no production domains;
- approved provider namespaces;
- cleanup and leak assertion.

The harness aborts when isolation cannot be proven.

## 12. Phased harness implementation

### H0 — migration and static guards

- scenario and claim schemas;
- `chv-agent`/no-`cellhvd` identity checks;
- forbidden QEMU-identity checks;
- minimal report skeleton.

### H1 — real Core lifecycle

- host verifier;
- native and legacy clients;
- real-KVM lifecycle;
- operation correlation;
- process/unit/socket inventory.

### H2 — recovery and leaks

- agent restart;
- host reboot continuation;
- database integrity;
- representative failpoints;
- 100-cycle leak suite.

### H3 — minimum providers

- network contract runner;
- storage contract runner;
- restart, integrity, cleanup, and unrelated-state tests.

### H4 — OpenStack discovery

- time-boxed Path A/B/C comparison;
- common evidence and gap report;
- no support claim.

### H5 — selected OpenStack qualification

- selected adapter/path runner;
- Nova/Placement, Neutron, Cinder, retry, and restart tests;
- compatibility claim generation.

### H6 — release qualification

- package install/upgrade/rollback;
- security checks;
- soak and resource budgets;
- signed claims and matrices.

## 13. First harness PR sequence

1. scenario and claim schemas;
2. host safety guard;
3. agent/Core identity verifier;
4. VMM identity verifier;
5. native/legacy request correlation;
6. real lifecycle runner;
7. recovery and leak suite;
8. network contract runner;
9. storage contract runner;
10. OpenStack discovery runner;
11. gap-report generator;
12. selected OpenStack qualification;
13. claim and evidence generator;
14. package/upgrade/soak runners.

## 14. Deferred harness work

Do not implement in this programme:

- CloudStack comparison runner;
- OpenNebula comparison runner;
- additional VMM verification;
- Kubernetes/Terraform/Designer labs;
- multi-region test scheduler;
- report portal.

Those receive separate plans after Core 1.0 and first OpenStack stability.
