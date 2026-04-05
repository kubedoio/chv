# Implementation Plan: End-to-End VM Provisioning

**Date:** 2026-04-05  
**Status:** Ready for Implementation  
**Estimates:** ~2-3 days of focused work

## Summary

This plan implements the full controller→agent→hypervisor VM provisioning pipeline with reliability improvements based on hypervisor best practices:
- Persistent VM state for crash recovery
- Cloud Hypervisor HTTP API for reliable operations
- Improved resource tracking

## Implementation Order

### Phase 1: Foundation (Day 1)

#### 1.1 Persistent VM State Management
**Files:**
- `internal/hypervisor/state.go` - New
- `internal/hypervisor/state_test.go` - New

**Tasks:**
- [ ] Define `VMInstanceState` struct with JSON serialization
- [ ] Implement `StateManager` with CRUD operations
- [ ] Add state directory creation and permissions
- [ ] Write unit tests for state persistence
- [ ] Handle corrupted state files gracefully

**Acceptance Criteria:**
- VM state survives agent restart
- Can recover VM list from disk
- Corrupted state files are logged and skipped

#### 1.2 Cloud Hypervisor HTTP Client
**Files:**
- `internal/hypervisor/chvclient.go` - New
- `internal/hypervisor/chvclient_test.go` - New

**Tasks:**
- [ ] Define `CHVClient` struct with unix socket HTTP client
- [ ] Implement `GetVMInfo()` - GET /api/v1/vm.info
- [ ] Implement `Shutdown()` - PUT /api/v1/vm.shutdown
- [ ] Implement `Reboot()` - PUT /api/v1/vm.reboot
- [ ] Implement `Pause()`/`Resume()` for future use
- [ ] Add timeout and error handling
- [ ] Write unit tests with mock HTTP server

**Acceptance Criteria:**
- Can query VM state via API socket
- Can initiate graceful shutdown
- Handles connection failures gracefully

---

### Phase 2: Core Components (Day 1-2)

#### 2.1 Enhanced TAP Manager
**Files:**
- `internal/network/tap.go` - Update
- `internal/network/tap_test.go` - New

**Tasks:**
- [ ] Update naming to use full VM UUID: `tap-<uuid>`
- [ ] Add `TAPManager` struct with bridge reference
- [ ] Implement `CreateTAP(vmID, bridgeName)`
- [ ] Implement `DeleteTAP(vmID)`
- [ ] Implement `GetTAPDevice(vmID)`
- [ ] Add MAC address generation (deterministic from VM ID)
- [ ] Write unit tests with mock ip commands

**Acceptance Criteria:**
- TAP devices named uniquely per VM
- MAC addresses are deterministic
- Proper cleanup on delete

#### 2.2 Cloud-Init ISO Generator
**Files:**
- `internal/cloudinit/iso.go` - Update
- `internal/cloudinit/iso_test.go` - New

**Tasks:**
- [ ] Implement `GenerateISO(vmID, config)` using `xorrisofs`
- [ ] Fallback to `mkisofs` if xorrisofs unavailable
- [ ] Validate ISO creation (check file exists and size > 0)
- [ ] Add cleanup of temporary files
- [ ] Write unit tests with mock ISO tools

**Acceptance Criteria:**
- Valid ISO9660 image generated
- Label is "cidata"
- Includes all three config files
- Works with either xorrisofs or mkisofs

#### 2.3 Hypervisor Launcher
**Files:**
- `internal/hypervisor/launcher.go` - New
- `internal/hypervisor/launcher_test.go` - New
- `internal/hypervisor/cmdbuilder.go` - New

**Tasks:**
- [ ] Implement `CommandBuilder` for cloud-hypervisor args
- [ ] Implement `Launcher` struct with state management
- [ ] Implement `StartVM(spec)`:
  - Build command line
  - Create state directory
  - Spawn process
  - Wait for API socket to appear
  - Save state to disk
  - Move process to cgroup (optional/MVP+)
- [ ] Implement `StopVM(vmID)`:
  - Try API shutdown first
  - Fall back to SIGTERM
  - SIGKILL if needed
  - Cleanup TAP
  - Remove state file
- [ ] Implement `GetVMState(vmID)` using API client
- [ ] Add logging redirection
- [ ] Write unit tests

**Acceptance Criteria:**
- VM starts and API socket becomes available
- VM stops gracefully via API
- State file created and removed appropriately
- Logs captured to file

---

### Phase 3: Controller Integration (Day 2)

#### 3.1 Controller gRPC Client
**Files:**
- `internal/agent/client.go` - New
- `internal/agent/client_test.go` - New

**Tasks:**
- [ ] Define `Client` interface
- [ ] Implement `agentClient` with connection pooling
- [ ] Add lazy connection establishment
- [ ] Implement timeout configuration
- [ ] Add retry logic with exponential backoff
- [ ] Implement all RPC methods
- [ ] Write unit tests

**Acceptance Criteria:**
- Connections established on first use
- Retries work for transient failures
- Proper cleanup on close

#### 3.2 Update Agent Service
**Files:**
- `cmd/chv-agent/service.go` - Update

**Tasks:**
- [ ] Integrate `Launcher` into `AgentService`
- [ ] Update `ProvisionVM` with real implementation:
  - Create volume from backing image
  - Generate cloud-init ISO
  - Create TAP device
  - Persist state
- [ ] Update `StartVM` with real implementation:
  - Call launcher.StartVM()
  - Update state
- [ ] Update `StopVM` with real implementation:
  - Call launcher.StopVM()
  - Cleanup resources
- [ ] Update `GetVMState` with real implementation:
  - Query launcher.GetVMState()
  - Return actual status
- [ ] Add operation ID tracking for idempotency
- [ ] Update `ListHostVMs` to use state manager

**Acceptance Criteria:**
- Full VM lifecycle works end-to-end
- State persists across agent restarts
- Proper cleanup on failures

#### 3.3 Update Reconciler
**Files:**
- `internal/reconcile/service.go` - Update
- `internal/reconcile/service_test.go` - Update

**Tasks:**
- [ ] Integrate agent client
- [ ] Update `reconcileRunning` to call agent
- [ ] Update `reconcileStopped` to call agent
- [ ] Update `reconcileDeleted` to call agent
- [ ] Add proper error handling with retry
- [ ] Add operation context with timeouts
- [ ] Update tests with mock agent client

**Acceptance Criteria:**
- Reconciler calls agent for state changes
- Retries on transient failures
- Updates DB with actual state from agent

---

### Phase 4: Integration & Testing (Day 2-3)

#### 4.1 Wire Everything Together
**Files:**
- `cmd/chv-controller/main.go` - Update
- `cmd/chv-agent/main.go` - Update

**Tasks:**
- [ ] Initialize agent client in controller
- [ ] Pass agent client to reconciler
- [ ] Initialize launcher in agent
- [ ] Add configuration for paths and timeouts
- [ ] Test full flow manually

#### 4.2 Testing
**Tasks:**
- [ ] Create mock cloud-hypervisor script for testing
- [ ] Run unit test suite
- [ ] Manual test: Create VM via API
- [ ] Manual test: Verify process starts
- [ ] Manual test: Verify TAP device created
- [ ] Manual test: Verify ISO generated
- [ ] Manual test: Stop VM
- [ ] Manual test: Verify cleanup
- [ ] Test agent restart recovery

---

## File Inventory

### New Files (13)
```
internal/
├── agent/
│   ├── client.go           # gRPC client for controller→agent
│   └── client_test.go
├── hypervisor/
│   ├── state.go            # Persistent VM state management
│   ├── state_test.go
│   ├── chvclient.go        # Cloud Hypervisor HTTP API client
│   ├── chvclient_test.go
│   ├── cmdbuilder.go       # Command-line builder
│   ├── launcher.go         # VM lifecycle management
│   └── launcher_test.go
└── network/
    └── tap_test.go         # TAP manager tests

cmd/chv-agent/
└── service.go              # Significant updates

internal/reconcile/
└── service_test.go         # Update for agent client
```

### Modified Files (3)
```
internal/
├── network/
│   └── tap.go              # Enhanced implementation
├── cloudinit/
│   └── iso.go              # Real ISO generation
└── reconcile/
    └── service.go          # Integrate agent client

cmd/chv-controller/
└── main.go                 # Initialize agent client
```

## Dependencies

### System Dependencies
```bash
# Required
cloud-hypervisor          # VMM binary
xorrisofs || mkisofs      # ISO generation
iproute2                  # ip command for networking

# Optional (for cgroups v2, deferred to post-MVP)
# cgroup-tools
```

### Go Dependencies
```go
// Already present
google.golang.org/grpc

// No new external dependencies needed
```

## Configuration Changes

### Agent Config (`/etc/chv/agent.yaml`)
```yaml
node_id: "..."
controller_addr: "..."
data_dir: "/var/lib/chv"
cloud_hypervisor_path: "/usr/local/bin/cloud-hypervisor"

# New fields
vm:
  state_dir: "/var/lib/chv/instances"
  log_dir: "/var/lib/chv/logs"
  api_socket_dir: "/var/lib/chv/sockets"
  shutdown_timeout: "30s"
  
network:
  bridge_name: "br0"
  tap_prefix: "tap"
```

## Testing Checklist

### Unit Tests
- [ ] State persistence (CRUD operations)
- [ ] CHV HTTP client (mock server)
- [ ] TAP manager (mock commands)
- [ ] Command builder (arg validation)
- [ ] Agent client (mock gRPC)
- [ ] Launcher (mock processes)

### Integration Tests
- [ ] Full VM create flow
- [ ] VM stop flow
- [ ] VM restart flow
- [ ] Agent restart recovery
- [ ] Concurrent VM operations

### Manual Verification
```bash
# 1. Setup
make docker-up

# 2. Prepare test image
docker compose exec agent mkdir -p /var/lib/chv/images
docker compose exec agent \
  dd if=/dev/zero of=/var/lib/chv/images/test.raw bs=1M count=100

# 3. Create token and register node
# (as per README quickstart)

# 4. Create network and pool
# (as per README quickstart)

# 5. Import image (manual placement)
# Update image status to "ready" in DB

# 6. Create VM
curl -X POST http://localhost:8080/api/v1/vms \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "name": "test-vm",
    "cpu": 1,
    "memory_mb": 512,
    "image_id": "...",
    "disk_size_bytes": 1073741824
  }'

# 7. Verify
# - VM process running: ps aux | grep cloud-hypervisor
# - TAP device exists: ip link show tap-<uuid>
# - Bridge has TAP: bridge link show
# - ISO exists: ls /var/lib/chv/volumes/*-cloudinit.iso
# - State file exists: ls /var/lib/chv/instances/<vm-id>/

# 8. Stop VM
curl -X POST http://localhost:8080/api/v1/vms/<id>/stop \
  -H "Authorization: Bearer $TOKEN"

# 9. Verify cleanup
# - Process stopped
# - TAP removed
# - State file removed
# - ISO removed (optional)
```

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Cloud Hypervisor API changes | Document version compatibility; use minimal API surface |
| TAP device leaks | Implement defer cleanup; periodic reconciliation |
| State file corruption | JSON validation; backup/restore; graceful degradation |
| Process zombie | Use API socket for health checks; prctl death signal |
| Concurrent operations | Operation ID deduplication; state file locking |

## Rollback Plan

If issues arise:
1. Revert agent service to simulation mode
2. Keep controller and reconciler changes (they're backward compatible)
3. Document feature flag for real vs simulated VM operations

## Success Criteria

✅ VM can be created via API and boots successfully  
✅ VM state persists across agent restarts  
✅ TAP devices are created and attached to bridge  
✅ Cloud-init ISO is generated and mounted  
✅ VM can be stopped gracefully  
✅ All resources cleaned up on delete  
✅ Unit tests pass  
✅ Manual integration test passes  

## Post-MVP Enhancements

- [ ] Cgroup resource management
- [ ] Image download and conversion
- [ ] Live migration support
- [ ] VM console access (serial/virtio-console)
- [ ] Metrics collection
- [ ] Event streaming to controller
