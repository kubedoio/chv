# Architecture Decision Records (ADRs)

This directory indexes all Architecture Decision Records for CHV. The canonical storage location is [`docs/specs/adr/`](../specs/adr/); this file provides a quick-reference index.

## Backend & Infrastructure ADRs

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| [ADR-001](../specs/adr/001-node-runtime-split.md) | Node Runtime Split | **Accepted** | Defines `chv-agent` (orchestrator), `chv-stord` (storage), `chv-nwd` (network), and `cloud-hypervisor` (VMM) as separate node-side daemons. Forbids collapsing storage/network into the agent. |
| [ADR-002](../specs/adr/002-control-plane-boundary.md) | Control Plane to Node Boundary | **Accepted** | Control plane communicates only with `chv-agent` over gRPC/mTLS. Cloud Hypervisor is local-only. Control plane owns desired state; nodes report observed state. |
| [ADR-003](../specs/adr/003-node-state-machine.md) | Node State Machine | **Accepted** | Defines explicit node states (`Discovered` → `TenantReady`) and scheduling rules. Only `TenantReady` nodes are schedulable. |
| [ADR-004](../specs/adr/004-storage-datapath.md) | Storage Datapath Model | **Accepted** | Rejects storage-VM/NBD for MVP-1 in favor of host-side `chv-stord`. Defines supported storage classes and runtime limits. |
| [ADR-005](../specs/adr/005-network-service-model.md) | Network Service Model | **Accepted** | Defers network-VM approach; `chv-nwd` is a host-side Linux bridge/netns/veth/nftables daemon. Defers eBPF and distributed overlay. |
| [ADR-006](../specs/adr/006-partition-policy.md) | Partition and Autonomy Policy | **Accepted** | Defines node behavior during control-plane outages: preserve runtime, allow limited local ops, deny destructive mutations, converge on reconnection. |
| [ADR-007](../specs/adr/007-upgrade-rollback.md) | Upgrade and Rollback Policy | **Accepted** | Mandates bundle-tested node upgrades with one-step rollback. Tracks versions for control plane, all node daemons, Cloud Hypervisor, and host helpers. |
| [ADR-008](../specs/adr/008-error-handling-patterns.md) | Error Handling Patterns | **Accepted** | Structured errors via `chv-errors`; no panics in service code; graceful mutex poison recovery; explicit gRPC/HTTP mapping. |
| [ADR-009](../specs/adr/009-logging-and-observability.md) | Logging and Observability | **Accepted** | `tracing` for structured logging; no `println!` in library crates; secret redaction; Prometheus metrics endpoint. |
| [ADR-010](../specs/adr/010-async-runtime-safety.md) | Async Runtime Safety | **Accepted** | `tokio::sync::Mutex` in async contexts; `std::sync::Mutex` only in sync helpers with graceful poison handling; minimize lock scope. |

## Web UI ADRs

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| [ADR-001-WebUI](../specs/adr/001-webui-product-principles.md) | WebUI Product Principles | **Accepted** | 7 principles: cluster-first nav, tasks as first-class, legible state, no browser-to-node coupling, progressive depth, predictable mutation UX, private-cloud-first usability. |
| [ADR-002-WebUI](../specs/adr/002-webui-architecture-boundary.md) | WebUI Architecture Boundary | **Accepted** | Browser talks only to control-plane BFF. Direct access to agent, stord, nwd, or CHV APIs is forbidden. |
| [ADR-003-WebUI](../specs/adr/003-webui-navigation-model.md) | WebUI Navigation Model | **Accepted** | Primary nav hierarchy and detail-page tab structure (Summary, Configuration, Tasks, Events, Related Resources). |
| [ADR-004-WebUI](../specs/adr/004-webui-task-and-state-model.md) | WebUI Task and State Model | **Accepted** | Tasks and state are first-class UI objects. Every mutation creates a task. Defines task states and resource health states. |
| [ADR-005-WebUI](../specs/adr/005-webui-design-system-direction.md) | WebUI Design System Direction | **Accepted** | Modern but restrained, enterprise-serious, light-mode first, high information density, border-first surfaces, strong typography. Avoids copying Proxmox/Xen Orchestra visually. |

## Architecture Designer ADRs

These ADRs scope the Architecture Designer feature (design-time topology editor with YAML source-of-truth, validation, plan/apply via the CHV task system, and drift detection). All currently **Proposed**; acceptance is per-ADR pending review.

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| [ADR-001-Designer](../specs/adr/001-designer-first-class-surface.md) | Architecture Designer as First-Class Surface | **Proposed** | Adds a `DESIGN` group above Fleet Overview with `Architecture Designer` and `Saved Topologies` routes. Designer is the design-time view; Fleet Overview remains the operational view. |
| [ADR-002-Designer](../specs/adr/002-designer-svelte-flow.md) | Use Svelte Flow for the Editable Designer Canvas | **Proposed** | Adopts Svelte Flow for the editable canvas; existing CHV live-topology canvas stays as the read-only operational view. |
| [ADR-003-Designer](../specs/adr/003-designer-yaml-source-of-truth.md) | CHVArchitecture YAML as Source of Truth | **Proposed** | `CHVArchitecture` YAML (`apiVersion: chv.kubedo.io/v1alpha1`) is the authoritative serialized form; the graph is derived. Schema lives at [`docs/schemas/chvarchitecture-v1alpha1.schema.json`](../schemas/chvarchitecture-v1alpha1.schema.json). |
| [ADR-004-Designer](../specs/adr/004-designer-validation-plan-apply.md) | Validation-, Plan-, and Task-Gated Deployment | **Proposed** | Deployment is forbidden without static validation, fleet consistency check, and a confirmed plan. Apply runs through the existing CHV task system. |
| [ADR-005-Designer](../specs/adr/005-designer-separate-desired-vs-live.md) | Separate Desired Topology from Live Fleet Topology | **Proposed** | The Designer's desired-state graph is intentionally separate from live-fleet topology data. Drift detection compares baseline vs current. |
| [ADR-006-Designer](../specs/adr/006-designer-no-tosca-engine.md) | Do Not Adopt a Generic Cloudify/TOSCA Engine | **Proposed** | The Designer borrows the topology-orchestrator pattern but stays scoped to CHV-native virtualization resources. No generic TOSCA/Cloudify DSL compatibility. |

Feature documentation (contracts, prompts, research notes, READMEs) lives at [`docs/specs/architecture-designer/`](../specs/architecture-designer/). Component specs live alongside other component specs at [`docs/specs/component/architecture-designer-*.md`](../specs/component/). The implementation roadmap is at [`docs/plans/2026-06-13-architecture-designer-roadmap.md`](../plans/2026-06-13-architecture-designer-roadmap.md).

## Lifecycle

```
PROPOSED → ACCEPTED → (SUPERSEDED by ADR-XXX or DEPRECATED)
```

Do not delete old ADRs. When a decision changes, write a new ADR that references and supersedes the old one.

## Naming Convention

ADRs use suffixes to disambiguate parallel namespaces with overlapping numbering:

- **Backend & Infrastructure ADRs** use no suffix (e.g., `ADR-001`).
- **Web UI ADRs** use the `-WebUI` suffix (e.g., `ADR-001-WebUI`).
- **Architecture Designer ADRs** use the `-Designer` suffix (e.g., `ADR-001-Designer`).

Cross-references to a non-backend ADR must include the suffix.

## Contributing

When making a significant architectural decision:

1. Write a new ADR following the template in [`documentation-and-adrs`](../.agents/skills/documentation-and-adrs/SKILL.md)
2. Store it in `docs/specs/adr/` with sequential numbering
3. Update this index
4. If the decision changes an existing ADR, mark the old one as `Superseded by ADR-XXX`
