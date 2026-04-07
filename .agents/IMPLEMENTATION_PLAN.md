# CHV Gap Implementation Plan with Subagents

## Overview
This plan uses parallel subagents to implement the critical gaps in the CHV platform.

## Phase 1: Critical Fixes (Parallel Execution)

### Task 1.1: Fix JWT Authentication
**Subagent:** jwt-auth-fix  
**Files:** `internal/agent/server/http.go`  
**Dependencies:** None  

### Task 1.2: Add Proper Logging  
**Subagent:** logging-fix  
**Files:** `internal/agent/manager/vm.go`, `internal/hypervisor/launcher.go`  
**Dependencies:** None  

### Task 1.3: Fix Database Write Permissions
**Subagent:** db-fix  
**Files:** `internal/controller/store/` (SQLite initialization)  
**Dependencies:** None  

### Task 1.4: Fix Volume Lock Contention
**Subagent:** volume-lock-fix  
**Files:** `internal/storage/manager.go`, `internal/agent/manager/vm.go`  
**Dependencies:** None  

### Task 1.5: Add Health Check Endpoints
**Subagent:** health-endpoints  
**Files:** `internal/api/health.go` (new), `internal/agent/server/http.go`  
**Dependencies:** None  

## Execution Order

1. **Parallel Wave 1** (All Phase 1 tasks can run simultaneously):
   - Tasks 1.1, 1.2, 1.3, 1.4, 1.5

2. **Integration Point**: After Phase 1 complete, rebuild and test

3. **Parallel Wave 2** (Phase 2 tasks):
   - Tasks 2.1, 2.2, 2.3, 2.4

4. **Integration Point**: After Phase 2 complete, rebuild and test

5. **Parallel Wave 3** (Phase 3 tasks):
   - Tasks 3.1, 3.2, 3.3

## Success Criteria

Each subagent must:
1. Implement the fix according to spec
2. Add/update tests
3. Verify no regressions
4. Update documentation if needed
