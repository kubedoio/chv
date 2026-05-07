# ADR-013 — Network Overlay (VXLAN + eBPF Policy)

## Status
Accepted

## Date
2026-05-07

## Context
VMs on different nodes need L2 connectivity within the same broadcast domain. Live migration requires MAC address preservation and seamless network continuity.

ADR-005 (network-service-model) established the single-node MVP: bridge, namespace, nftables, and dnsmasq. ADR-005 explicitly deferred distributed overlay to a future phase.

The current `chv-nwd` implements Linux bridge, network namespace, TAP devices, nftables rules, and dnsmasq. The current proto (TopologySpec) has network_id, tenant_id, bridge_name, namespace_name, subnet_cidr, and gateway_ip — no overlay fields.

VXLAN is the standard Linux overlay technology (kernel module, well-supported tooling). eBPF provides flexible per-packet policy enforcement without nftables rule explosion.

## Decision
- Kernel VXLAN module handles encapsulation and decapsulation (NOT eBPF for the datapath)
  - Standard form: `ip link add vxlan{VNI} type vxlan id {VNI} local {VTEP_IP} dstport 4789 nolearning`
  - Each tenant network gets a unique VNI (uint32) assigned by the control plane
  - FDB entries managed explicitly by chv-nwd (no multicast, no BUM flooding to unknown destinations)
- Control plane maintains the VTEP registry:
  - Maps: `{network_id → [(node_id, vtep_ip, mac_addresses)]}`
  - On VM placement: CP notifies all participating nodes to update their FDB entries
  - On migration: CP notifies peers to update the FDB entry for the migrated MAC
- eBPF for policy enforcement only (not datapath):
  - TC hook on TAP and bridge interfaces (egress and ingress)
  - Per-VM security groups: allow/deny rules by source/destination IP, port, and protocol
  - Per-VM rate limiting: token bucket implemented in eBPF maps
  - Traffic classification and marking for QoS
  - Programs loaded and updated by chv-nwd on control-plane instruction
- chv-nwd responsibilities extended:
  - Create and delete VXLAN interfaces
  - Manage FDB entries: `bridge fdb append/del {MAC} dev vxlan{VNI} dst {PEER_VTEP_IP}`
  - ARP suppression (optional, reduces broadcast traffic)
  - Post-migration: send gratuitous ARP within 1s of VM resuming: `arping -U -c 3 -I {bridge} {VM_IP}`
  - Load eBPF programs from filesystem (pre-compiled .o files)
  - Update eBPF maps (security rules, rate limit parameters) on policy change
- Proto changes required:
  - TopologySpec: add `vni: uint32`, `vtep_endpoints: repeated VtepEndpoint`, `overlay_type: OverlayType`
  - New message: `VtepEndpoint { string node_id; string vtep_ip; uint32 vtep_port; }`
  - New message: `SecurityPolicy { string vm_id; repeated SecurityRule rules; }`
  - New enum: `OverlayType { OVERLAY_NONE = 0; OVERLAY_VXLAN = 1; }`
  - New RPCs on the nwd service: UpdateOverlay, UpdateSecurityPolicy
- MTU: the underlay network must support at least 1550 bytes (1500 inner + 50 VXLAN header)
  - chv-nwd SHOULD set the inner bridge MTU to 1450 if the underlay is 1500
  - If the underlay supports jumbo frames (9000 bytes), the inner MTU can remain the standard 1500

## Consequences
Pros:
- Standard kernel VXLAN: well-tested, debuggable with tcpdump and wireshark
- Explicit FDB management: no unknown unicast flooding, predictable forwarding
- eBPF policy: updated without disrupting the data plane (atomic map swap)
- Clean separation: kernel handles packet forwarding, eBPF handles policy
- Migration-friendly: FDB update and gratuitous ARP provide seamless continuity

Cons:
- 50-byte MTU overhead requires either jumbo frames or a reduced inner MTU
- Explicit FDB management requires control-plane coordination (single CP must be available to update topology)
- eBPF program compilation and loading add operational complexity
- East-west traffic between VMs on the same node bypasses VXLAN (bridge-local path differs from cross-node path)
- Kernel VXLAN performance is slightly lower than an eBPF/XDP datapath (acceptable for target scale)

## Guardrails
- VNI allocation MUST be globally unique per control plane (no reuse after network deletion for 24 hours)
- FDB entries MUST be cleaned up when a VM is destroyed or migrated away
- eBPF programs MUST pass the kernel verifier before loading
- eBPF MUST NOT drop packets silently; denied packets MUST increment a counter visible in metrics
- Gratuitous ARP MUST be sent within 1s of the VM resuming on the destination after migration
- chv-nwd MUST handle VXLAN interface failure (link down) gracefully: log, report health degraded, do not crash

## Related ADRs
- **ADR-005** (network-service-model): this ADR extends the single-node model to multi-node overlay
- **ADR-012** (disk-migration): migration requires the overlay for network continuity
- **ADR-011** (single-node-CP): the VTEP registry lives in the control plane's SQLite database
- **ADR-006** (partition-policy): overlay state is cached locally; during partition, no new VNI joins occur but existing tunnels persist
