# Project CHV — MVP-1 System Contracts

## 1. Contract Principles

1. The controller owns desired state.
2. The agent owns node-local execution and reports actual state.
3. The scheduler must never place a VM onto a node that does not satisfy declared requirements.
4. Storage and network capabilities must be explicit and queryable.
5. Operations must be idempotent where possible.
6. Partial failure must be represented explicitly, never hidden.

---

## 2. Core Domain Objects

### Node
Represents a hypervisor host running `chv-agent`.

Fields:
- id
- hostname
- management_ip
- status
- maintenance_mode
- total_cpu_cores
- total_ram_mb
- allocatable_cpu_cores
- allocatable_ram_mb
- labels
- capabilities
- last_heartbeat_at
- agent_version
- hypervisor_version

### Network
Represents a host-networking-backed network definition.

Fields:
- id
- name
- bridge_name
- cidr
- gateway_ip
- dns_servers
- mtu
- mode (`bridge`)
- status

### StoragePool
Represents a storage target on a node or shared storage endpoint.

Fields:
- id
- name
- pool_type (`local`, `nfs`)
- node_id (nullable for shared/global pool definitions if modeled separately)
- path_or_export
- capacity_bytes
- allocatable_bytes
- status
- supports_online_resize
- supports_clone
- supports_snapshot

### Image
Represents an imported bootable cloud image template.

Fields:
- id
- name
- os_family
- source_format (`qcow2`, `raw`)
- normalized_format (`raw`)
- architecture
- cloud_init_supported
- default_username
- checksum
- status

### VirtualMachine
Represents a desired and actual VM object.

Fields:
- id
- name
- node_id
- desired_state
- actual_state
- placement_status
- cpu
- memory_mb
- boot
- disks
- networks
- cloud_init
- console
- created_at
- updated_at
- last_error

### Volume
Represents a VM-attached runtime disk.

Fields:
- id
- vm_id
- pool_id
- format (`raw`)
- size_bytes
- backing_image_id (optional)
- path
- attachment_state
- resize_state

### APIToken
Represents an opaque machine token.

Fields:
- id
- name
- token_hash
- role_id
- expires_at
- created_at
- revoked_at

---

## 3. State Machines

### VM Desired State
- `present`
- `running`
- `stopped`
- `deleted`

### VM Actual State
- `provisioning`
- `starting`
- `running`
- `stopping`
- `stopped`
- `deleting`
- `error`
- `unknown`

### Node State
- `online`
- `degraded`
- `offline`
- `maintenance`

### Operation State
- `pending`
- `in_progress`
- `succeeded`
- `failed`
- `timed_out`
- `aborted`

---

## 4. Placement Contract

A VM may be placed onto a node only if:
- node is `online`
- node is not in `maintenance`
- node has sufficient allocatable CPU and RAM
- required network bridge(s) exist or can be prepared safely
- chosen storage pool is available and writable
- image architecture matches node architecture
- requested boot mode is supported by node capabilities
- migration requirements are satisfied if action is migration

If any condition fails, placement must be denied with structured reasons.

---

## 5. Storage Contract

### Disk format contract
- runtime-attached VM disks are `raw` in MVP-1
- imported source images may be qcow2 or raw
- qcow2 is normalized during image import unless explicitly allowed later by validated matrix

### Online resize contract
Online resize in MVP-1 is supported only when all of the following are true:
- runtime disk format is `raw`
- storage pool supports resize
- guest filesystem type is in supported matrix
- VM bus/device configuration supports runtime expansion
- guest-side partition/filesystem growth method is documented

If unsupported, the API must deny the request with a reason code.

---

## 6. Networking Contract

### MVP-1 network mode
- Linux bridge only

### VM attachment contract
- each VM NIC maps to exactly one network
- each network maps to exactly one host bridge
- TAP interface lifecycle is owned by the agent
- MAC assignment is deterministic and recorded
- IP assignment is either static-from-IPAM or explicitly delegated

### Minimum network observability
The platform must expose:
- bridge name
- tap device name
- MAC address
- assigned IP
- attachment status

---

## 7. Auth Contract

### Auth model
- Bearer opaque tokens
- tokens are generated once and only the plain token is returned at creation time
- only SHA-256 hashes are stored
- revocation must be supported
- expiry must be supported

### Human auth
Out of scope for MVP-1, but the API must not assume token auth is the permanent human auth model.

---

## 8. Reconciliation Contract

The controller reconciles desired vs actual state.

Rules:
- agent reports actual state periodically and on lifecycle events
- controller issues operations based on desired state
- failed operations are recorded with structured errors
- reconciliation must be idempotent
- unknown state must never be silently treated as healthy state

---

## 9. Error Contract

Every API and agent error must return:
- machine-readable code
- human-readable message
- object reference
- retryability flag
- optional remediation hint

Example fields:
- `code`
- `message`
- `resource_type`
- `resource_id`
- `retryable`
- `hint`

---

## 10. Minimal REST API Contract

### POST /api/v1/tokens
Create API token.

### POST /api/v1/nodes/register
Register or enroll node.

### GET /api/v1/nodes
List nodes.

### POST /api/v1/networks
Create network.

### GET /api/v1/networks
List networks.

### POST /api/v1/storage-pools
Create storage pool.

### GET /api/v1/storage-pools
List storage pools.

### POST /api/v1/images/import
Import cloud image.

### GET /api/v1/images
List images.

### POST /api/v1/vms
Create VM.

### GET /api/v1/vms/{id}
Get VM.

### POST /api/v1/vms/{id}/start
Start VM.

### POST /api/v1/vms/{id}/stop
Stop VM.

### POST /api/v1/vms/{id}/reboot
Reboot VM.

### POST /api/v1/vms/{id}/resize-disk
Resize runtime disk.

### DELETE /api/v1/vms/{id}
Delete VM.

### POST /api/v1/nodes/{id}/maintenance
Enter/leave maintenance mode.

---

## 11. Minimal Agent RPC Contract

Methods:
- `Ping`
- `ReportNodeStatus`
- `ValidateNode`
- `EnsureBridge`
- `ImportImage`
- `CreateVolume`
- `ProvisionVM`
- `StartVM`
- `StopVM`
- `RebootVM`
- `DeleteVM`
- `ResizeVolume`
- `GetVMState`
- `ListHostVMs`
- `PrepareDrain`

---

## 12. Compatibility Matrix Contract

MVP-1 must ship with a documented matrix covering:
- supported host OS versions
- supported kernels
- supported CPU architectures
- supported Cloud Hypervisor versions
- supported guest image families
- supported filesystem resize combinations
- supported storage pool types
- migration-allowed combinations, if any
