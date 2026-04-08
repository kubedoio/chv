# Implementation Review and Next Steps

## Review of Leftovers Implementation

### ✅ What Was Implemented

#### Backend (Go)
1. **Agent Error Format Consistency**
   - All agent handlers now return structured `agentapi.Error` format
   - Controller client properly parses structured errors
   - Error responses include code, message, and retryable flag

2. **Image Import End-to-End**
   - `images.Worker` - Background worker for async imports
   - Downloads via agent, validates checksums, updates status
   - Auto-resumes pending imports on controller restart
   - Queue-based processing

3. **Events Repository**
   - Operations service with `ListOperations()`
   - Events API returns actual operations with filtering
   - Proper event transformation from operations

4. **VM Service Lifecycle**
   - Singleton VM service injected into handlers
   - No more per-request service creation
   - Thread-safe for concurrent operations

5. **Real VM Start/Stop with Cloud Hypervisor**
   - `VMManagementService` on agent manages CH processes
   - Agent endpoints: POST /v1/vms/start, POST /v1/vms/stop
   - Controller uses agent client for VM lifecycle
   - PID tracking for health checking
   - SIGTERM/SIGKILL for graceful shutdown

### 🔍 Code Quality Assessment

| Aspect | Status | Notes |
|--------|--------|-------|
| Error Handling | ✅ Good | Structured errors throughout |
| Logging | ⚠️ Basic | Uses fmt.Printf, needs structured logging |
| Testing | ⚠️ Minimal | Unit tests exist but coverage low |
| Documentation | ✅ Good | DESIGN.md, LEFTOVERS_COMPLETE.md |
| Type Safety | ✅ Good | Strong TypeScript types |

### 🐛 Known Issues

1. **VM Status Polling**: UI doesn't poll for VM status updates
2. **Image Import Progress**: No progress indication during download
3. **Network TAP Creation**: TAP devices not dynamically created
4. **CH API Integration**: Not using CH HTTP API for status/metrics
5. **VM Console Access**: No serial console in UI
6. **Event Real-time**: Events page doesn't auto-refresh

## Next Steps Implementation Plan

### Phase 8: WebUI Enhancements (Priority: High)

#### 8.1 VM Status Polling
- Add polling to VM detail page (5s interval when running)
- Show real-time state changes
- Display PID and uptime

#### 8.2 Image Import Progress
- Add progress tracking to image import
- Show download percentage
- Display status transitions (downloading → validating → ready)

#### 8.3 Event Real-time Updates
- Auto-refresh events page (10s interval)
- Show new events badge in sidebar
- Filter by resource type

#### 8.4 Dashboard Enhancements
- Show system health summary
- Resource usage cards (CPU, Memory, Storage)
- Recent events widget
- Quick actions (Create VM, Import Image)

#### 8.5 VM Detail Improvements
- Show VM lifecycle timeline
- Display boot logs
- Add restart button
- Show network configuration details

### Phase 9: Backend Polish (Priority: Medium)

#### 9.1 Structured Logging
- Replace fmt.Printf with proper logger
- Add request IDs for tracing
- Structured JSON logs

#### 9.2 VM Health Checking
- Poll CH API for VM health
- Update VM status based on process health
- Handle unexpected CH crashes

#### 9.3 TAP Device Management
- Create TAP devices on VM start
- Clean up TAP on VM stop
- Handle bridge attachment

#### 9.4 Image Import Improvements
- Support resume for failed downloads
- Parallel downloads (up to N concurrent)
- Import from local file (not just URL)

### Phase 10: Advanced Features (Priority: Low)

#### 10.1 VM Console
- WebSocket proxy to CH serial console
- Terminal UI in browser
- Session recording

#### 10.2 VM Metrics
- CPU usage via CH API
- Memory usage
- Disk I/O stats
- Network I/O stats

#### 10.3 Snapshots
- Create VM snapshots
- Restore from snapshot
- Snapshot management UI

#### 10.4 Live Migration
- Migrate VMs between hosts
- Progress tracking
- Pre-copy optimization

## WebUI Changes Required

### Components to Create

1. **ProgressBar.svelte** - Show download/progress percentage
2. **LogViewer.svelte** - Display VM boot logs
3. **ResourceChart.svelte** - CPU/Memory usage charts
4. **StatusIndicator.svelte** - Animated status indicator
5. **Timeline.svelte** - VM lifecycle timeline

### Pages to Enhance

1. **Dashboard (+page.svelte)**
   - Add stats cards
   - Recent events widget
   - Quick action buttons
   - System health status

2. **Images (+page.svelte)**
   - Show import progress
   - Add retry button for failed imports
   - Bulk import from list

3. **VMs/[id] (+page.svelte)**
   - Auto-refresh status
   - Boot logs tab
   - Network details tab
   - Restart action
   - Console access (future)

4. **Events (+page.svelte)**
   - Auto-refresh
   - Better filtering
   - Export to CSV
   - Event detail view

### API Client Updates

```typescript
// Add to client.ts
subscribeToEvents(callback: (event: Event) => void): () => void;
getVMLogs(id: string): Promise<string>;
getVMMetrics(id: string): Promise<VMMetrics>;
```

### Types to Add

```typescript
interface VMMetrics {
  cpu_usage: number;
  memory_usage: number;
  disk_read_bytes: number;
  disk_write_bytes: number;
  net_rx_bytes: number;
  net_tx_bytes: number;
}

interface ImageImportProgress {
  image_id: string;
  status: 'pending' | 'downloading' | 'validating' | 'ready' | 'failed';
  progress_percent: number;
  bytes_downloaded: number;
  total_bytes: number;
  error?: string;
}
```

## Implementation Priority

### Week 1: Core WebUI
1. VM status polling
2. Event auto-refresh
3. Dashboard enhancements

### Week 2: Image & VM Polish
1. Image import progress
2. VM detail improvements
3. Better error handling in UI

### Week 3: Backend Polish
1. Structured logging
2. VM health checking
3. TAP device management

### Week 4: Advanced Features
1. VM console (if time permits)
2. VM metrics
3. Performance optimizations

## Success Criteria

- [ ] VM status updates in real-time without refresh
- [ ] Image import shows progress percentage
- [ ] Events page auto-refreshes
- [ ] Dashboard provides system overview
- [ ] All API errors show user-friendly messages
- [ ] Console logs accessible for debugging VMs
