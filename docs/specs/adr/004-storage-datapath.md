# ADR-004 — Storage Datapath Model

## Status
Accepted; storage lifecycle ownership superseded by [ADR-028](028-volvisor-storage-orchestration.md)

## Date
2026-04-13

## Context
A storage VM plus NBD was considered and rejected for MVP-1 because it would add extra indirection, operational complexity, and performance risk. The selected model uses a host-side daemon.

## Decision
The storage fast path remains host-side and does not require a storage VM or
NBD proxy. Storage lifecycle and provider ownership are now defined by
[ADR-028](028-volvisor-storage-orchestration.md): Volvisor is the selected
storage authority, to be integrated through `chv-agent`. Until that integration
and the explicit volume-owner transition ship, `chv-stord` remains the current
storage service. It will remain the compatibility service for volumes assigned
to the legacy path during migration.

## Storage classes in MVP-1
This list describes the original CHV storage contract; provider lifecycle
ownership is now governed by ADR-028. The current implementation serves these
through `chv-stord`; Volvisor-backed support remains pending qualification.

Mandatory:
- local raw/qcow2
- local LVM/direct block
- iSCSI-backed block
- Ceph RBD

Deferred:
- NFS-backed tenant disks
- NVMe-oF
- distributed replication control
- advanced storage migration logic

## Guest-facing model
The guest sees block devices through an external userspace backend integration.
In the current implementation, `chv-stord` is the logical storage service
boundary. The target provider boundary and lifecycle ownership are defined by
ADR-028; runtime integration remains pending.

## Backend access rule
Where required, the host prepares stable backend access first. Examples:
- iSCSI login/session established at host or controlled host-integration layer
- Ceph prerequisites available on host
- local devices provisioned on host

For legacy-owned volumes, `chv-stord` owns:
- attach/open/serve
- per-device policy
- health reporting
- lifecycle hooks

## Format policy
- qcow2 for general-purpose/template-driven workloads
- raw/LVM/direct block for performance-sensitive classes

## Runtime policy
Per-device limits must be adjustable at runtime, including:
- bandwidth
- IOPS
- burst behavior where supported

## Persistence model
For the current `chv-stord` implementation, long-lived metadata belongs to the
control plane and node-local durable cache, while runtime attachment/session
state should be reconstructable. Under ADR-028, Volvisor will own provider
records and storage generations for Volvisor-owned volumes; CHV will retain
tenant-facing volume intent and references.

## Consequences
Pros:
- avoids NBD as universal dependency
- lower datapath overhead than a storage VM
- better fit for Cloud Hypervisor external block model
- easier to evolve backend adapters later

Cons:
- stronger host hardening needed
- backend integration complexity remains real
