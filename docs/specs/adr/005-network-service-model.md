# ADR-005 — Network Service Model

## Status
Accepted

## Context
A network VM was considered. For MVP-1, the service-VM approach is deferred in favor of a host-daemon model because simplicity is the first priority.

## Decision
`chv-nwd` is a host-side network daemon.

## MVP-1 datapath stack
- Linux bridge
- network namespaces
- veth/tap primitives
- nftables

Explicitly deferred:
- eBPF datapath
- distributed overlay control plane
- full flow-state replication across upgrades

## Responsibilities
- bridge creation and verification
- namespace lifecycle
- routing
- NAT
- DHCP
- DNS
- firewall policy
- simple L4/L3 public exposure and LB functions

## Traffic policy
- east-west traffic should remain local when safe
- only routed, NAT, policy, or public exposure paths need higher-order handling in `chv-nwd`

## Service cardinality
- one `chv-nwd` per host in MVP-1

## Availability goal
- preserve host network topology during daemon restart or replacement
- minimize disruption during restart or upgrade
- brief disturbance to active flows may still occur unless explicit flow-state preservation exists

## Upgrade model
- drain and replace
- no in-place mutable upgrade requirement for MVP-1

## Consequences
Pros:
- simpler than a network VM
- more direct datapath
- easier local recovery and host integration

Cons:
- weaker isolation than a network VM
- stronger dependence on correct Linux networking behavior
- “no noticeable interruption” must be interpreted realistically for active flows
