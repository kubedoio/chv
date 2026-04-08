# Leftovers Implementation Plan

## Overview
Post-Phase 7 identified gaps that need completion for a functional MVP.

## Identified Issues

### 1. Agent Error Format Inconsistency (Priority: Medium)
**Issue**: Agent handlers use simple `{"error": "message"}` format while controller expects structured `{"error": {"code": "...", "message": "...", "retryable": ...}}` format.

**Files to Modify**:
- `internal/agent/handlers/common.go` - Update respondError to use agentapi.Error
- All agent handlers - Update error responses

### 2. Image Import End-to-End (Priority: High)
**Issue**: Image import creates DB record but doesn't trigger actual download. Download exists in agent but not called.

**Implementation Steps**:
1. Add DownloadImage method to agentclient.Client
2. Create background download queue/worker in controller
3. Wire up image import to trigger download via agent
4. Add checksum validation after download
5. Update image status through import lifecycle

**Files to Create/Modify**:
- `internal/agentclient/client.go` - Add DownloadImage method
- `internal/images/worker.go` - New: background download worker
- `internal/api/images.go` - Trigger async download after create
- `internal/operations/service.go` - Add more comprehensive logging

### 3. Events Repository (Priority: Medium)
**Issue**: Events endpoint returns empty list. Operations exist in DB but no retrieval API.

**Implementation Steps**:
1. Extend operations service with query methods
2. Update events API to return operations with filtering
3. Add resource_type filtering support

**Files to Modify**:
- `internal/operations/service.go` - Add ListOperations method
- `internal/api/events.go` - Implement proper listEvents handler
- `internal/db/sqlite.go` - Add filtering to ListOperations

### 4. VM Service Lifecycle (Priority: Medium)
**Issue**: vm.Service instantiated per-request in handlers. Should be singleton.

**Implementation Steps**:
1. Create VM service at handler initialization
2. Pass service to handler instead of creating per-request
3. Ensure thread-safety for concurrent VM operations

**Files to Modify**:
- `internal/api/handler.go` - Create vm.Service at init
- `internal/api/vms.go` - Use injected service

### 5. Real VM Start/Stop with Cloud Hypervisor (Priority: High)
**Issue**: StartVM/StopVM simulate state changes without actually launching CH.

**Implementation Steps**:
1. Create hypervisor launcher package (exists but needs integration)
2. Add LaunchVM/StopVM methods to agent
3. Controller calls agent to manage CH processes
4. Implement PID tracking and health checking
5. Handle CH stdout/stderr logging

**Files to Create/Modify**:
- `internal/agent/handlers/vms.go` - New: VM lifecycle handlers
- `internal/agent/services/vmmanagement.go` - New: CH process management
- `internal/agentclient/client.go` - Add VM lifecycle methods
- `internal/vm/service.go` - Call agent for actual start/stop

### 6. Password Security (Priority: Low - Document)
**Issue**: Cloud-init user-data stores passwords in plaintext.
**Resolution**: Document as MVP limitation. SSH-only recommended for production.

## Implementation Order

1. **Task 1**: Agent error format consistency (foundation)
2. **Task 2**: Agent image download client method (enables Task 3)
3. **Task 3**: Image import end-to-end (high user value)
4. **Task 4**: Events repository (better observability)
5. **Task 5**: VM service lifecycle (code quality)
6. **Task 6**: Real VM start/stop (core functionality)
7. **Task 7**: E2E integration test

## Acceptance Criteria

- [ ] Agent returns structured errors matching controller format
- [ ] Image import creates record, downloads via agent, validates checksum, updates status
- [ ] Events endpoint returns actual operations with filtering
- [ ] VM service is singleton, thread-safe
- [ ] VM start/stop actually launches/terminates CH processes
- [ ] Full E2E test: Create VM → Start VM → Stop VM → Delete VM
