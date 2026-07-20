# CellHV Core Acceptance Test Harness Implementation Plan

**Status:** Proposed  
**Date:** 2026-07-20

## 1. Objective

Build the smallest harness that proves:

- standalone Core behavior;
- truthful VMM identity;
- independent network and storage profiles;
- real cloud-platform integration paths;
- compatibility claims backed by evidence.

The harness observes public interfaces and never repairs private product state.

## 2. Commands

```text
cellhv-test verify-host
cellhv-test list
cellhv-test run <scenario-id>
cellhv-test collect <run-id>
cellhv-test report <run-id>
cellhv-test gap-report <run-id>
cellhv-test claim <run-id>
```

## 3. Environment classes

| Environment | Purpose |
|---|---|
| normal CI | schema, dependency, identity, property tests |
| privileged disposable Linux | services, SQLite, providers, permissions |
| Cloud Hypervisor KVM host | real Core lifecycle and recovery |
| libvirt discovery host | upstream `ch` inventory and optional profile |
| OpenStack lab | compare generic libvirt and native adapter paths |
| CloudStack lab | measure KVM-agent assumptions and candidate paths |
| OpenNebula lab | compare integration paths |
| provider lab | network and storage qualification |
| release lab | upgrade, rollback, soak, packages |

## 4. Core modules

- scenario registry;
- host safety guard;
- native Core client;
- process/systemd/socket inventory;
- VMM identity verifier;
- network inventory;
- storage inventory;
- operation/event collector;
- provider contract runner;
- libvirt discovery runner;
- platform adapters for testing only;
- leak checker;
- JSON/JUnit reports;
- gap-report generator;
- compatibility-claim generator.

The test harness may drive a product adapter but MUST NOT implement product behavior itself.

## 5. VMM identity verification

The harness records:

- expected VMM backend;
- actual process executable and version;
- libvirt URI where used;
- reported hypervisor type;
- VMM API sockets;
- systemd units and ownership markers;
- advertised capabilities.

It fails when:

- Cloud Hypervisor is reported as QEMU;
- QEMU/QMP functionality is advertised without actual support;
- a future QEMU profile has no QEMU process;
- two VMM authorities own the same VM.

## 6. Network and storage contracts

Provider runners are independent from VM lifecycle tests.

### Network evidence

- endpoint source and owner;
- bridge/TAP/VLAN/namespace state;
- guest connectivity;
- firewall changes;
- restart recovery;
- cleanup and leak result.

### Storage evidence

- endpoint source and owner;
- file/block/provider identity;
- lock/exclusivity state;
- guest data integrity;
- restart recovery;
- cleanup and leak result.

## 7. Platform comparison labs

Each platform discovery run may evaluate several integration candidates.

Required report fields:

```yaml
platform:
platform_version:
candidate:
vmm_backend:
hypervisor_interface:
network_path:
storage_path:
configuration:
platform_patches:
cellhv_adapter:
expected:
observed:
qemu_specific_assumptions:
generic_upstream_option:
security_risk:
maintenance_cost:
result:
recommended_path:
```

### OpenStack

Compare:

- Nova LibvirtDriver with `ch`;
- generic upstream changes;
- native CellHV ComputeDriver.

### CloudStack

Compare:

- standard KVM agent with non-QEMU libvirt;
- extension framework;
- native hypervisor plugin;
- future actual QEMU backend only if separately approved.

### OpenNebula

Compare:

- generic libvirt;
- generic VMM changes;
- native CellHV VMM driver.

A candidate cannot be selected solely because it needs fewer initial code changes.

## 8. Evidence profiles

### Minimal

- scenario result;
- exact versions;
- VMM identity;
- Core operations/events;
- network/storage path identifiers;
- leak result.

### Failure

- minimal evidence;
- logs;
- API/XML payloads;
- process/network/storage inventory;
- correlation timeline;
- gap report.

### Qualification

- complete claim tuple;
- package/configuration manifest;
- all required scenarios;
- upgrade and rollback;
- security result;
- signed evidence digest;
- unsupported matrix.

## 9. Safety

Destructive tests require:

- explicit test-host marker;
- reserved resource prefixes;
- disposable VM/network/storage;
- no production domains;
- approved provider namespaces;
- cleanup and leak assertion.

The harness aborts when isolation cannot be proven.

## 10. Phased implementation

### H0 — schemas and static checks

- scenario schema;
- compatibility-claim schema;
- forbidden QEMU-identity checks;
- report skeleton.

### H1 — Core lifecycle

- native client;
- KVM host verification;
- lifecycle, recovery, and leak tests.

### H2 — provider contracts

- network runner;
- storage runner;
- restart and cleanup tests.

### H3 — libvirt discovery

- upstream `ch` support inventory;
- URI, XML, events, statistics, and process ownership;
- optional bounded profile.

### H4 — OpenStack comparison

- run each candidate path;
- collect common evidence;
- compare reliability and maintenance.

### H5 — CloudStack comparison

- capture URI, QEMU hook/tooling, storage, and network assumptions;
- test viable candidate paths.

### H6 — OpenNebula comparison

- run and compare viable paths.

### H7 — qualification

- packages;
- upgrade/rollback;
- soak;
- signed claims and matrices.

## 11. First harness PR sequence

1. scenario and claim schemas;
2. host safety guard;
3. VMM identity verifier;
4. native Core runner;
5. network contract runner;
6. storage contract runner;
7. recovery and leak suite;
8. libvirt `ch` discovery runner;
9. gap-report generator;
10. OpenStack candidate comparison;
11. CloudStack candidate comparison;
12. OpenNebula candidate comparison;
13. claim and evidence generator.
