# Leftovers Implementation - Complete

## Summary
All identified leftover tasks have been completed.

## Tasks Completed

### ✅ Task 1: Agent Error Format Consistency
**Changes**:
- Updated `internal/agent/handlers/common.go` - respondError now uses structured `agentapi.Error` format
- Updated all agent handlers (images.go, install.go, bootstrap.go, cloudinit.go) to use new error format
- Updated `internal/agentclient/client.go` - parseError now parses structured errors

**Result**: Agent and controller now use consistent error formats.

### ✅ Task 2: Agent Image Download Client Method
**Changes**:
- Added `DownloadImage()` method to `internal/agentclient/client.go`

**Result**: Controller can now call agent to download images.

### ✅ Task 3: Complete Image Import End-to-End
**Changes**:
- Created `internal/images/worker.go` - Background worker for image imports
  - Downloads images via agent
  - Validates checksums
  - Updates image status through lifecycle
  - Resumes pending imports on startup
- Updated `internal/api/images.go` - Queue import after creating record
- Updated `internal/api/handler.go` - Inject imageWorker
- Updated `cmd/chv-controller/main.go` - Create and start image worker

**Result**: Image import is now fully async and end-to-end functional.

### ✅ Task 4: Implement Events Repository
**Changes**:
- Updated `internal/operations/service.go` - Added `ListOperations()` method
- Rewrote `internal/api/events.go` - Returns actual operations with filtering

**Result**: Events endpoint returns real operational data.

### ✅ Task 5: Fix VM Service Lifecycle
**Changes**:
- Updated `internal/api/handler.go` - Added vmService as singleton
- Updated `internal/api/vms.go` - Use injected vmService instead of creating per-request
- Updated `cmd/chv-controller/main.go` - Create VM service singleton

**Result**: VM service is now a proper singleton, improving performance and consistency.

### ✅ Task 6: Implement Real VM Start/Stop with CH
**Changes**:
- Created `internal/agentapi/vm.go` - VM lifecycle API types
- Created `internal/agent/services/vmmanagement.go` - CH process management
  - StartVM: Launches CH with proper args
  - StopVM: SIGTERM/SIGKILL process
  - Status checking
- Created `internal/agent/handlers/vms.go` - HTTP handlers for VM routes
- Updated `internal/agent/server.go` - Added VM routes
- Updated `internal/agentclient/client.go` - Added StartVM, StopVM, GetVMStatus methods
- Updated `internal/vm/service.go` - Use agent client when available
- Updated `cmd/chv-controller/main.go` - Set agent client on VM service

**Result**: VM start/stop now actually launches/terminates Cloud Hypervisor processes via agent.

## Files Created

```
internal/images/worker.go                 # Background image import worker
internal/agentapi/vm.go                   # VM lifecycle API types
internal/agent/services/vmmanagement.go   # CH process management
internal/agent/handlers/vms.go            # VM HTTP handlers
docs/LEFTOVERS_PLAN.md                    # Implementation plan
docs/LEFTOVERS_COMPLETE.md                # This file
```

## Files Modified

```
internal/agent/handlers/common.go         # Structured errors
internal/agent/handlers/images.go         # Error format
internal/agent/handlers/install.go        # Error format
internal/agent/handlers/bootstrap.go      # Error format
internal/agent/handlers/cloudinit.go      # Error format
internal/agent/server.go                  # VM routes
internal/agentclient/client.go            # DownloadImage, VM lifecycle methods
internal/api/images.go                    # Async import trigger
internal/api/events.go                    # Real events from operations
internal/api/vms.go                       # Use singleton service
internal/api/handler.go                   # Inject services
internal/operations/service.go            # ListOperations
internal/vm/service.go                    # Agent integration
 cmd/chv-controller/main.go               # Wire up all services
```

## Architecture

```
Controller                              Agent
----------                              -----
Image Worker ──────DownloadImage──────> Image Download Handler
   │                                         │
   │                                    File System
   │
VM Service ────────StartVM/StopVM─────> VM Management Handler
   │                                         │
   │                                    Cloud Hypervisor
   │                                         │
   │                                    Actual VMs
```

## Next Steps (Post-MVP)

1. **Image Import Progress**: Stream download progress to UI
2. **VM Console**: Access serial console of running VMs
3. **VM Metrics**: Collect CPU/memory/disk stats from CH API
4. **Network Management**: Create TAP devices dynamically
5. **Migration**: Live migration support

## Build Verification

```bash
cd /srv/data02/projects/chv
go build -o chv-controller ./cmd/chv-controller
go build -o chv-agent ./cmd/chv-agent
cd ui && npm run build
```

All builds successful ✅
