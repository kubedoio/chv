# CellHV Core Acceptance Test Harness Implementation Plan

**Status:** Proposed  
**Date:** 2026-07-20  
**Depends on:**

- `docs/specs/cellhv-core-acceptance-test-spec.md`
- `docs/specs/contracts/cellhv-libvirt-compatibility-profile-v1.md`
- `docs/plans/2026-07-19-cellhv-core-foundation-implementation.md`

## 1. Objective

Build the smallest harness that proves standalone Core behavior, `ch:///system` CellHV delegation, bounded upstream direct-mode preservation, and unchanged existing-platform integration paths.

The harness MUST observe public interfaces and MUST NOT repair product state or write private databases.

## 2. Initial commands

```text
cellhv-test verify-host
cellhv-test list
cellhv-test run <scenario-id>
cellhv-test collect <run-id>
cellhv-test report <run-id>
cellhv-test gap-report <run-id>
cellhv-test mode-status
```

`mode-status` reads only trusted host-local diagnostics. It does not change the libvirt backend mode.

## 3. Environment classes

| Environment | Purpose |
|---|---|
| normal CI | schemas, dependencies, unit/property, XML mapping, mode-selection validation |
| privileged disposable Linux | daemons, sockets, SQLite, permissions, libvirt daemon integration, mode isolation |
| qualified KVM host | real Core and libvirt VM lifecycle |
| libvirt direct-mode baseline host | inventory upstream `ch:///system` behavior without CellHV delegation |
| libvirt delegation host | `ch:///system` with CellHV delegation mode, `virsh`, bindings, restart tests |
| OpenStack lab | upstream Nova LibvirtDriver with `ch:///system` |
| CloudStack lab | standard KVM agent attempting `ch:///system` |
| OpenNebula lab | standard KVM/VMM path with `LIBVIRT_URI = ch:///system` |
| release lab | packages, upgrade/rollback, mode transition, soak |

Cloud labs must use published upstream packages plus documented standard configuration. Any CellHV-specific platform driver, extension, or plugin invalidates the “unchanged path” profile and must be declared.

## 4. Harness modules

Initial modules:

- scenario registry and schema validation;
- host safety guard;
- native Core client;
- libvirt client runner;
- `virsh` runner;
- trusted backend-mode inspector;
- libvirt direct/delegated namespace inspector;
- process/systemd inventory;
- network/storage inventory;
- operation/event collector;
- Cloud Hypervisor socket/process audit;
- leak checker;
- JSON/JUnit output;
- gap-report generator.

Deferred:

- multi-host scheduler;
- report portal;
- automatic upstream patching;
- full cloud-lab lifecycle management;
- advanced Ceph/network chaos;
- automatic backend-mode switching.

## 5. Libvirt conformance implementation

The harness must test:

- connection through `ch:///system`;
- preservation of the upstream driver identity;
- trusted host-local direct/delegated mode selection;
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
- direct/delegated ownership conflicts;
- bounded upstream direct-mode regression.

The harness captures each libvirt API call, return/error, active mode, related Core operation, and resulting process state.

In delegation mode it also proves that the libvirt driver does not open Cloud Hypervisor API sockets, launch Cloud Hypervisor, or mutate Linux resources directly.

## 6. Mode-isolation implementation

The harness records and compares:

- active mode and configuration source;
- VM UUIDs and names;
- Cloud Hypervisor process IDs;
- API socket paths;
- systemd units;
- runtime and state directories;
- ownership markers;
- network and storage attachment identities.

Required checks:

- client URI and domain XML cannot select mode;
- mode cannot change while domains or owned runtime resources exist;
- direct and delegated resources never overlap;
- delegation mode sends mutations through Core;
- direct mode does not contact Core;
- restart preserves mode and VM runtime;
- direct-mode regression uses an isolated host or clean runtime namespace.

## 7. Platform lab rule

The platform is driven through its own public API or CLI. The harness observes:

- platform request;
- requested libvirt URI and configuration;
- libvirt call/XML;
- active driver mode;
- Core operation/event in delegation mode;
- actual Linux/Cloud Hypervisor state.

This correlation chain is mandatory evidence.

A standard URI configuration change is permitted in the unchanged-path profile. Platform-specific CellHV code is not.

## 8. Gap reports

A failed unchanged-path scenario generates machine-readable and Markdown reports:

```yaml
platform:
platform_version:
scenario:
configuration:
requested_libvirt_uri:
active_backend_mode:
libvirt_api:
domain_xml_fragment:
expected:
observed:
layer: core|ch-delegation|libvirt|platform
classification: mode-configuration|missing-profile|generic-backend-assumption|qemu-specific|hard-coded-uri|configuration|defect
generic_upstream_fix:
fallback_adapter_required: unknown|no|yes
separate_driver_uri_required: unknown|no|yes
security_impact:
maintenance_impact:
```

A `yes` value is not sufficient to start an adapter or a separate `cellhv:///system` driver; an ADR is still required.

## 9. Safety

Destructive execution requires:

- test-host marker;
- reserved resource prefixes;
- disposable test VM/storage/network;
- no production Core or libvirt domains;
- explicit direct-mode or delegation-mode environment profile;
- cleanup and leak check.

The harness aborts when it cannot prove isolation.

Direct-mode and delegation-mode tests MUST NOT run concurrently on shared runtime resources.

## 10. Evidence profiles

### Minimal

- versions and package digests;
- scenario result;
- requested URI and active backend mode;
- operations/events where applicable;
- libvirt calls;
- leak result.

### Failure

- minimal evidence;
- Core, libvirt, and platform logs;
- mode configuration and diagnostics;
- XML/API payloads with secrets redacted;
- process/network/storage/socket inventories;
- correlation timeline;
- gap report.

### Qualification

- complete support matrices;
- signed evidence manifest;
- package/configuration manifest;
- active-mode configuration contract;
- direct-mode regression result;
- all required scenario results;
- upgrade, mode-transition, rollback, and soak results.

## 11. Phased harness plan

### H0 — registry and static checks

- scenario schema;
- gates and profiles;
- API/contract lint;
- dependency guard;
- mode-selection threat-model tests;
- report skeleton.

### H1 — minimal Core

- host verification;
- native client;
- Core lifecycle and leak checks;
- recovery resume support.

### H2 — upstream `ch` direct-mode baseline

- `ch:///system` connection runner;
- upstream function and XML inventory;
- direct lifecycle baseline;
- process/socket/resource inventory;
- support-matrix generator.

### H3 — CellHV delegation profile

- trusted mode inspector;
- libvirt/virsh runners using the same URI;
- XML fixtures;
- API call tracing;
- Core operation correlation;
- Cloud Hypervisor socket/process boundary audit;
- mixed-client and mode-conflict tests;
- direct-mode regression comparison;
- profile-v1 matrix generation.

### H4 — OpenStack

- deploy or connect to a supported lab;
- configure upstream LibvirtDriver with `ch:///system`;
- enable delegation only through host-local configuration;
- run OS-LIBVIRT scenarios;
- produce gap matrix.

### H5 — CloudStack

- deploy/connect standard KVM agent;
- attempt documented `ch:///system` configuration;
- capture exact hard-coded URI, hooks, tools, and QEMU assumptions;
- run CS-LIBVIRT scenarios where possible;
- produce gap matrix.

### H6 — OpenNebula

- configure existing KVM/VMM driver with `LIBVIRT_URI = ch:///system`;
- run ONE-LIBVIRT scenarios;
- produce gap matrix.

### H7 — qualification

- package installation;
- direct/delegated mode upgrade and rollback;
- invalid mode-switch tests;
- long soak;
- signed evidence and published matrices.

## 12. First harness PR sequence

1. scenario registry and report schema;
2. host safety guard;
3. native Core runner;
4. upstream `ch:///system` connection and support-matrix runner;
5. direct-mode lifecycle and resource baseline;
6. trusted mode-status and namespace inventory;
7. delegation-mode XML and lifecycle fixture suite;
8. operation-correlation and Cloud Hypervisor boundary checks;
9. mode-conflict and direct-mode regression suite;
10. gap-report generator;
11. OpenStack lab runner;
12. CloudStack lab runner;
13. OpenNebula lab runner.

No harness change may introduce product behavior, a platform-specific adapter, or a separate libvirt URI.
