# ADR-001 — Node Runtime Split

## Status
Accepted

## Context
CHV MVP-1 needs a clear split between orchestration, storage, networking, and the VMM. Earlier designs considered storage and network service VMs. For MVP-1, the priorities are simplicity first, then isolation, then performance, then operator control. The host should remain thin, but small sandboxed daemons are allowed where necessary.

## Decision
Each compute node uses this runtime split:

- `chv-agent` — sole node orchestrator and reconciler
- `chv-stord` — host-side storage daemon
- `chv-nwd` — host-side network daemon
- `cloud-hypervisor` — VM monitor
- minimal host helper layer — install/runtime prerequisites only

### `chv-agent`
Responsibilities:
- register and authenticate the node with the control plane
- fetch and reconcile desired state
- maintain the local durable cache
- supervise `chv-stord` and `chv-nwd`
- prepare and call Cloud Hypervisor over local Unix API sockets
- manage VM lifecycle operations
- report health, inventory, and operation outcomes

Non-responsibilities:
- no storage datapath
- no network datapath
- no direct remote exposure of Cloud Hypervisor

### `chv-stord`
Responsibilities:
- volume open/close
- attach/detach
- resize hooks
- snapshot/clone preparation hooks
- expose block devices to Cloud Hypervisor through an external userspace backend model
- publish storage health and metrics
- apply per-device policy

Non-responsibilities:
- no control-plane client role
- no VM lifecycle management
- no global storage scheduling or replication control in MVP-1

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
This ADR does not permit collapsing `chv-stord` or `chv-nwd` into `chv-agent`.
