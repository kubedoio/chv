# CellHV Core Foundation Implementation Plan

**Status:** Proposed  
**Date:** 2026-07-21  
**Authority:** ADR-015 and ADR-016

## 1. Objective

Evolve the existing `chv-agent` into a self-contained, locally authoritative CellHV Core runtime without creating a second daemon or performing a flag-day rewrite.

The active programme focuses on:

1. local authority and durable state;
2. one qualified Cloud Hypervisor VM lifecycle;
3. crash/reboot recovery;
4. minimum provider and privilege hardening;
5. one evidence-selected OpenStack integration;
6. Controller/O3K migration and Core 1.0 qualification.

CloudStack, OpenNebula, additional libvirt work, and other VMMs remain separate follow-on programmes.

## 2. Migration decision

`chv-agent` and CellHV Core are the same runtime.

- do not create `cellhvd`;
- keep the existing binary and service name until a separate naming ADR;
- add local authority beneath the current agent interfaces;
- retain the current control-plane gRPC path during migration;
- route legacy control-plane and new native requests into one operation engine;
- migrate the JSON node cache into the new durable store only through an explicit migration path;
- never allow old and new runtime paths to control the same VM independently.

## 3. Repository mapping

| Existing area | Direction |
|---|---|
| `chv-agent` binary/service | evolves into CellHV Core runtime |
| `chv-agent-core` | primary extraction/refactoring location for authority, lifecycle, recovery, and API |
| `chv-agent-runtime-ch` | Cloud Hypervisor VMM implementation reused and narrowed |
| current agent NodeCache/JSON | migration input and temporary compatibility cache, never final authority |
| existing agent gRPC | retained compatibility surface routed into the same operation engine |
| `chv-stord-*` | retained provider service; narrow contract before redesign |
| `chv-nwd-*` | retained provider service; narrow contract before redesign |
| `chv-common`, `chv-errors`, `chv-observability` | reuse |
| Controller, UI, Designer | remain above Core and become projections/clients |

Indicative target structure, without a mandatory rename:

```text
cmd/chv-agent/
api/openapi/cellhv-core-v1.yaml
crates/chv-agent-core/
crates/cellhv-core-store/
crates/cellhv-core-api/
crates/cellhv-core-operations/
crates/chv-agent-runtime-ch/
crates/chv-nwd-*/
crates/chv-stord-*/
integrations/openstack/
tests/qualification/
```

New crates are added only when they enforce a real dependency boundary. Empty architecture placeholders are forbidden.

## 4. Active phases

### Phase A — baseline, migration lock, and OpenStack discovery

Duration estimate: 1–2 engineering weeks.

Work:

- map current `chv-agent` state, lifecycle, process, storage, and network ownership;
- enforce ADR-016 in documentation and dependency guards;
- define the initial host/VMM/guest qualification tuple;
- add static checks preventing QEMU identity and cloud models in Core;
- run a time-boxed DevStack/Nova discovery against upstream `ch:///system`;
- inventory exact Nova/libvirt/QEMU assumptions;
- estimate the smallest native Nova driver path;
- publish evidence, not a final support claim.

Exit:

- migration path from current agent is unambiguous;
- no parallel runtime is planned;
- OpenStack Path A/B/C has factual initial data;
- later prompts can target concrete code.

### Phase B — local authority inside `chv-agent`

Duration estimate: 4–6 engineering weeks.

Work:

- platform-neutral Core domain types;
- SQLite schema and migrations;
- durable operation journal;
- idempotency and resource versions;
- one authority path for legacy gRPC and native local requests;
- native API skeleton over a Unix socket;
- truthful capabilities with no unimplemented behavior;
- migration adapter from existing NodeCache where necessary.

Exit:

- agent starts without Controller;
- accepts and persists a VM definition and operation;
- survives restart without losing identity;
- performs no duplicate or parallel VM action.

### Phase C — minimal standalone runtime and recovery

Duration estimate: 6–8 engineering weeks.

Work:

- reuse/narrow the existing Cloud Hypervisor runtime adapter;
- one qualified Linux guest;
- one pre-existing disk and network endpoint;
- create, inspect, start, stop, reboot, and delete;
- daemon restart and process re-adoption;
- host reboot policy;
- fail-closed database behavior;
- ownership markers and conflict protection;
- real-KVM leak and fault tests.

Exit:

- standalone lifecycle and recovery profiles pass on the qualification tuple.

### Phase D — minimum provider and privilege hardening

Duration estimate: 4–6 engineering weeks.

Work:

- document and narrow `chv-nwd` and `chv-stord` contracts;
- prove attachment ownership, recovery, detach, and cleanup;
- isolate only the privileged mutations actually required;
- qualify the minimum network and storage paths required by the chosen OpenStack route;
- do not implement every planned provider.

Exit:

- minimum network/storage profiles pass independently;
- no arbitrary privileged command surface exists.

### Phase E — first supported OpenStack integration

Duration estimate: 6–10 engineering weeks after Phase A evidence.

Select one path through a short decision ADR:

- generic libvirt `ch` path;
- generic upstream Nova/libvirt changes;
- official CellHV Nova `ComputeDriver` using the native API.

Requirements:

- Nova lifecycle and Placement reporting;
- Neutron network mapping through a qualified network path;
- Cinder block mapping through a qualified storage path;
- retry and nova-compute restart idempotency;
- no bypass of agent/Core authority;
- published version matrix and unsupported features;
- named maintenance owner.

Exit:

- one OpenStack compatibility claim tuple reaches Preview or Supported level.

### Phase F — Controller/O3K migration and Core 1.0

Duration estimate: 6–8 engineering weeks.

Work:

- migrate Controller operations to the public Core authority path;
- migrate O3K to the native Core API;
- prove projection rebuild after manager loss;
- package and service upgrade/rollback;
- security review, SBOM, checksums, and signed artifacts;
- 24-hour and extended soak profiles;
- operator documentation and support matrix.

Exit:

- Core 1.0 qualification passes for the published scope.

## 5. Deferred programmes

The following are strategic but not part of the active Core 1.0 implementation sequence:

- CloudStack discovery and integration;
- OpenNebula discovery and integration;
- broad `ch:///system` productisation beyond demonstrated consumers;
- Kubernetes and Terraform providers;
- additional network/storage providers;
- other VMM backends;
- Designer integration.

Each deferred programme receives its own discovery evidence, ADR where necessary, prompts, maintainers, and acceptance profile.

## 6. Resource and schedule assumptions

Minimum assumed capacity:

- one dedicated senior Rust/Linux virtualization engineer;
- half-time infrastructure/test engineer;
- disposable KVM and OpenStack labs;
- regular architecture review.

Indicative schedule from July 2026:

| Period | Target |
|---|---|
| Q3 2026 | Phase A and start Phase B |
| Q4 2026 | complete Phase B; begin Phase C |
| Q1 2027 | complete Phase C and Phase D |
| Q2 2027 | Phase E OpenStack integration |
| Q3 2027 | Phase F and Core 1.0 qualification |

These estimates are intentionally conservative. With less capacity, extend dates rather than weakening recovery, safety, or acceptance gates.

## 7. PR discipline

One narrow PR per implementation slice. A phase may require multiple PRs.

Each PR states:

- current phase and slice;
- existing code being evolved;
- authority impact;
- acceptance IDs;
- explicit non-scope;
- migration and rollback;
- test tier and evidence;
- residual risks.

No PR may combine a new state model, new provider family, new cloud integration, and unrelated UI work.

## 8. Coding-agent rules

Agents MUST NOT:

- create a second `cellhvd` runtime;
- perform a flag-day rewrite or broad rename;
- expose Cloud Hypervisor as `qemu:///system`;
- add QMP emulation;
- infer platform support from a connection test;
- infer network/storage support from VM lifecycle;
- write cloud-platform state into Core;
- bypass public Core APIs or the single operation engine;
- fabricate capabilities;
- silently accept unsupported XML/devices;
- claim compatibility from mocks;
- begin CloudStack/OpenNebula implementation before the active OpenStack and Core gates are complete.

## 9. Open decisions

- exact native local API implementation after prototype validation;
- selected OpenStack integration path;
- minimum OpenStack network and storage paths;
- exact privilege boundary between `chv-agent`, `chv-nwd`, and `chv-stord`;
- support distributions and pinned versions;
- final product binary/package naming;
- long-term maintenance ownership for ecosystem bridges.
