# ADR-006-WebUI Revised WebUI Navigation Model (Cloud/Hosts/Instances)

## Status
Accepted

## Date
2026-06-13

## Context
[ADR-003-WebUI](003-webui-navigation-model.md) (2026-04-15) defined the original
left-panel navigation hierarchy using platform-engineer terminology: "Datacenters /
Clusters", "Nodes", and "Virtual Machines". As implementation progressed, that
taxonomy proved poorly aligned with the operator-facing language used by adjacent
private-cloud and public-cloud products:

- "Node" is overloaded (Kubernetes node, cluster node, hardware node) and reads
  as platform-internal jargon to many operators.
- "Virtual Machines" is verbose; the surrounding ecosystem (AWS EC2, GCE,
  Proxmox community usage, OpenStack Nova) uses **Instances** as the operator-
  facing noun for a running compute unit.
- "Datacenter / Cluster" forced an artificial distinction at MVP scale; a
  single sovereign cloud rooted at "Default Cloud" matches the actual deployment
  topology and leaves room for multi-cloud later.

The implemented design captured in [`left-panel-redesign-spec.md`](../left-panel-redesign-spec.md)
adopted the revised taxonomy. The shipped UI in
[`ui/src/lib/components/shell/SidebarNav.svelte`](../../../ui/src/lib/components/shell/SidebarNav.svelte)
and its surrounding components already use Cloud / Hosts / Instances. ADR-003-WebUI
was never marked superseded, leaving the ADR record contradicting both the spec
and the code.

## Decision
The primary left-panel navigation taxonomy is:

```
INFRASTRUCTURE
  Default Cloud
    Hosts
      <host-name>
        Instances
        Networks
        Storage
        Images

GLOBAL
  Images
  Networks
  Storage Pools
  Tasks
  Events
  Backups
  Settings
```

### Rename Rules
- **Datacenter / Cluster / Default-DC** → **Default Cloud** (Infrastructure root)
- **Nodes** → **Hosts** (topology tree)
- **Virtual Machines / VMs** → **Instances** (topology tree + global)
- Other global items keep their plain operator-facing labels (Images, Networks,
  Storage Pools, Tasks, Events, Backups, Settings).

The detail-page tab pattern from ADR-003-WebUI (Summary, Configuration, Tasks,
Events, Related Resources) is **retained unchanged**; only the top-level
taxonomy and labels are revised.

This ADR **supersedes [ADR-003-WebUI](003-webui-navigation-model.md)**.

## Consequences
Pros:
- Operator-facing, cloud-native vocabulary consistent with AWS/GCE/Proxmox usage.
- ADR record now matches the implementing spec and shipped code.
- Removes "Node/VM" platform-engineer-isms that do not survive contact with operators.
- Clearer single-cloud rooting (Default Cloud) without prematurely modeling
  multi-datacenter taxonomy.

Cons:
- "Host" overlaps slightly with "host machine" terminology in some docs; needs
  consistent usage in user-facing copy.
- Backend route paths still use legacy `/nodes/{id}` and `/vms/{id}`; UI labels
  and routes are intentionally decoupled until a separate ADR addresses route
  rename cost vs. churn.

## Related ADRs
- Supersedes: [ADR-003-WebUI WebUI Navigation Model](003-webui-navigation-model.md)
- Implementing spec: [`docs/specs/left-panel-redesign-spec.md`](../left-panel-redesign-spec.md)
- Implementation: [`ui/src/lib/components/shell/SidebarNav.svelte`](../../../ui/src/lib/components/shell/SidebarNav.svelte)

## Follow-up
- [`docs/specs/spec/webui-information-architecture.md`](spec/webui-information-architecture.md)
  still references the legacy "Nodes / Virtual Machines" taxonomy and must be
  realigned with this ADR. Tracked as a separate documentation task; out of
  scope for this ADR.
