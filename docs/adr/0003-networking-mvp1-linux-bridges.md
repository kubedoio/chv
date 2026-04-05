# ADR-0003: Use Linux bridges in MVP-1

## Status
Accepted

## Context
A custom datapath with netns/veth/eBPF/VXLAN would create excessive networking complexity for MVP-1.

## Decision
MVP-1 uses Linux bridges for VM networking.
Overlay networking, eBPF datapaths, and router/firewall micro-VM patterns are deferred.

## Consequences
### Positive
- simpler packet path
- easier host-level debugging
- lower execution risk
- easier supportability

### Negative
- less advanced multi-tenant isolation in MVP-1
- delayed advanced SDN story
