# ADR-004 — Storage Datapath Model

## Status
Accepted

## Context
A storage VM plus NBD was considered and rejected for MVP-1 because it would add extra indirection, operational complexity, and performance risk. The selected model uses a host-side daemon.

## Decision
Storage is host-side and served by `chv-stord`.

## Storage classes in MVP-1
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
The guest sees block devices through an external userspace backend integration. `chv-stord` is the logical storage service boundary.

## Backend access rule
Where required, the host prepares stable backend access first. Examples:
- iSCSI login/session established at host or controlled host-integration layer
- Ceph prerequisites available on host
- local devices provisioned on host

Then `chv-stord` owns:
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
`chv-stord` is mostly stateless. Long-lived metadata belongs to the control plane and the node-local durable cache. Runtime attachment/session state should be reconstructable.

## Consequences
Pros:
- avoids NBD as universal dependency
- lower datapath overhead than a storage VM
- better fit for Cloud Hypervisor external block model
- easier to evolve backend adapters later

Cons:
- stronger host hardening needed
- backend integration complexity remains real
