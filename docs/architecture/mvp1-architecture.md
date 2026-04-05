# Project CHV — MVP-1 Architecture

## 1. High-Level Architecture

Project CHV consists of:
- central controller
- PostgreSQL database
- node-local agents
- host-native Cloud Hypervisor VM processes
- privileged bootstrap installer container

### Control plane
The controller exposes the public API, stores desired state, reconciles actual state, and schedules VM placement.

### Data plane
Each node runs `chv-agent`, which manages:
- bridge preparation
- image import
- volume creation
- VM launch
- VM lifecycle operations
- actual-state reporting

### Hypervisor execution
Each VM is executed as a host-native Cloud Hypervisor process.

---

## 2. Main Components

### Controller
Responsibilities:
- API handling
- auth
- scheduler
- reconciliation
- operation tracking
- state transitions
- node inventory
- maintenance mode
- storage/network validation

### PostgreSQL
Stores:
- nodes
- networks
- storage pools
- images
- virtual machines
- volumes
- operations
- api tokens

### Agent
Responsibilities:
- host validation
- bridge existence validation
- image import/normalization
- raw volume creation
- cloud-init disk generation
- VM launch/stop/delete
- runtime state observation
- resize operations
- node heartbeat

### Bootstrap Installer Container
Responsibilities:
- validate host
- install binaries
- install systemd units
- prepare directories
- prepare bridges
- write config
- perform upgrades

---

## 3. Provisioning Flow

1. Operator imports a cloud image.
2. Controller stores image metadata.
3. Agent downloads/imports source image and normalizes it to raw.
4. Operator submits VM create request.
5. Scheduler selects a compatible node.
6. Controller creates VM, volume, and network attachment intent.
7. Agent creates raw runtime volume from image/template.
8. Agent generates cloud-init disk.
9. Agent ensures bridge availability and TAP attachment.
10. Agent launches Cloud Hypervisor VM.
11. Agent reports actual state.
12. Controller converges desired and actual state.

---

## 4. Online Resize Flow

1. Operator requests volume resize.
2. Controller validates:
   - runtime disk format = raw
   - pool supports resize
   - VM is on supported matrix
3. Controller creates resize operation.
4. Agent resizes volume on node/storage.
5. Agent triggers or records guest-visible disk expansion state.
6. Guest-side partition/filesystem growth is handled via supported workflow.
7. Controller records final result.

---

## 5. Maintenance / Drain Flow

1. Operator marks node as maintenance.
2. Scheduler stops placing new workloads there.
3. Controller evaluates existing workloads.
4. If migration is supported for a VM on validated matrix, controller may migrate.
5. Otherwise workloads must be stopped/evacuated according to policy.
6. Agent reports drain readiness.
7. Node enters maintenance state.

---

## 6. Failure Model

### Controller failure
- desired state remains in PostgreSQL
- agents continue reporting when controller returns
- reconciliation resumes on recovery

### Agent failure
- node heartbeats expire
- node becomes degraded/offline
- controller stops scheduling there
- actual state may become stale until agent recovers

### VM launch failure
- operation marked failed
- partial artifacts must be cleaned or explicitly marked for cleanup
- controller preserves structured reason

### Storage failure
- affected pool marked degraded/unavailable
- scheduling blocked
- existing workload state may degrade depending on pool type and operation in progress

### Network misconfiguration
- attachment state must reflect failure
- VM start must fail fast if required bridge is absent and cannot be prepared safely

---

## 7. Architectural Constraints

- no ISO workflows
- no distributed storage in MVP-1
- no advanced overlay networking in MVP-1
- no hidden migration promises
- no runtime disk format sprawl
- no controller decisions without explicit reason codes

---

## 8. Repository Layout

- `cmd/chv-controller` - Controller entry point
- `cmd/chv-agent` - Agent entry point
- `cmd/chv-bootstrap` - Bootstrap installer entry point
- `internal/api` - HTTP API handlers
- `internal/auth` - Token authentication
- `internal/models` - Domain models
- `internal/store` - Database repository layer
- `internal/reconcile` - Desired vs actual state reconciliation
- `internal/scheduler` - VM placement scheduler
- `internal/operations` - Operation tracking
- `internal/network` - Network management
- `internal/storage` - Storage management
- `internal/hypervisor` - Cloud Hypervisor abstraction
- `internal/cloudinit` - Cloud-init disk generation
- `internal/nodevalidate` - Host validation
- `internal/pb` - Protocol buffer definitions
- `pkg/errorsx` - Structured errors
- `pkg/uuidx` - UUID utilities
- `deploy/systemd` - Systemd unit files
- `deploy/bootstrap-container` - Bootstrap container files
- `docs/specs` - Product specifications
- `docs/contracts` - System contracts and protobuf
- `docs/adr` - Architecture Decision Records
- `docs/architecture` - Architecture documentation
