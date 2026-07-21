# ADR-018: Optional Libvirt Delegation Over the CellHV Core API

## Status

Proposed

## Date

2026-07-21

## Context

Libvirt has an upstream Cloud Hypervisor driver exposed as `ch:///system`.
That driver is stateful: it creates, launches, monitors, and stops Cloud
Hypervisor processes. Used unchanged for CellHV-managed VMs, it would become a
second lifecycle authority beside `chv-agent`, contrary to ADR-016 and ADR-017.

Some consumers may nevertheless benefit from the bounded libvirt surface in
`docs/specs/contracts/cellhv-libvirt-compatibility-profile-v1.md`. Supporting
that surface requires delegation to the public CellHV Core API, not direct use
of the upstream driver's process-control path.

This ADR does not select the first OpenStack path. ADR-015 remains evidence
driven, and a successful libvirt connection is not an OpenStack support result.

## Proposed decision

1. Libvirt compatibility, if productised, is implemented as an optional adapter
   outside `chv-agent` Core.
2. The adapter uses only the versioned public Core API. It cannot open the Core
   database, Cloud Hypervisor sockets, provider-private sockets, or privileged
   host-mutation interfaces.
3. `chv-agent` remains the sole persistent identity, mutation, operation,
   recovery, and Cloud Hypervisor process authority.
4. For the libvirt profile, only UUID-shaped Core VM identifiers are eligible,
   and the domain UUID equals that Core identifier without rewriting. Core
   identifiers remain opaque outside this profile. Driver-local state is a
   rebuildable projection, never authoritative state.
5. Every mutating libvirt call creates or reuses one durable Core operation and
   carries a stable idempotency identity. Core unavailability rejects new
   mutations without stopping existing VMs.
6. Delegated CellHV resources and the upstream direct `ch` driver's resources
   occupy disjoint, explicitly configured namespaces. They cannot manage the
   same UUID, runtime directory, socket, or process.
7. The adapter preserves truthful Cloud Hypervisor identity. It never reports
   QEMU, exposes `qemu:///system`, or emulates QMP/QEMU behavior.
8. Network and storage remain independently qualified Core attachment paths.
   A libvirt network or storage driver may prepare an endpoint only when a
   published ownership contract allows Core to consume it.
9. Unsupported XML and flags fail before a Core mutation is accepted.
10. The adapter package, process model, URI/mode selection, authentication, and
    version-skew policy require prototype validation before this ADR can be
    accepted.

This is a delegation architecture, not a libvirt dependency inside Core. Core
domain types do not include domain XML or libvirt-specific state.

## Bounded v1 surface

The proposed adapter implements only the compatibility profile's surface:

- connection open/auth/close, type and version reporting, liveness, truthful
  host and domain capabilities, basic node information, and domain listing;
- stable name and UUID, vCPU, memory, modern HVM boot, qualified firmware or
  kernel boot, raw file/block virtio disk, bridge/pre-created TAP virtio NIC,
  and serial console;
- define, undefine, start, graceful shutdown, force stop, reboot, pause, resume,
  state inspection, and observable lifecycle events;
- supported virtio disk and NIC attach/detach only after their independent
  provider profiles pass, with explicit live/config flag handling;
- only measured state, CPU, memory, block, and interface statistics; unavailable
  values are explicitly unavailable.

The v1 surface excludes live migration, snapshots, checkpoints, managed save,
QMP, QEMU monitor and guest-agent calls, block jobs, QEMU migration semantics,
CPU or memory hotplug, arbitrary host-device passthrough, a complete graphics
stack, arbitrary domain XML, arbitrary emulator paths, and platform support
claims.

## Relationship to OpenStack and O3K

OpenStack Path A using the upstream stateful `ch` driver is not acceptable for
CellHV-managed resources because it bypasses Core authority. A generic upstream
change is viable only if it creates a true delegation mode satisfying this ADR.
A native Nova `ComputeDriver` using the public Core API remains a separate
candidate and does not require libvirt delegation.

O3K is a CellHV-controlled, lightweight OpenStack-compatible control plane. It
uses the native Core API and is not evidence that Nova, libvirt, Neutron, or
Cinder works. O3K tests may prove native API and idempotency behavior, but they
cannot substitute for the real T5 OpenStack discovery and qualification gates.

## Prerequisites

Implementation must not begin until:

- Phase A OpenStack discovery records the exact real-lab `ch:///system` result;
- Phase B provides one durable store, operation engine, versioned public local
  API, idempotency, resource versions, events, and truthful capabilities;
- Phase C standalone lifecycle and recovery pass without libvirt;
- required Phase D network and storage profiles pass independently;
- a prototype documents adapter process/package ownership, URI or mode
  selection, socket authentication and authorization, namespace isolation,
  XML translation, events, restart projection, and version skew;
- named maintainers accept the downstream and upstream maintenance policy.

## Acceptance gates

Before moving this ADR to Accepted:

- ADR-015 discovery and path-selection conditions are satisfied;
- CH-001 through CH-009 pass at their required T2/T3 tiers;
- real `virsh` and selected language bindings pass the bounded profile;
- all mutations correlate to the single Core operation journal;
- agent, adapter/libvirt, and host restarts preserve UUID and projection;
- unsupported XML fails before mutation;
- tests prove the adapter cannot access VMM sockets or host mutation APIs;
- used network and storage paths have separate passing profiles;
- direct upstream `ch` behavior affected by any patch has bounded regression
  evidence;
- exact versions, unsupported features, maintenance owner, upgrade/rollback
  procedure, and evidence digest are published.

Passing these gates proves only the named libvirt profile. OpenStack support
still requires OS-001 through OS-008 on a real platform.

## Rejected alternatives

### Use upstream `ch:///system` unchanged

Rejected for CellHV-managed resources because upstream libvirt owns the Cloud
Hypervisor process and lifecycle, creating a second authority.

### Put libvirt XML and lifecycle handling inside `chv-agent`

Rejected because platform-specific compatibility code belongs outside Core and
would enlarge the trusted runtime and canonical domain unnecessarily.

### Advertise Cloud Hypervisor as QEMU

Rejected by ADR-017 because URI or protocol impersonation promises unsupported
semantics and obscures executable capabilities.

### Treat O3K tests as OpenStack qualification

Rejected because O3K and upstream OpenStack/Nova are distinct integrations with
different contracts and evidence tiers.

## Consequences

The architecture preserves one runtime authority and permits a small, auditable
compatibility surface. It also means upstream libvirt's current Cloud
Hypervisor driver is not directly reusable for CellHV-owned VMs. A maintained
delegation adapter or upstream delegation mode is additional product surface
and may be rejected after discovery if its value does not justify that cost.
