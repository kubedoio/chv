# CellHV Core Acceptance Test Harness Implementation Plan

**Status:** Proposed  
**Date:** 2026-07-20  
**Depends on:**

- `docs/specs/cellhv-core-acceptance-test-spec.md`
- `docs/specs/contracts/cellhv-libvirt-compatibility-profile-v1.md`
- `docs/plans/2026-07-19-cellhv-core-foundation-implementation.md`

## 1. Objective

Build the smallest harness that proves standalone Core behavior, libvirt compatibility, and unchanged existing-platform integration paths.

The harness MUST observe public interfaces and MUST NOT repair product state or write private databases.

## 2. Initial commands

```text
cellhv-test verify-host
cellhv-test list
cellhv-test run <scenario-id>
cellhv-test collect <run-id>
cellhv-test report <run-id>
cellhv-test gap-report <run-id>
```

## 3. Environment classes

| Environment | Purpose |
|---|---|
| normal CI | schemas, dependencies, unit/property, XML mapping |
| privileged disposable Linux | daemons, sockets, SQLite, permissions |
| qualified KVM host | real Core and libvirt VM lifecycle |
| libvirt compatibility host | CellHV libvirt driver, `virsh`, bindings, and restart tests |
| OpenStack lab | upstream Nova LibvirtDriver path |
| CloudStack lab | standard KVM agent path |
| OpenNebula lab | standard KVM/VMM path |
| release lab | packages, upgrade/rollback, soak |

Cloud labs must use published upstream packages plus documented configuration. Any CellHV-specific platform patch or package invalidates the “unchanged path” profile and must be declared.

## 4. Harness modules

Initial modules:

- scenario registry and schema validation;
- host safety guard;
- native Core client;
- libvirt client runner;
- `virsh` runner;
- process/systemd inventory;
- network/storage inventory;
- operation/event collector;
- leak checker;
- JSON/JUnit output;
- gap-report generator.

Deferred:

- multi-host scheduler;
- report portal;
- automatic upstream patching;
- full cloud-lab lifecycle management;
- advanced Ceph/network chaos.

## 5. Libvirt conformance implementation

The harness must test:

- connection URI;
- capability XML;
- domain XML acceptance/rejection;
- identity and lookup;
- lifecycle and persistence;
- events;
- statistics;
- disk/NIC attach/detach;
- driver/Core restart;
- operation correlation;
- security boundary;
- mixed native/libvirt conflicts;
- direct `ch` ownership conflict.

The harness captures each libvirt API call, return/error, related Core operation, and resulting process state.

## 6. Platform lab rule

The platform is driven through its own public API or CLI. The harness observes:

- platform request;
- libvirt call/XML;
- Core operation/event;
- actual Linux/Cloud Hypervisor state.

This correlation chain is mandatory evidence.

## 7. Gap reports

A failed unchanged-path scenario generates machine-readable and Markdown reports:

```yaml
platform:
platform_version:
scenario:
configuration:
libvirt_api:
domain_xml_fragment:
expected:
observed:
layer: core|cellhv-libvirt|libvirt|platform
classification: missing-profile|generic-backend-assumption|qemu-specific|configuration|defect
generic_upstream_fix:
fallback_adapter_required: unknown|no|yes
security_impact:
maintenance_impact:
```

A `yes` value is not sufficient to start an adapter; an ADR is still required.

## 8. Safety

Destructive execution requires:

- test-host marker;
- reserved resource prefixes;
- disposable test VM/storage/network;
- no production Core or libvirt domains;
- explicit environment profile;
- cleanup and leak check.

The harness aborts when it cannot prove isolation.

## 9. Evidence profiles

### Minimal

- versions and package digests;
- scenario result;
- operations/events;
- libvirt calls;
- leak result.

### Failure

- minimal evidence;
- Core, libvirt, and platform logs;
- XML/API payloads with secrets redacted;
- process/network/storage inventories;
- correlation timeline;
- gap report.

### Qualification

- complete support matrices;
- signed evidence manifest;
- package/configuration manifest;
- all required scenario results;
- upgrade/rollback and soak results.

## 10. Phased harness plan

### H0 — registry and static checks

- scenario schema;
- gates and profiles;
- API/contract lint;
- dependency guard;
- report skeleton.

### H1 — minimal Core

- host verification;
- native client;
- Core lifecycle and leak checks;
- recovery resume support.

### H2 — libvirt profile

- libvirt/virsh runners;
- XML fixtures;
- API call tracing;
- operation correlation;
- boundary audit;
- profile-v1 matrix generation.

### H3 — OpenStack

- deploy or connect to a supported lab;
- configure upstream LibvirtDriver;
- run OS-LIBVIRT scenarios;
- produce gap matrix.

### H4 — CloudStack

- deploy/connect standard KVM agent;
- run CS-LIBVIRT scenarios;
- capture hooks and QEMU assumptions;
- produce gap matrix.

### H5 — OpenNebula

- configure existing KVM/VMM driver;
- run ONE-LIBVIRT scenarios;
- produce gap matrix.

### H6 — qualification

- package installation;
- upgrade/rollback;
- long soak;
- signed evidence and published matrices.

## 11. First harness PR sequence

1. scenario registry and report schema;
2. host safety guard;
3. native Core runner;
4. libvirt connection/capability runner;
5. XML fixture suite;
6. lifecycle/events/stats runner;
7. operation-correlation and boundary checks;
8. gap-report generator;
9. OpenStack lab runner;
10. CloudStack lab runner;
11. OpenNebula lab runner.

No harness change may introduce product behavior or a platform-specific adapter.
