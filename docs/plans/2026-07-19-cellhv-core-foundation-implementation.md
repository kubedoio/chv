# CellHV Core Foundation Implementation Plan

**Status:** Proposed  
**Date:** 2026-07-20  
**Depends on:**

- `docs/specs/cellhv-core-foundation-spec.md`
- `docs/specs/adr/015-libvirt-first-ecosystem-compatibility.md`
- `docs/specs/contracts/cellhv-libvirt-compatibility-profile-v1.md`
- `docs/specs/cellhv-core-api-cloud-integration-spec.md`

## 1. Objective

Extract a small autonomous Core from the current control-plane-led implementation, then add one libvirt compatibility bridge that can serve multiple existing cloud platforms through the existing `ch:///system` identity.

The implementation MUST avoid a flag-day rewrite, MUST keep one VM authority, and MUST avoid creating a new libvirt URI until the existing `ch` path has been proven unsuitable.

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
| upstream libvirt `ch` driver | preferred compatibility identity, direct-mode baseline, and delegation-mode implementation target |

## 3. Target shape

```text
api/openapi/cellhv-core-v1.yaml
cmd/cellhvd/
cmd/cellhv-hostd/
crates/cellhv-core-*/
crates/cellhv-runtime-cloud-hypervisor/
crates/cellhv-*-provider-*/
integrations/libvirt-ch-delegation/
tests/libvirt-conformance/
tests/platform-conformance/
```

The libvirt integration MUST depend on the public Core client/service boundary. Core MUST NOT depend on libvirt.

The preferred implementation changes or packages the existing libvirt Cloud Hypervisor driver so that `ch:///system` can operate in an explicit CellHV delegation mode. A new libvirt driver directory or URI is not part of the first implementation path.

## 4. Phase plan

### Phase 0 — authority and compatibility lock

- accept ADR-015;
- review the libvirt v1 contract;
- inventory current Core/runtime paths;
- inventory upstream libvirt `ch` APIs, XML, events, statistics, storage/network interactions, and direct process ownership;
- identify how existing `ch` driver code can delegate to Core without changing client URI identity;
- identify all mode-selection, packaging, and upgrade implications;
- establish dependency guards;
- record current direct-mode resource and lifecycle baseline.

Exit: no ambiguity about who launches, owns, and recovers a VM, and no assumption that URI compatibility equals platform compatibility.

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
- ownership-conflict detection;
- runtime namespace and ownership markers suitable for libvirt mode isolation.

Exit: recovery profile passes.

### Phase 4 — upstream `ch` discovery and delegation spike

Run two bounded experiments using the same public URI:

1. test upstream `ch:///system` direct mode with `virsh` and selected platform clients to measure the current baseline;
2. prototype `ch:///system` CellHV delegation mode so every mutation reaches Core.

The prototype MUST NOT expose a new `cellhv:///system` URI.

Deliver:

- upstream `ch` function and XML support matrix;
- direct-mode process, socket, storage, and network ownership map;
- delegation seam options inside the existing driver;
- trusted host-local mode-selection options;
- direct/delegated namespace-isolation design;
- code-sharing and patch-scope decision;
- downstream-package versus upstream-proposal analysis;
- security boundary review;
- estimate for a separate driver only as a fallback.

Exit: prove or reject `ch:///system` delegation without weakening single authority or upstream direct mode.

### Phase 5 — `ch:///system` CellHV delegation profile v1

- connection and capability APIs using the existing URI;
- preserve upstream driver identity;
- explicit host-local delegation-mode configuration;
- domain XML parser/translator;
- identity and lookup;
- lifecycle;
- events;
- basic statistics;
- supported disk/NIC attachment;
- explicit unsupported APIs;
- Core operation correlation;
- restart/rebuild behavior;
- direct-mode regression profile;
- mode-switch and ownership-conflict protection.

Exit: LIBVIRT-001 through LIBVIRT-015 and CH-MODE-001 through CH-MODE-008 pass.

### Phase 6 — OpenStack unchanged-path experiment

- configure upstream Nova LibvirtDriver with `connection_uri = ch:///system` or the supported equivalent;
- enable CellHV delegation mode through host-local configuration, not Nova-specific logic;
- run supported lifecycle;
- collect every generic, QEMU-specific, `virt_type`, capability, networking, storage, image, console, and migration assumption;
- propose generic upstream fixes;
- avoid a CellHV ComputeDriver.

Exit: passing profile or a reviewed gap report.

### Phase 7 — CloudStack unchanged-path experiment

- determine whether the standard KVM agent can configure `ch:///system`;
- if connection is possible, test lifecycle, network, storage, hooks, image tooling, and monitoring;
- if connection is not possible, record the exact hard-coded/configuration blocker;
- classify QEMU-specific assumptions;
- propose generic upstream changes;
- avoid a CellHV extension/plugin.

Exit: passing profile or a reviewed gap report.

### Phase 8 — OpenNebula unchanged-path experiment

- configure existing KVM/VMM driver with `LIBVIRT_URI = ch:///system`;
- enable delegation through host-local configuration;
- test lifecycle and monitoring;
- classify template/QEMU assumptions.

Exit: passing profile or reviewed gap report.

### Phase 9 — fallback decisions

For each platform that does not pass:

- evaluate compatibility-profile extension;
- evaluate generic libvirt or platform patch;
- estimate long-term maintenance;
- issue a new ADR only if a CellHV-specific platform adapter is necessary.

For the libvirt URI/driver itself:

- compare upstream `ch` delegation maintenance with a downstream package;
- evaluate whether a separate `cellhv:///system` driver would actually improve safety or maintainability;
- require a new ADR before implementing a separate driver or URI.

No fallback adapter or separate URI begins without this gate.

### Phase 10 — providers, managed endpoint, and Core 1.0

- privilege helper;
- standard providers;
- HTTPS/mTLS and enrollment;
- Controller and O3K native API integration;
- packages, upgrades, rollback, soak, SBOM;
- upstream libvirt submission or maintained downstream `ch` delegation package;
- documented direct/delegated mode transition and rollback procedure.

## 5. Initial pull-request sequence

1. ADR-015 and v1 compatibility contract using `ch:///system`.
2. Core dependency guard and authority types.
3. SQLite state and operation journal.
4. Native API skeleton and client.
5. Minimal Cloud Hypervisor runtime.
6. Recovery, runtime ownership markers, and conflict checks.
7. Libvirt `ch` inventory and automated support-matrix extractor.
8. Trusted direct/delegated mode configuration spike.
9. `ch:///system` delegation proof of concept.
10. Lifecycle and XML delegation profile.
11. Events/statistics and attachment profile.
12. Direct-mode regression and mode-conflict suite.
13. OpenStack unchanged-path lab.
14. CloudStack unchanged-path lab.
15. OpenNebula unchanged-path lab.
16. Fallback ADRs only where required.

## 6. Coding-agent rules

Every task must name:

- phase;
- affected authority boundary;
- active libvirt mode where relevant;
- contract APIs;
- acceptance IDs;
- unsupported behavior;
- rollback.

Agents MUST NOT:

- add platform models to Core;
- introduce `cellhv:///system` without a new approved ADR;
- let URI query data, domain XML, or cloud request data select backend mode;
- call Cloud Hypervisor directly from the libvirt driver in CellHV delegation mode;
- regress inventoried direct-mode behavior without an explicit upstream compatibility decision;
- store a second authoritative VM definition;
- silently accept unsupported domain XML;
- start a platform-specific adapter before the fallback gate;
- claim compatibility from mocks;
- remove current paths before parity and rollback are proven.

## 7. Open decisions

- exact trusted host-local delegation-mode configuration;
- generic upstream backend abstraction versus downstream libvirt patch/package;
- exact libvirt version range;
- code reuse and modification scope in the existing `ch` driver;
- service and package topology;
- direct/delegated runtime namespace separation;
- standard storage/network driver coexistence;
- Nova `virt_type` and connection configuration;
- CloudStack agent connection configurability;
- OpenNebula template restrictions;
- first platform selected for Core 1.0 qualification;
- separate `cellhv:///system` fallback only if the preferred path fails.
