# Project CHV — MVP-1 Product Specification

## 1. Product Definition

Project CHV is a Linux-first, cloud-image-first virtualization platform for sovereign private cloud and edge cloud environments.

It is not a general-purpose legacy virtualization platform in MVP-1.
It is not an ISO-driven VM installer platform in MVP-1.
It is not a broad Windows-first platform in MVP-1.

MVP-1 is designed for:
- modern Linux guest workloads
- cloud images
- API-first provisioning
- tightly validated host/image/storage combinations
- small-to-medium private cloud and edge cloud deployments

## 2. Product Thesis

CHV aims to provide a modern VM platform built on Cloud Hypervisor with a smaller and cleaner operational surface than legacy-heavy virtualization stacks.

The value proposition is:
- modern workload assumptions
- cloud-image-first lifecycle
- simple and explainable architecture
- explicit compatibility matrix
- API-driven control plane
- strong fit for sovereign/private/edge environments

## 3. MVP-1 Scope

### Included
- Linux guests only
- Cloud Hypervisor as the VMM
- Central controller + per-node agent model
- PostgreSQL control-plane database
- Opaque API tokens stored as SHA-256 hashes
- Linux bridge networking
- Local storage and NFS storage
- Cloud-image-based provisioning
- Cloud-init user-data support
- Raw runtime disks
- VM lifecycle:
  - create
  - start
  - stop
  - reboot
  - delete
  - inspect
- Basic online disk resize for supported runtime disk types
- Serial console / console access path
- Node inventory and heartbeats
- Scheduler with explicit placement rules
- Maintenance / drain mode
- Desired vs actual state reconciliation

### Explicit Non-Goals
- ISO installation
- Appliance-style guest support
- Windows guest support
- GPU / VFIO
- Distributed block storage
- DRBD-based hyperconverged storage
- eBPF/VXLAN multi-tenant networking in MVP-1
- Advanced SDN
- Full firewall product
- Broad live migration promise without validated matrix
- LXC / containers as workload type

## 4. Installation Model

### Node installation
MVP-1 uses a privileged bootstrap container for installation and upgrades.

The privileged bootstrap container is responsible for:
- validating host prerequisites
- installing CHV binaries and configuration
- writing systemd units
- preparing directories and permissions
- preparing Linux bridges
- validating KVM, networking, storage, and runtime dependencies

### Node runtime
The long-lived node runtime is host-native:
- `chv-agent` runs as a systemd service on the host
- `cloud-hypervisor` runs as host-native VM processes launched by the agent

### Controller runtime
The controller may run in containers.

## 5. Workload Model

### Supported guest assumptions
- Linux cloud images
- cloud-init initialization
- virtio devices
- modern kernels
- UEFI and direct-kernel boot may exist in the platform, but cloud-image boot is the normal provisioning path in MVP-1

### Unsupported guest assumptions
- legacy BIOS-only flows
- manual ISO installer flows
- opaque appliance guests
- GPU/VFIO-dependent workloads

## 6. Storage Model

### Supported storage classes
- local
- NFS

### Runtime disk strategy
- imported cloud images may originate as qcow2
- runtime-attached VM disks in MVP-1 are raw
- templates may be stored in source format and normalized during import
- online resize support is designed around runtime raw disks first

### Storage semantics to enforce
- every runtime disk belongs to exactly one storage pool
- the control plane is the source of truth for attachment intent
- the agent is the source of truth for node-local execution state
- snapshots, clone, backup, and export must be separately defined and never conflated

## 7. Networking Model

### MVP-1 networking
- Linux bridges on the host
- TAP-based VM attachment
- simple L2/L3 host networking model
- explicit IPAM ownership in control plane
- no overlay networking requirement in MVP-1

### Deferred networking
- VXLAN overlays
- eBPF datapath
- tenant router/firewall micro-VMs
- advanced policy enforcement

## 8. Migration Positioning

MVP-1 must not market live migration as a universal feature.

Migration, if exposed in MVP-1 or MVP-1.x, must be:
- limited to a validated matrix
- explicitly tied to supported host/storage combinations
- denied automatically when prerequisites are not met

## 9. Target Users

Primary target users:
- sovereign private cloud operators
- edge cloud operators
- infrastructure teams running Linux-first service workloads
- teams that want a more modern and narrower VM platform than legacy-heavy stacks

## 10. Acceptance Criteria for MVP-1

MVP-1 is acceptable when all of the following are true:
- a Linux cloud image can be provisioned via API onto a validated host
- networking works via Linux bridge without manual host edits after install
- local and NFS pools can host runtime disks
- runtime disks can be resized online for supported disk/filesystem combinations
- controller and agent converge desired and actual VM state
- placement failures are explainable
- maintenance/drain mode works
- operators can inspect VM/node state and recover from common failures
- the supported matrix is documented and enforced
