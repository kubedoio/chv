# CellHV O3K Compatibility Source Audit

**Audit date:** 2026-07-21  
**Evidence tier:** T0 source and unit-test inspection  
**Repository:** `https://github.com/kubedoio/o3k`  
**Revision:** `53fd2cb36ee79f42da49c8181d6ceed12b41b3aa`

O3K is an OpenStack-compatible control plane, not upstream Nova. This audit
therefore supplements CellHV API-adapter discovery; it does not satisfy the
DevStack/Nova requirements of OSD-001 or runtime OSD-002.

## Reusable compatibility surface

- `internal/nova/handlers.go:33-52` defines the Nova-compatible service and
  `RegisterRoutes` at `:140-339` exposes its HTTP surface.
- `CreateServer` (`:453`), `DeleteServer` (`:1743`), and `ServerAction`
  (`:1868`) are candidate above-Core compatibility entry points.
- `cmd/o3k/main.go:954-1122` constructs Nova, Neutron, Cinder, Glance, and
  Placement API servers independently.
- Contract suites under `test/contract/nova`, `test/contract/neutron`, and
  `test/contract/cinder` can become client/API conformance inputs after an O3K
  adapter routes mutations through the public Core API.

Placement is not capacity evidence: `internal/placement/placement.go:14-27`
registers endpoints, while resource providers, resource classes, and traits
return empty arrays at `:67-85`.

## Authority conflicts requiring replacement

O3K in its current real mode cannot be connected directly to CellHV:

- `internal/nova/handlers.go:586-610` creates VM identity and BUILD state in
  the O3K database. The same create path either inserts an O3K `VM_CREATE`
  task (`:639-669`) or prepares networking, disks, XML, and calls its VM
  manager directly (`:671-837`).
- `internal/scheduler/worker.go:48-91`, `internal/tunnel/server.go:183`, and
  `internal/tunnel/task.go:9-36` form another operation dispatch system.
- `internal/tunnel/executor.go:20-172` creates another VM lifecycle executor
  for create, delete, start, stop, and reboot.
- `pkg/hypervisor/libvirt.go:48-600` directly defines and controls libvirt
  domains. `connectLibvirt` at `:66-83` hardcodes
  `/var/run/libvirt/libvirt-sock` regardless of the configured URI.

Reusing those paths would create a second VM database, operation engine,
process authority, and lifecycle owner, violating ADR-016. A supported O3K
integration must retain O3K only as an above-Core projection/client and make
`chv-agent` the sole accepted-state and operation authority.

## QEMU-specific assumptions

- `pkg/hypervisor/xml_template.go:93` emits domain type `kvm`.
- The same generator selects `/usr/bin/qemu-system-x86_64` at `:126`, QEMU
  disk drivers at `:139`, `:149`, and `:165`, and an IDE cloud-init device at
  `:226-230`.
- `GenerateDiskXML` repeats QEMU driver assumptions at `:361-385`.
- `internal/tunnel/executor.go:20` constructs its manager with
  `qemu:///system`.
- `deployments/docker-compose-kvm.yml:1-15` grants privilege and mounts the
  libvirt socket.

These paths are evidence of work to retire, not a basis for presenting Cloud
Hypervisor as QEMU. An O3K adapter must not mount the libvirt socket, generate
hypervisor XML, access Cloud Hypervisor sockets, or maintain a VM-operation
journal.

## Network and storage separation

The source already exposes separable translation boundaries:

- Nova allocates and binds ports at `internal/nova/handlers.go:694-755`;
  interface attach/detach is in `internal/nova/interface_attach.go:31-235`;
  Neutron API/state is under `internal/neutron`, and host mutation is under
  `pkg/networking`.
- Image/disk preparation occurs at `internal/nova/handlers.go:492-529` and
  `:792-813`; volume attach/detach at
  `internal/nova/volume_attachment.go:38-235` currently generates XML and
  calls the VM manager; Cinder and provider implementations are under
  `internal/cinder` and `pkg/storage`.

A future adapter may translate one qualified O3K port model and one qualified
volume model into public Core attachment references. It must not call
`chv-nwd`, `chv-stord`, or their databases directly.

## Compatibility contradictions

- `go.mod:1` still identifies `github.com/cobaltcore-dev/o3k`; consumers must
  pin the repository commit rather than infer module provenance.
- `internal/nova/handlers.go:54-57` exposes Nova microversions 2.1 through
  2.93, while compatibility documentation reports a newer range. Source is
  authoritative for this audit.
- `test/libvirt_smoke_test.sh` targets QEMU/libvirt and proves only domain
  presence/removal. The self-hosted workflow is not a required CellHV gate.

## Disposition

O3K cannot replace the required upstream Nova/DevStack discovery. It is a
fourth, distinct integration candidate: an OpenStack-compatible O3K adapter
using the public native Core API. Selecting it requires a focused ADR because
ADR-015 currently compares only generic libvirt, generic upstream changes, and
a native Nova driver.

The viable reuse is its external API surface and contract fixtures. Its
hypervisor manager, VM task scheduler, tunnel executor, direct networking and
storage mutation, and authoritative VM state must be bypassed or retired for
CellHV-managed workloads.
