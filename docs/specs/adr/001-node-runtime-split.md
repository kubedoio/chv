# ADR-001 — Node Runtime Split

## Status
Accepted; storage ownership updated by [ADR-028](028-volvisor-storage-orchestration.md)

## Date
2026-04-13

## Context
CHV MVP-1 needs a clear split between orchestration, storage, networking, and the VMM. Earlier designs considered storage and network service VMs. For MVP-1, the priorities are simplicity first, then isolation, then performance, then operator control. The host should remain thin, but small sandboxed daemons are allowed where necessary.

## Decision
Each compute node uses this runtime split:

- `chv-agent` — sole node orchestrator and reconciler
- `volvisord` — selected node-local storage lifecycle and provider authority (target; integration pending)
- `chv-stord` — legacy host-side storage service during explicit volume-owner migration
- `chv-nwd` — host-side network daemon
- `cloud-hypervisor` — VM monitor
- minimal host helper layer — install/runtime prerequisites only

### `chv-agent`
Responsibilities:
- register and authenticate the node with the control plane
- fetch and reconcile desired state
- maintain the local durable cache
- supervise the configured storage service and `chv-nwd`
- prepare and call Cloud Hypervisor over local Unix API sockets
- manage VM lifecycle operations
- report health, inventory, and operation outcomes

Non-responsibilities:
- no storage datapath
- no network datapath
- no direct remote exposure of Cloud Hypervisor

### `volvisord` (target)

Responsibilities:
- own storage resource identity, generations, provider capabilities, provisioning, and health/capacity
- expose a versioned node-local API used only by `chv-agent`
- provide storage-side attachment fencing and host-scoped frontend handles

Non-responsibilities:
- no tenant-facing API or direct Web UI access
- no VM desired-state or Cloud Hypervisor lifecycle authority
- no production-support claim until the exact provider and CHV integration are qualified

### `chv-stord` (legacy compatibility path)

Responsibilities:
- serve and report volumes explicitly assigned to the legacy storage owner
- preserve existing open/close, migration, policy, and backend behavior during transition
- never independently mutate a volume owned by Volvisor

Non-responsibilities:
- no control-plane client role
- no VM lifecycle management
- no new storage-class or placement authority alongside Volvisor

### `chv-nwd`
Responsibilities:
- tenant bridge management
- Linux network namespace management
- routing
- NAT
- DHCP
- DNS
- basic LB/public exposure functions
- firewall rule application

Non-responsibilities:
- no network service VM in MVP-1
- no full SDN controller
- no eBPF datapath in MVP-1

### Host helper layer
Responsibilities:
- install-time preparation
- package/tool verification
- directory and permission layout
- service unit installation
- emergency/manual recovery entrypoints

Non-responsibilities:
- must not grow into platform logic

## Consequences
Pros:
- simpler than service-VM model
- fewer moving parts
- lower datapath overhead
- clean Cloud Hypervisor fit
- easier local recovery

Cons:
- weaker compartmentalization than service-VM model
- stronger host hardening requirements
- strict daemon boundary discipline is required

## Notes
This ADR does not permit collapsing the storage provider or `chv-nwd` into
`chv-agent`. The storage lifecycle ownership change and transition rules are
defined by ADR-028.

## Related ADRs
- **ADR-028** selects Volvisor as the storage lifecycle/provider authority and
  defines the legacy transition boundary.
- **ADR-005** defers the eBPF datapath and network-VM approach for MVP-1, consistent with the host-daemon model chosen here.
- **ADR-007** defines upgrade policy for these daemons. The compatibility
  matrix must include Volvisor once its node service is integrated; legacy
  `chv-stord` and `chv-nwd` policies apply during transition.
