# CellHV Core Foundation Implementation Plan

**Status:** Proposed  
**Date:** 2026-07-20  
**Depends on:**

- `docs/specs/cellhv-core-foundation-spec.md`
- `docs/specs/adr/015-libvirt-first-ecosystem-compatibility.md`
- `docs/specs/contracts/cellhv-libvirt-compatibility-profile-v1.md`
- `docs/specs/cellhv-core-api-cloud-integration-spec.md`

## 1. Objective

Extract a small autonomous Core from the current control-plane-led implementation, then add one libvirt compatibility bridge that can serve multiple existing cloud platforms.

The implementation MUST avoid a flag-day rewrite and MUST keep one VM authority.

## 2. Repository mapping

| Existing area | Direction |
|---|---|
| `chv-agent-runtime-ch` | source for Cloud Hypervisor runtime adapter |
| `chv-agent-core` | source for lifecycle, console, telemetry, and recovery |
| `chv-stord-*` | source for later storage providers |
| `chv-nwd-*` | source for later network providers |
| `chv-common`, `chv-errors`, `chv-observability` | reuse identifiers, errors, tracing, metrics |
| current node cache | compatibility input only; never authoritative Core store |
| existing gRPC | migration/internal compatibility; not the only public surface |
| Controller, Web UI, Designer | remain above Core |
| libvirt `ch` driver | external upstream reference and experiment target |

## 3. Target shape

```text
api/openapi/cellhv-core-v1.yaml
cmd/cellhvd/
cmd/cellhv-hostd/
crates/cellhv-core-*/
crates/cellhv-runtime-cloud-hypervisor/
crates/cellhv-*-provider-*/
integrations/libvirt/
tests/libvirt-conformance/
tests/platform-conformance/
```

The libvirt integration MUST depend on the public Core client/service boundary. Core MUST NOT depend on libvirt.

## 4. Phase plan

### Phase 0 — authority and compatibility lock

- accept ADR-015;
- review the libvirt v1 contract;
- inventory current Core/runtime paths;
- inventory current libvirt `ch` support matrix;
- identify how libvirt driver code can call Core;
- establish dependency guards;
- record current resource and lifecycle baseline.

Exit: no ambiguity about who launches, owns, and recovers a VM.

### Phase 1 — Core M0

- Core domain types;
- SQLite schema and migrations;
- operation journal;
- idempotency and resource versions;
- native OpenAPI skeleton;
- generated Rust client;
- no real VM required.

Exit: state and operation tests pass.

### Phase 2 — Core M1

- minimal `cellhvd`;
- one Cloud Hypervisor adapter;
- one Linux VM;
- pre-existing raw/block attachment;
- pre-existing bridge/TAP;
- create, inspect, start, stop, delete;
- compatibility adapter from current `chv-agent`.

Exit: minimal Core scenarios pass on real KVM.

### Phase 3 — Core M2

- daemon re-adoption;
- host reboot;
- fail-closed DB;
- crash recovery;
- leak tests;
- ownership-conflict detection.

Exit: recovery profile passes.

### Phase 4 — libvirt discovery spike

Run two bounded experiments:

1. test upstream `ch:///system` with `virsh`, Nova, CloudStack, and OpenNebula to measure existing capability;
2. prototype a `cellhv:///system` translation driver that sends every mutation to Core.

Deliver:

- upstream `ch` gap matrix;
- code-sharing decision;
- driver naming/URI decision;
- downstream-incubation versus upstream proposal;
- security boundary review.

Exit: choose the implementation path without weakening single authority.

### Phase 5 — libvirt profile v1

- connection and capability APIs;
- domain XML parser/translator;
- identity and lookup;
- lifecycle;
- events;
- basic statistics;
- supported disk/NIC attachment;
- explicit unsupported APIs;
- Core operation correlation;
- restart/rebuild behavior.

Exit: LIBVIRT-001 through LIBVIRT-015 pass.

### Phase 6 — OpenStack unchanged-path experiment

- configure Nova LibvirtDriver to use the CellHV libvirt path;
- run supported lifecycle;
- collect every generic/QEMU-specific assumption;
- propose generic upstream fixes;
- avoid a CellHV ComputeDriver.

Exit: passing profile or a reviewed gap report.

### Phase 7 — CloudStack unchanged-path experiment

- configure standard KVM agent against CellHV libvirt;
- test lifecycle, network, storage, hooks, monitoring;
- classify QEMU-specific assumptions;
- propose generic upstream changes;
- avoid a CellHV extension/plugin.

Exit: passing profile or a reviewed gap report.

### Phase 8 — OpenNebula unchanged-path experiment

- configure existing KVM/VMM driver with CellHV `LIBVIRT_URI`;
- test lifecycle and monitoring;
- classify template/QEMU assumptions.

Exit: passing profile or reviewed gap report.

### Phase 9 — fallback decision

For each platform that does not pass:

- evaluate compatibility-profile extension;
- evaluate generic libvirt or platform patch;
- estimate long-term maintenance;
- issue a new ADR only if a CellHV-specific adapter is necessary.

No fallback adapter begins without this gate.

### Phase 10 — providers, managed endpoint, and Core 1.0

- privilege helper;
- standard providers;
- HTTPS/mTLS and enrollment;
- Controller and O3K native API integration;
- packages, upgrades, rollback, soak, SBOM;
- upstream libvirt submission or maintained compatibility package.

## 5. Initial pull-request sequence

1. ADR-015 and v1 compatibility contract.
2. Core dependency guard and authority types.
3. SQLite state and operation journal.
4. Native API skeleton and client.
5. Minimal Cloud Hypervisor runtime.
6. Recovery and ownership-conflict checks.
7. Libvirt `ch` inventory and automated support-matrix extractor.
8. `cellhv:///system` proof of concept.
9. Lifecycle and XML profile.
10. Events/statistics and attachment profile.
11. OpenStack unchanged-path lab.
12. CloudStack unchanged-path lab.
13. OpenNebula unchanged-path lab.
14. Fallback ADRs only where required.

## 6. Coding-agent rules

Every task must name:

- phase;
- affected authority boundary;
- contract APIs;
- acceptance IDs;
- unsupported behavior;
- rollback.

Agents MUST NOT:

- add platform models to Core;
- call Cloud Hypervisor directly from the libvirt compatibility layer;
- store a second authoritative VM definition;
- silently accept unsupported domain XML;
- start a platform-specific adapter before the fallback gate;
- claim compatibility from mocks;
- remove current paths before parity and rollback are proven.

## 7. Open decisions

- libvirt upstream acceptance strategy;
- driver URI and reported hypervisor type;
- exact libvirt version range;
- code reuse with `src/ch`;
- service and package topology;
- standard storage/network driver coexistence;
- Nova `virt_type` and connection configuration;
- CloudStack agent connection configurability;
- OpenNebula template restrictions;
- first platform selected for Core 1.0 qualification.
