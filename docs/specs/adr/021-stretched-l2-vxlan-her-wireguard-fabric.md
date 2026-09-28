# ADR-021 — Stretched-L2 Fabric: VXLAN Head-End Replication over WireGuard Underlay

## Status
Accepted

## Date
2026-09-28

## Context

Issue #270 requires that CHV tenant networks behave as a **literal shared L2 segment (one VLAN) across all hypervisor nodes**, including distant locations behind untrusted networks. The acceptance behavior is concrete: two VMs attached to the same network must ping each other **with ordinary ARP resolution** (guest ARPs, learns the peer's real MAC, unicast ICMP flows) regardless of whether they sit on the same hypervisor or on different hypervisors — indistinguishable from a physical switch.

ADR-013 chose kernel VXLAN with `nolearning` and explicitly CP-managed FDB entries, and declared encryption a non-goal. That design is **known-unicast only**: it cannot carry ARP broadcasts, DHCP discovery, or unknown-unicast across nodes, so it does not satisfy the stretched-VLAN requirement. It is also unencrypted on the wire.

The o3k project (o3kio/o3k, P11 edge fabric) was analyzed as a candidate. Its fabric — Geneve (per-realm VNI) encapsulated inside WireGuard — has excellent host authentication, encryption, and control-plane discipline, but it **deliberately does not extend Ethernet**: ARP is proxied with synthetic MACs, and BUM traffic never crosses hosts (SPEC-0029 "No cross-host flood contract"). Adopting it verbatim would not meet the requirement.

CHV implementation status (relevant to cost): the VXLAN/FDB/VNI-allocation code in `chv-nwd` and the control plane exists but is operationally inert (VTEP IP never configured, VNI allocation never invoked, overlay RPCs fail-closed under core authority). Replacing the overlay semantics now is cheap; deferring it is not.

## Decision

CHV adopts a **stretched-L2 fabric**: kernel VXLAN with **head-end replication (HER)** over an encrypted **WireGuard underlay mesh**, with the control-plane and key-management discipline borrowed from the o3k fabric design. The fabric is implemented **inside `chv-nwd`** (extending the `NetworkExecutor` seam); `chv-nwd` is not replaced. The kernel-realization layer is **not written from scratch**: `chv-nwd` consumes the shared Kubedo fabric provider codebase ([`o3kio/fabric`](https://github.com/o3kio/fabric) — `fabric-plan` plan types, `fabric-linux` provider, `fabric-conformance` suite, governed by its `contracts/fabric-provider-v1.md`), so CHV and o3k realize the same datapath with the same invariants and the same conformance gate. CHV's canonical models, control plane, and proto surface remain its own.

### 1. Datapath — VXLAN with head-end replication

- Kernel VXLAN remains the overlay encapsulation (UDP/4789), one VXLAN interface per tenant network, enslaved to the network's bridge.
- Each tenant network keeps a unique VNI allocated by the control plane (existing `vni_allocations` store, 24-hour no-reuse rule).
- **BUM delivery via static flood entries**: for every enrolled peer VTEP, `chv-nwd` programs
  `bridge fdb replace 00:00:00:00:00:00 dev vxlan{VNI} dst <peer_vtep_ip>`
  so broadcast, unknown-unicast, and multicast frames are replicated once per peer (head-end replication). No multicast underlay, no external EVPN/BGP.
- **Kernel MAC learning stays enabled** (`nolearning` is dropped): after an ARP exchange crosses the fabric, subsequent frames are unicast to the owning VTEP via the learned FDB entry. This is what makes cross-hypervisor ARP/ping behave exactly like same-hypervisor ARP/ping: the guest's ARP request is flooded to all peers, the remote guest replies with its **real MAC**, and both sides populate their ARP tables with real addresses.
- Per-VM unicast FDB entries may still be pushed by the control plane on placement/migration as an optimization, but correctness must never depend on them (the flood path is the fallback).
- One dnsmasq instance per network serves DHCP/DNS for **all** sites: DHCP broadcast traverses the fabric like any L2 broadcast.

### 2. Underlay — WireGuard mesh

- One WireGuard interface per host (`chv-wg`), created inside a dedicated **fabric network namespace** (`chv-fabric`), reachable from the host namespace via a veth pair plus MASQUERADE/DNAT rules (pattern proven in the o3k fabric provider — **this underlay mechanism was later falsified and replaced; see the Postmortem addendum below**). Interface names are generated deterministically by the shared provider from the configured `chv` name prefix.
- One keypair per host. The private key is provisioned at **node enrollment** (delivered over the existing mTLS enrollment channel), stored mode 0600 under the nwd state root, never logged, never present in plans, protocol messages, or CP state, and survives fabric teardown so peer public keys stay valid.
- Peer configuration: `wg set chv-wg peer <pubkey> endpoint <underlay_ip:port> allowed-ips <peer_fabric_ip>/32`. AllowedIPs carry **only the peer's fabric transport address** — never tenant prefixes. The control plane distributes peer identities (`host_id`, `public_key`, `underlay_endpoint`, `fabric_ip`, MTU) as part of the VTEP registry.
- WireGuard listen port: configurable, default **65001** (the shared Kubedo fabric convention, matching the o3k fabric and the `o3kio/fabric` provider default); a port conflict fails closed (no random fallback); peers consume the advertised endpoint, never assume the remote port equals the local one.
- The VXLAN `local`/`dst` addresses are the peers' **fabric addresses on `chv-wg`**; tenant VXLAN traffic never appears in cleartext on the physical underlay.

### 3. MTU — layered, verified, advertised

- Overhead is computed per layer: WireGuard (60 bytes IPv4) + VXLAN (50 bytes). Default tenant MTU on a 1500-byte underlay is **1380**; with jumbo-frame underlays the tenant MTU is raised accordingly (formula: `tenant_mtu = underlay_mtu − 110`, capped at 1500).
- The agent measures the underlay MTU at enrollment; the control plane **MUST reject joining a node whose underlay MTU is smaller than the fabric's advertised tenant MTU + 110**.
- The effective tenant MTU is advertised to guests via DHCP (option 26).

### 4. Control-plane and execution discipline (adopted from the o3k fabric)

- The control plane remains the single authority for VNI allocation, the VTEP registry, flood lists, and peer membership. Overlay state is distributed only via the CP→agent→nwd path; nodes never accept overlay or FDB updates from peer nodes.
- All fabric mutations are **idempotent, ownership-fenced, and journaled before kernel mutation**: `chv-nwd` records the desired overlay state durably before creating links, verifies existing kernel objects match expected kind/VNI/endpoint (rejecting foreign state rather than adopting or deleting it), and reconciles pending changes at startup.
- Node identities carry monotonic generations; stale-generation overlay commands are rejected.
- Removal runs in reverse dependency order (FDB entries → VXLAN → bridge/netns teardown), preserving the WireGuard keypair.

### 5. eBPF

eBPF keeps the ADR-013 role: **policy only** (per-VM security groups, rate limiting) on TC hooks. It is never the overlay datapath.

## Consequences
Pros:
- True stretched VLAN: ARP, DHCP, unknown-unicast, and non-IP L2 protocols work transparently across all hypervisors; VM behavior is identical on-site and cross-site
- All inter-site traffic is authenticated and encrypted (WireGuard); no cleartext tenant traffic on the underlay
- Head-end replication needs only enrolled-peer flood entries: no multicast in the underlay, no BGP/EVPN operational weight at CHV's target scale (≤ ~20 nodes)
- Kernel VXLAN with learning + HER is a battle-tested datapath; the existing ADR-013 proto surface (`TopologySpec.vni/vtep_endpoints/overlay_type`, `UpdateOverlay`) carries over unchanged
- Control-plane discipline (idempotency, fencing, ownership) makes the fabric crash-safe and reconcile-friendly

Cons:
- Stretched L2 across WAN is inherently risky: broadcast domains span all sites, so ARP noise, chatty protocols, and unknown-unicast floods propagate globally per VNI
- MTU overhead (110 bytes) requires jumbo frames on the underlay for a 1500-byte tenant MTU
- Every enrolled host is a trusted peer for every VNI's BUM traffic: intra-VLAN privacy is that of a real VLAN (any VM in the network can observe its broadcasts); cross-tenant isolation remains VNI-enforced
- Flood fan-out is O(peers) per BUM frame per VNI; acceptable at the target scale, must be revisited beyond it
- Live migration still requires FDB re-point plus gratuitous ARP (unchanged from ADR-013)

## Guardrails
- VNI allocation MUST remain globally unique per control plane with 24-hour no-reuse
- Flood-list entries MUST only ever target VTEPs present in the CP's enrolled, mTLS-authenticated inventory; an unknown VTEP address MUST be rejected, never dialed
- The WireGuard private key MUST never appear in logs, plans, protocol messages, or CP state; key files MUST be 0600 and survive teardown
- Underlay MTU MUST be verified at enrollment; tenant MTU MUST be computed as underlay − 110 and advertised via DHCP option 26
- Fabric mutations MUST be journaled before kernel mutation and MUST be idempotent under replay
- Foreign kernel state (interfaces/FDB/WG peers not matching expected identity) MUST be rejected with a structured error, never adopted or silently deleted
- Dropped and flooded packets MUST increment visible counters (metrics), per ADR-013's observability guardrail
- The fabric MUST be per-network opt-in (`overlay_type: OVERLAY_VXLAN`), never imposed on single-node deployments
- `chv-nwd` MUST survive fabric link failure gracefully: log, report health degraded, do not crash
- Stretched-L2 networks MUST NOT be bridged into site-local physical switches (no external STP interaction)

## Non-goals
- EVPN/BGP control plane (revisit beyond ~20 nodes; the VNI/flood-list abstraction is designed so it can replace HER later without datapath changes)
- Multicast underlay or ASM/Bidir replication
- IPv6 VTEPs/fabric addresses in v1
- Per-realm overlapping tenant CIDRs on one host (the o3k AddressRealm model); CHV networks keep unique subnets per VNI
- Encryption of intra-hypervisor (bridge-local) traffic

## Related ADRs
- **ADR-013** (network-overlay-vxlan-ebpf): partially superseded — the `nolearning`/no-BUM decision and the encryption non-goal are replaced by this ADR; ADR-013's eBPF-policy role and proto surface remain authoritative
- **ADR-005** (network-service-model): fabric extends the nwd host-daemon model; `chv-nwd` is extended, never replaced
- **ADR-011** (single-node-controlplane): the VTEP registry, VNI allocations, and flood lists live in the CP's SQLite database
- **ADR-012** (disk-migration-precopy): live migration depends on this fabric for network continuity (FDB re-point + gratuitous ARP)
- Issue kubedoio/chv#270 and the o3k P11 edge-fabric analysis (SPEC-0028/0029, ADR-0168/0171/0172) informed the underlay, key-management, and execution-discipline decisions
- **Shared implementation**: [o3kio/fabric](https://github.com/o3kio/fabric) (`fabric-plan`/`fabric-linux`/`fabric-conformance`, contract `fabric-provider-v1.md`), the one provider codebase consumed by both `chv-nwd` and o3k's `o3k-network`; its normative counterparts are o3k [ADR-0186](https://github.com/o3kio/o3k/blob/main/docs/adr/ADR-0186-stretched-l2-edge-fabric-vxlan-her.md) and [SPEC-0049](https://github.com/o3kio/o3k/blob/main/docs/specs/SPEC-0049-stretched-l2-edge-fabric-v3.md)

## Postmortem addendum (2026-09-28, fabric v0.1.2): the NAT underlay was falsified

The original underlay described above — steering the WG transport through
a veth into the fabric netns with `PREROUTING DNAT` (inbound) and
`POSTROUTING MASQUERADE` (outbound) — carried two production races that
the fabric repo's privileged multi-host evidence gate root-caused:

1. **DNAT black-holes NEW inbound flows.** A WireGuard interface's UDP
   socket binds in its *creating* namespace and never follows
   `ip link set netns` (kernel-verified on 6.8: `creating_net` is
   immutable). A wg created in the root ns and moved into the fabric ns
   keeps its socket in the root ns while the DNAT rewrites every NEW
   inbound flow into the fabric ns, where nothing listens. Pairs survived
   only behind conntrack reply-tuple shields — an intermittent,
   timing-dependent dead-pair flake (≈1-in-3 in evidence loops).
2. **MASQUERADE remaps the source port under simultaneous initiation.**
   For a same-port peer pair, the outbound MASQ flow's reply tuple always
   equals the peer's inbound DNAT entry's orig tuple; conntrack requires
   global tuple uniqueness, so simultaneous initiation remaps one side's
   source port and the peer's WireGuard roams to a port the DNAT rule
   does not steer (≈1-in-10 dead pairs). No iptables formulation avoids
   this.

**Resolution (fabric v0.1.2, "design F"):** the NAT machinery is removed
entirely. The wg interface is created in the **root** namespace and then
moved into the fabric namespace — so its UDP socket (bound in the
creating namespace) lives in the root ns: outbound rides normal host
routing, inbound is delivered directly to the root-ns listener. There is
no NAT state left to race. Everything tenant-facing (VXLAN, HER, bridge,
tenant ns) remains fully namespaced; the transport socket is root-ns, as
it de-facto was in every released version. The provider verifies the
placement at runtime on every apply (a three-way `ss -uln` discriminator:
fabric-ns listener + quiet root ns → heal; listeners in both namespaces →
fail closed unattributable, nothing deleted) and migrates legacy
v0.1.0/v0.1.1 hosts via tolerant exact-spec cleanup on apply.

Full postmortem, kernel references, and migration semantics:
`o3kio/fabric` contract §3.10 + CHANGELOG at tag
[v0.1.2](https://github.com/o3kio/fabric/releases/tag/v0.1.2). Evidence
scope, honestly stated: the multi-host gate (three privileged containers
on one kernel — real handshakes, real ARP/MAC learning, encrypted-underlay
capture, 10/10 acceptance loop) proves the datapath and lifecycle; it is
not a substitute for cross-machine runs over real networks, which remain
the production gate. One-fabric-per-WG-port-per-host is a documented
design-F limitation; CHV runs one fabric per host.
