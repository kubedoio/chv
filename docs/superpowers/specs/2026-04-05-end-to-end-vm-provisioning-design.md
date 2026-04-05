# End-to-End VM Provisioning Design

**Date:** 2026-04-05  
**Status:** Approved  
**Scope:** MVP-1 Minimal Viable Path

## 1. Overview

This design enables CHV to provision and run actual VMs through the full controller→agent→hypervisor stack. It focuses on the critical path components needed to boot a VM with networking and cloud-init configuration.

### Key Decisions

- **Pre-placed images:** Images must be manually placed on nodes (no download in this iteration)
- **Linux bridge + TAP:** Simple L2 networking without overlay complexity
- **Cloud-init ISO:** Standard nocloud datasource for VM initialization
- **Process-per-VM:** Each VM runs as a separate cloud-hypervisor process

## 2. Architecture

```
┌─────────────┐    gRPC    ┌─────────────┐    exec    ┌─────────────────┐
│ Controller  │ ──────────▶ │   Agent     │ ──────────▶ │ Cloud Hypervisor │
│  reconcile  │             │  (per node) │             │    (per VM)      │
└─────────────┘             └─────────────┘             └─────────────────┘
       │                           │                            │
       │                    ┌──────┴──────┐                     │
       │                    ▼             ▼                     │
       │              ┌─────────┐   ┌─────────┐                │
       │              │  TAP    │   │  ISO    │                │
       │              │ device  │   │  disk   │                │
       │              └────┬────┘   └────┬────┘                │
       │                   │             │                      │
       └───────────────────┴─────────────┴──────────────────────┘
                          Linux Bridge
```

## 3. Components

### 3.1 Controller gRPC Client (`internal/agent/client.go`)

**Responsibilities:**
- Establish and maintain gRPC connections to agents
- Provide typed interface for agent RPCs
- Handle timeouts and retries

**Interface:**
```go
type Client interface {
    ProvisionVM(ctx context.Context, nodeID string, req *pb.ProvisionVMRequest) error
    StartVM(ctx context.Context, nodeID string, vmID string) error
    StopVM(ctx context.Context, nodeID string, vmID string) error
    GetVMState(ctx context.Context, nodeID string, vmID string) (*pb.VMStateResponse, error)
}
```

**Connection Management:**
- Map of nodeID -> *grpc.ClientConn
- Lazy connection establishment
- Connection pooling per node

### 3.2 Hypervisor Launcher (`internal/hypervisor/launcher.go`)

**Responsibilities:**
- Build cloud-hypervisor command-line arguments
- Spawn and manage VM processes
- Track process state

**Command-Line Builder:**
```bash
cloud-hypervisor \
  --cpus boot=<vcpu> \
  --memory size=<memory_mb>M \
  --disk path=<volume_path> \
  --disk path=<cloudinit_iso> \
  --net tap=<tap_name>,mac=<mac> \
  --api-socket <api_socket_path> \
  --console off \
  --serial tty
```

**Process Management:**
- Start: exec.Command with logging
- Stop: SIGTERM, then SIGKILL after timeout
- Query: check /proc/<pid> existence

### 3.3 TAP Manager (`internal/network/tap.go`)

**Responsibilities:**
- Create TAP devices
- Attach to bridge
- Cleanup on VM stop

**Operations:**
```bash
# Create
ip tuntap add <tap_name> mode tap
ip link set <tap_name> master <bridge_name>
ip link set <tap_name> up

# Delete
ip tuntap del <tap_name> mode tap
```

**Naming:** `tap<vm_short_id>` (first 8 chars of VM UUID)

### 3.4 Cloud-Init ISO (`internal/cloudinit/iso.go`)

**Responsibilities:**
- Generate valid ISO9660 image
- Include nocloud datasource files

**Implementation:**
- Use `xorrisofs` (preferred) or `mkisofs`
- Label as "cidata"
- Files: user-data, meta-data, network-config

```bash
xorrisofs -input-charset utf-8 \
  -o <output.iso> \
  -V cidata \
  -J -R \
  <source_dir>
```

### 3.5 Agent Service Updates (`cmd/chv-agent/service.go`)

**Integration Points:**
- Use Launcher for VM lifecycle
- Use TAPManager for networking
- Use ISO generator for cloud-init

**State Tracking:**
```go
type VMInstance struct {
    VMID       string
    PID        int
    TAPDevice  string
    APISocket  string
    State      string
}

var runningVMs map[string]*VMInstance
```

## 4. Data Flow

### 4.1 VM Creation Flow

```
1. POST /api/v1/vms
   ↓
2. Controller creates VM record (desired_state=running)
   ↓
3. Scheduler assigns node_id
   ↓
4. Reconciler calls agent.ProvisionVM()
   ↓
5. Agent:
   a. Create volume (raw disk from image backing)
   b. Generate cloud-init ISO
   c. Create TAP device on bridge
   d. Return success
   ↓
6. Reconciler calls agent.StartVM()
   ↓
7. Agent:
   a. Build cloud-hypervisor args
   b. Spawn process
   c. Store PID in map
   d. Return running state
   ↓
8. Controller updates VM actual_state=running
```

### 4.2 VM Stop Flow

```
1. POST /api/v1/vms/{id}/stop
   ↓
2. Controller sets desired_state=stopped
   ↓
3. Reconciler calls agent.StopVM()
   ↓
4. Agent:
   a. Send SIGTERM to cloud-hypervisor process
   b. Wait for graceful shutdown (timeout 30s)
   c. SIGKILL if needed
   d. Cleanup TAP device
   e. Return stopped state
   ↓
5. Controller updates VM actual_state=stopped
```

### 4.3 State Reconciliation

- Poll interval: 30 seconds
- For each VM with `desired != actual`:
  - Call `GetVMState()` to verify actual state
  - Reconcile differences
- Handle agent disconnections gracefully

## 5. Error Handling

### 5.1 Error Categories

| Category | Examples | Handling |
|----------|----------|----------|
| **Transient** | Network timeout, agent temporarily unavailable | Retry with exponential backoff (max 5 retries) |
| **Resource** | Bridge not found, KVM unavailable | Fail fast with clear error message, no retry |
| **Validation** | Invalid VM spec, image not found | Fail fast, return 400 to API client |
| **Process** | Cloud-hypervisor crash, disk full | Set VM state to error, log details, alert |

### 5.2 Retry Logic

- Retries only for gRPC timeouts/unavailable
- Exponential backoff: 1s, 2s, 4s, 8s, 16s
- Max retry window: 60 seconds per operation
- Idempotency: operations must be safe to retry

### 5.3 Cleanup on Failure

If `ProvisionVM` fails after partial setup:
- Rollback created TAP devices
- Remove cloud-init ISO
- Keep volume (may be reused)
- Log failure details for debugging

### 5.4 Process Monitoring

- Agent tracks child PIDs in memory
- On agent restart: scan /proc for cloud-hypervisor processes
- Rebuild VM state map from process list + API sockets

## 6. File Layout

```
internal/
├── agent/
│   └── client.go           # gRPC client for controller→agent
├── hypervisor/
│   ├── launcher.go         # Cloud-hypervisor process management
│   └── launcher_test.go    # Unit tests with mock binary
├── network/
│   ├── tap.go              # TAP device management
│   └── bridge.go           # Bridge utilities
└── cloudinit/
    └── iso.go              # ISO generation via xorrisofs

cmd/chv-agent/
├── main.go                 # (existing)
└── service.go              # (updated with real implementations)
```

## 7. Testing Strategy

### 7.1 Unit Tests

- Mock `cloud-hypervisor` binary (shell script that validates args)
- Mock TAP operations in tests
- Test command-line builder logic

### 7.2 Integration Tests

- Test full flow with minimal VM (tiny Linux image)
- Verify network connectivity after boot
- Test stop/start cycles

### 7.3 Manual Testing Steps

```bash
# 1. Place a raw cloud image
cp ubuntu-22.04.raw /var/lib/chv/images/test-image.raw

# 2. Create VM via API
curl -X POST http://localhost:8080/api/v1/vms ...

# 3. Verify VM process
ps aux | grep cloud-hypervisor

# 4. Verify TAP device
ip link show tap0

# 5. Verify bridge attachment
bridge link show

# 6. Verify cloud-init ISO
ls /var/lib/chv/volumes/<vm-id>-cloudinit.iso
```

## 8. Dependencies

### Required Tools

- `cloud-hypervisor` binary on agents
- `xorrisofs` or `mkisofs` for ISO generation
- `ip` command (iproute2) for network management

### Go Dependencies

- `google.golang.org/grpc` (already present)
- No additional external dependencies

## 9. Security Considerations

- Agent runs as root (required for TAP device creation)
- Controller validates all VM specs before sending to agent
- Cloud-init user-data is validated for size limits
- API socket paths are restricted to agent owner

## 10. Future Work (Out of Scope)

- Image download and conversion
- Live migration
- VXLAN/overlay networking
- Distributed storage
- GPU/VFIO support
