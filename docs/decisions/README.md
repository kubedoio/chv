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

## Web UI ADRs

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| [ADR-001-WebUI](../specs/adr/001-webui-product-principles.md) | WebUI Product Principles | **Accepted** | 7 principles: cluster-first nav, tasks as first-class, legible state, no browser-to-node coupling, progressive depth, predictable mutation UX, private-cloud-first usability. |
| [ADR-002-WebUI](../specs/adr/002-webui-architecture-boundary.md) | WebUI Architecture Boundary | **Accepted** | Browser talks only to control-plane BFF. Direct access to agent, stord, nwd, or CHV APIs is forbidden. |
| [ADR-003-WebUI](../specs/adr/003-webui-navigation-model.md) | WebUI Navigation Model | **Accepted** | Primary nav hierarchy and detail-page tab structure (Summary, Configuration, Tasks, Events, Related Resources). |
| [ADR-004-WebUI](../specs/adr/004-webui-task-and-state-model.md) | WebUI Task and State Model | **Accepted** | Tasks and state are first-class UI objects. Every mutation creates a task. Defines task states and resource health states. |
| [ADR-005-WebUI](../specs/adr/005-webui-design-system-direction.md) | WebUI Design System Direction | **Accepted** | Modern but restrained, enterprise-serious, light-mode first, high information density, border-first surfaces, strong typography. Avoids copying Proxmox/Xen Orchestra visually. |

## Lifecycle

```
PROPOSED → ACCEPTED → (SUPERSEDED by ADR-XXX or DEPRECATED)
```

Do not delete old ADRs. When a decision changes, write a new ADR that references and supersedes the old one.

## Contributing

When making a significant architectural decision:

1. Write a new ADR following the template in [`documentation-and-adrs`](../.agents/skills/documentation-and-adrs/SKILL.md)
2. Store it in `docs/specs/adr/` with sequential numbering
3. Update this index
4. If the decision changes an existing ADR, mark the old one as `Superseded by ADR-XXX`
