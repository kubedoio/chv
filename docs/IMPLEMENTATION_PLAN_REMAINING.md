# CHV Implementation Plan - Remaining Work

## Summary

This document outlines the remaining TODOs, stubs, and missing features for the CHV (Cloud Hypervisor Virtualization) platform. The codebase is functional with core VM lifecycle management, but several features need completion for a production-ready MVP.

---

## Current State

### ✅ Completed
- VM CRUD operations (Create, Read, Delete)
- VM Start/Stop via Cloud Hypervisor
- Image import with async worker
- TAP device management
- Network and Storage Pool management
- Structured logging
- WebUI with all major pages
- Authentication and token management
- Event/Operations tracking

### ⚠️ Partial/Stubs
- VM Console (WebSocket infrastructure exists, CH API PTY missing)
- VM Metrics collection (backend implemented, UI integration needed)
- Image fetcher (stub - uses agent directly)

### ❌ Missing
- VM Restart action
- Bulk operations (bulk delete, bulk start)
- Image import progress streaming
- Real-time UI updates (polling/WebSocket)
- VM snapshots
- Live migration

---

## Phase 1: Critical Fixes (Week 1)

### 1.1 Database Schema Fix
**Priority:** Critical
**File:** `internal/db/sqlite.go`

**Issue:** Images table missing `format` column causing SQL errors.

**Implementation:**
```go
// Add migration for format column
func (r *Repository) migrateImagesTable() error {
    // Check if format column exists
    // ALTER TABLE images ADD COLUMN format TEXT DEFAULT 'qcow2'
}
```

**Acceptance Criteria:**
- [ ] Image worker starts without SQL errors
- [ ] Image import lifecycle works end-to-end

---

### 1.2 VM Console - CH API PTY Integration
**Priority:** High
**File:** `internal/agent/services/vmconsole.go:190`

**Current State:** Has TODO placeholder
```go
// TODO: In full implementation, send HTTP request to CH to create PTY
```

**Implementation:**
1. Research CH API console endpoint (`/api/v1/vm.console`)
2. Implement HTTP-over-Unix-socket request to CH
3. Create PTY and return path
4. Wire into WebSocket handler

**API Flow:**
```
UI WebSocket → Controller (proxy) → Agent WebSocket → CH API → VM Serial Console
```

**Acceptance Criteria:**
- [ ] Can connect to running VM console from browser
- [ ] Terminal displays VM boot output
- [ ] Can send keystrokes to VM
- [ ] Connection cleanup on VM stop

---

### 1.3 VM Status Polling
**Priority:** High
**Files:** 
- `ui/src/routes/vms/[id]/+page.svelte`
- `internal/vm/service.go`

**Issue:** UI shows stale VM status; users must refresh page to see state changes.

**Implementation:**
```typescript
// Add polling to VM detail page
onMount(() => {
  const interval = setInterval(async () => {
    if (vm.actual_state === 'running' || vm.actual_state === 'starting') {
      vm = await client.getVM($page.params.id);
    }
  }, 5000);
  return () => clearInterval(interval);
});
```

**Backend Enhancement:**
- Add `GET /api/v1/vms/:id/status` lightweight endpoint
- Return only state, PID, uptime (not full VM object)

**Acceptance Criteria:**
- [ ] VM status updates automatically in UI
- [ ] State transitions visible in real-time
- [ ] Polling stops when VM reaches terminal state

---

## Phase 2: UX Improvements (Week 2)

### 2.1 Event Real-time Updates
**Priority:** Medium
**Files:**
- `ui/src/routes/events/+page.svelte`
- `ui/src/lib/components/Sidebar.svelte`

**Implementation:**
```typescript
// Auto-refresh events page
const POLL_INTERVAL = 10000; // 10 seconds

// Show badge for new events in sidebar
$: newEventCount = events.filter(e => isNew(e)).length;
```

**Acceptance Criteria:**
- [ ] Events page auto-refreshes
- [ ] Sidebar shows badge with new event count
- [ ] Clicking badge clears notification

---

### 2.2 Image Import Progress
**Priority:** Medium
**Files:**
- `internal/images/worker.go`
- `internal/agent/services/imagedownload.go`
- `ui/src/lib/components/ProgressBar.svelte`

**Implementation:**
1. Add progress tracking to image download
2. Store progress in database or memory cache
3. Add `GET /api/v1/images/:id/progress` endpoint
4. Poll progress from UI during import

**API Design:**
```go
type ImageProgress struct {
    ImageID          string  `json:"image_id"`
    Status           string  `json:"status"` // pending/downloading/validating/ready/failed
    ProgressPercent  int     `json:"progress_percent"`
    BytesDownloaded  int64   `json:"bytes_downloaded"`
    TotalBytes       int64   `json:"total_bytes"`
    DownloadSpeed    string  `json:"download_speed"` // e.g., "5.2 MB/s"
    Error            string  `json:"error,omitempty"`
}
```

**Acceptance Criteria:**
- [ ] Image list shows progress bar during import
- [ ] Status transitions visible (downloading → validating → ready)
- [ ] Failed imports show error message with retry button

---

### 2.3 VM Restart Action
**Priority:** Medium
**Files:**
- `internal/api/vms.go`
- `internal/vm/service.go`
- `ui/src/routes/vms/[id]/+page.svelte`

**Implementation:**
```go
// Backend
func (s *Service) RestartVM(ctx context.Context, vmID string) error {
    // Stop VM
    if err := s.StopVM(ctx, vmID); err != nil {
        return err
    }
    // Wait for stop
    time.Sleep(2 * time.Second)
    // Start VM
    return s.StartVM(ctx, vmID)
}
```

**Acceptance Criteria:**
- [ ] Restart button in VM detail page
- [ ] Backend implements atomic restart (stop + start)
- [ ] UI shows restart progress

---

## Phase 3: Advanced Features (Week 3-4)

### 3.1 Metrics Dashboard
**Priority:** Medium
**Files:**
- `ui/src/lib/components/MetricsChart.svelte`
- `ui/src/routes/vms/[id]/+page.svelte`

**Backend:** Already implemented (`internal/agent/services/vmhealth.go`)

**Implementation:**
1. Poll metrics every 30 seconds for running VMs
2. Display charts using MetricsChart component
3. Show CPU, Memory, Disk I/O, Network I/O

**Acceptance Criteria:**
- [ ] VM detail page shows metrics charts
- [ ] Charts update automatically
- [ ] Historical data visible (last 1 hour)

---

### 3.2 Bulk Operations
**Priority:** Low
**Files:**
- `ui/src/routes/vms/+page.svelte`
- `internal/api/vms.go`

**Implementation:**
```typescript
// Multi-select in VM list
let selectedVMs: string[] = [];

// Bulk actions
async function bulkStart() {
  await Promise.all(selectedVMs.map(id => client.startVM(id)));
}
```

**Acceptance Criteria:**
- [ ] Checkbox selection in VM list
- [ ] Bulk start/stop/delete actions
- [ ] Confirmation dialog for destructive actions

---

### 3.3 Image Import from Local File
**Priority:** Low
**Files:**
- `internal/api/images.go`
- `ui/src/lib/components/ImportImageModal.svelte`

**Implementation:**
1. Add file upload endpoint
2. Stream upload to agent
3. Validate and move to images directory

**Acceptance Criteria:**
- [ ] Can upload image file from browser
- [ ] Progress shown during upload
- [ ] File validated (checksum, format)

---

## Phase 4: Performance & Polish (Week 4)

### 4.1 Remove Image Fetcher Stub
**Priority:** Low
**File:** `internal/images/fetcher.go`

**Implementation:**
Since image download is handled by agent, either:
- Remove fetcher entirely, OR
- Implement as wrapper around agent client

**Acceptance Criteria:**
- [ ] No "not implemented" stub in codebase
- [ ] Clean separation: controller = orchestration, agent = execution

---

### 4.2 VM Lifecycle Timeline
**Priority:** Low
**Files:**
- `ui/src/routes/vms/[id]/+page.svelte`
- `internal/db/sqlite.go` (add events tracking)

**Implementation:**
1. Track VM lifecycle events (created, started, stopped, error)
2. Display timeline in VM detail

**Acceptance Criteria:**
- [ ] Timeline shows VM history
- [ ] Errors highlighted

---

### 4.3 Boot Logs
**Priority:** Low
**Files:**
- `internal/agent/services/vmmanagement.go`
- `ui/src/lib/components/LogViewer.svelte`

**Implementation:**
1. Capture CH stdout/stderr to log file
2. Add `GET /api/v1/vms/:id/logs` endpoint
3. Display in LogViewer component

**Acceptance Criteria:**
- [ ] Can view VM boot logs
- [ ] Auto-scroll to latest
- [ ] Search/filter logs

---

## Implementation Priority Matrix

| Feature | Impact | Effort | Priority |
|---------|--------|--------|----------|
| Database Schema Fix | High | Low | P0 |
| VM Console | High | High | P1 |
| VM Status Polling | High | Low | P1 |
| Event Real-time | Medium | Low | P2 |
| Image Import Progress | Medium | Medium | P2 |
| VM Restart | Medium | Low | P2 |
| Metrics Dashboard | Medium | Low | P2 |
| Bulk Operations | Low | Medium | P3 |
| Local File Import | Low | Medium | P3 |
| VM Timeline | Low | Medium | P3 |
| Boot Logs | Low | Medium | P3 |

---

## Technical Debt

### Code Cleanup
- [ ] Remove unused `internal/images/fetcher.go` or implement properly
- [ ] Standardize error handling across agent handlers
- [ ] Add request ID propagation for tracing

### Testing Gaps
- [ ] Unit tests for VM console service
- [ ] Integration tests for image import worker
- [ ] E2E tests for VM lifecycle

### Documentation
- [ ] API documentation (OpenAPI/Swagger)
- [ ] Admin guide for troubleshooting
- [ ] Developer contribution guide

---

## Architecture Decisions Needed

1. **Console Implementation:**
   - Option A: CH API HTTP console endpoint
   - Option B: Direct PTY access via Unix socket
   - Decision needed before implementation

2. **Progress Tracking:**
   - Option A: Database polling
   - Option B: In-memory cache (Redis/simple cache)
   - Option C: Server-Sent Events (SSE)

3. **Real-time Updates:**
   - Option A: Polling (simple, robust)
   - Option B: WebSocket (complex, real-time)
   - Recommendation: Start with polling, upgrade to WebSocket later

---

## Success Criteria

- All P0 and P1 items completed
- UI feels responsive (status updates within 5 seconds)
- Console access works for debugging VMs
- Image import shows progress and handles failures gracefully
