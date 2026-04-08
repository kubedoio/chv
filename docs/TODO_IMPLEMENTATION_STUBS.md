# Implementation Stubs - Detailed TODOs

## 1. Database Migration - Add `format` Column

**Location:** `internal/db/sqlite.go`

**Add after table creation:**
```go
func (r *Repository) migrateAddImageFormat() error {
    // Check if column exists
    var count int
    err := r.db.QueryRow(`
        SELECT COUNT(*) FROM pragma_table_info('images') WHERE name = 'format'
    `).Scan(&count)
    if err != nil {
        return err
    }
    
    if count == 0 {
        _, err = r.db.Exec(`ALTER TABLE images ADD COLUMN format TEXT DEFAULT 'qcow2'`)
        return err
    }
    return nil
}
```

**Call from NewRepository:**
```go
func NewRepository(dbPath string) (*Repository, error) {
    // ... existing code ...
    if err := repo.migrateAddImageFormat(); err != nil {
        return nil, fmt.Errorf("failed to migrate images table: %w", err)
    }
    return repo, nil
}
```

---

## 2. VM Console - CH API PTY Creation

**Location:** `internal/agent/services/vmconsole.go:178-199`

**Replace TODO with:**
```go
func (s *VMConsoleService) getSerialConsole(apiSocket string) (string, error) {
    // CH doesn't have a direct PTY API. Options:
    // 
    // Option 1: Use --serial tty which creates /dev/pts/X
    // Option 2: Use --console off and query logs via API
    // Option 3: Create pseudo-terminal ourselves and proxy
    //
    // Recommended approach for MVP: Use PTY path convention
    // When CH starts with --serial tty, it outputs PTY path to stdout
    // We capture this in vmmanagement.go and store it
    
    // For now, try common PTY paths based on convention
    workspace := filepath.Dir(apiSocket)
    ptyPath := filepath.Join(workspace, "serial.ptty")
    
    // Check if PTY exists
    if _, err := os.Stat(ptyPath); err == nil {
        return ptyPath, nil
    }
    
    // Alternative: Try to get from CH API
    conn, err := net.Dial("unix", apiSocket)
    if err != nil {
        return "", fmt.Errorf("cannot connect to CH API: %w", err)
    }
    defer conn.Close()
    
    // Try CH API console endpoint
    req, _ := http.NewRequest("GET", "/api/v1/vm.console", nil)
    if err := req.Write(conn); err != nil {
        return "", err
    }
    
    resp, err := http.ReadResponse(bufio.NewReader(conn), req)
    if err != nil {
        return "", err
    }
    defer resp.Body.Close()
    
    if resp.StatusCode == http.StatusOK {
        // Parse response for PTY path
        var result struct {
            Path string `json:"path"`
        }
        if err := json.NewDecoder(resp.Body).Decode(&result); err == nil && result.Path != "" {
            return result.Path, nil
        }
    }
    
    return "", fmt.Errorf("console not available - VM may not have serial enabled")
}
```

**Update vmmanagement.go to capture PTY:**
```go
// In StartVM, capture stdout to extract PTY path
// CH outputs: "PTY path: /dev/pts/X" when started with --serial tty
```

---

## 3. VM Status Endpoint

**Location:** `internal/api/vms.go` (add new handler)

**Add:**
```go
func (h *Handler) getVMStatus(w http.ResponseWriter, r *http.Request) {
    ctx := requestContext(r)
    vmID := chi.URLParam(r, "id")
    
    vm, err := h.repo.GetVMByID(ctx, vmID)
    if err != nil {
        h.writeError(w, http.StatusInternalServerError, apiError{
            Code: "vm_get_failed", Message: err.Error(), Retryable: true,
        })
        return
    }
    if vm == nil {
        h.writeError(w, http.StatusNotFound, apiError{Code: "not_found", Message: "VM not found"})
        return
    }
    
    // If running, get fresh status from agent
    var health *agentapi.VMHealth
    if vm.ActualState == "running" && h.vmService != nil {
        health, _ = h.vmService.GetVMHealth(ctx, vmID)
    }
    
    h.writeJSON(w, http.StatusOK, map[string]any{
        "id": vm.ID,
        "actual_state": vm.ActualState,
        "desired_state": vm.DesiredState,
        "pid": vm.CloudHypervisorPID,
        "uptime": calculateUptime(vm),
        "health": health,
    })
}
```

**Register route in handler.go:**
```go
r.Get("/{id}/status", h.getVMStatus)
```

---

## 4. Image Progress Tracking

**Location:** New file `internal/images/progress.go`

**Create:**
```go
package images

import (
    "sync"
    "time"
)

type ProgressTracker struct {
    mu       sync.RWMutex
    progress map[string]*ImportProgress
}

type ImportProgress struct {
    ImageID         string    `json:"image_id"`
    Status          string    `json:"status"`
    ProgressPercent int       `json:"progress_percent"`
    BytesDownloaded int64     `json:"bytes_downloaded"`
    TotalBytes      int64     `json:"total_bytes"`
    Speed           string    `json:"speed"`
    Error           string    `json:"error,omitempty"`
    UpdatedAt       time.Time `json:"updated_at"`
}

func NewProgressTracker() *ProgressTracker {
    return &ProgressTracker{
        progress: make(map[string]*ImportProgress),
    }
}

func (pt *ProgressTracker) Update(imageID string, p *ImportProgress) {
    pt.mu.Lock()
    defer pt.mu.Unlock()
    p.UpdatedAt = time.Now()
    pt.progress[imageID] = p
}

func (pt *ProgressTracker) Get(imageID string) *ImportProgress {
    pt.mu.RLock()
    defer pt.mu.RUnlock()
    return pt.progress[imageID]
}

func (pt *ProgressTracker) Delete(imageID string) {
    pt.mu.Lock()
    defer pt.mu.Unlock()
    delete(pt.progress, imageID)
}

// Cleanup removes entries older than 24 hours
func (pt *ProgressTracker) Cleanup() {
    pt.mu.Lock()
    defer pt.mu.Unlock()
    
    cutoff := time.Now().Add(-24 * time.Hour)
    for id, p := range pt.progress {
        if p.UpdatedAt.Before(cutoff) {
            delete(pt.progress, id)
        }
    }
}
```

**Update worker.go to use tracker:**
```go
type Worker struct {
    repo     *db.Repository
    client   *agentclient.Client
    tracker  *ProgressTracker  // Add this
    logger   *logger.Logger
}
```

---

## 5. VM Restart Action

**Location:** `internal/vm/service.go` (add method)

**Add:**
```go
// RestartVM restarts a VM (stop then start)
func (s *Service) RestartVM(ctx context.Context, vmID string) error {
    // Get current state
    vm, err := s.repo.GetVMByID(ctx, vmID)
    if err != nil {
        return fmt.Errorf("failed to get VM: %w", err)
    }
    if vm == nil {
        return fmt.Errorf("VM not found: %s", vmID)
    }
    
    // Only restart if running or stopped (not in transition)
    if vm.ActualState == StatusStarting || vm.ActualState == StatusStopping {
        return fmt.Errorf("VM is in transition state: %s", vm.ActualState)
    }
    
    // Stop if running
    if vm.ActualState == StatusRunning {
        if err := s.StopVM(ctx, vmID); err != nil {
            return fmt.Errorf("failed to stop VM for restart: %w", err)
        }
        
        // Wait for stop to complete (with timeout)
        timeout := time.After(30 * time.Second)
        ticker := time.NewTicker(500 * time.Millisecond)
        defer ticker.Stop()
        
        for {
            select {
            case <-timeout:
                return fmt.Errorf("timeout waiting for VM to stop")
            case <-ticker.C:
                vm, _ = s.repo.GetVMByID(ctx, vmID)
                if vm != nil && vm.ActualState == StatusStopped {
                    goto stopped
                }
            }
        }
    stopped:
    }
    
    // Small delay to ensure cleanup
    time.Sleep(1 * time.Second)
    
    // Start VM
    return s.StartVM(ctx, vmID)
}
```

**Add API handler in vms.go:**
```go
func (h *Handler) restartVM(w http.ResponseWriter, r *http.Request) {
    ctx := requestContext(r)
    vmID := chi.URLParam(r, "id")
    
    if err := h.vmService.RestartVM(ctx, vmID); err != nil {
        h.writeError(w, http.StatusInternalServerError, apiError{
            Code: "vm_restart_failed", Message: err.Error(), Retryable: true,
        })
        return
    }
    
    h.writeJSON(w, http.StatusOK, map[string]any{
        "message": "VM restart initiated",
    })
}
```

**Register route:**
```go
r.Post("/{id}/restart", h.restartVM)
```

---

## 6. UI Polling for VM Status

**Location:** `ui/src/routes/vms/[id]/+page.svelte`

**Add:**
```typescript
<script>
import { onMount, onDestroy } from 'svelte';

let pollInterval: ReturnType<typeof setInterval>;

onMount(() => {
    // Initial load
    loadVM();
    
    // Poll every 5 seconds if VM is in active state
    pollInterval = setInterval(() => {
        if (vm && (vm.actual_state === 'running' || vm.actual_state === 'starting' || vm.actual_state === 'stopping')) {
            refreshVM();
        }
    }, 5000);
});

onDestroy(() => {
    if (pollInterval) clearInterval(pollInterval);
});

async function refreshVM() {
    try {
        const updated = await client.getVM($page.params.id);
        vm = updated;
    } catch (err) {
        console.error('Failed to refresh VM:', err);
    }
}
</script>
```

---

## 7. Event Real-time Badge

**Location:** `ui/src/lib/components/Sidebar.svelte`

**Add:**
```typescript
<script>
import { onMount } from 'svelte';

let newEvents = 0;
let lastEventCheck = new Date();

onMount(() => {
    const interval = setInterval(checkNewEvents, 30000); // Check every 30s
    return () => clearInterval(interval);
});

async function checkNewEvents() {
    try {
        const events = await client.listEvents();
        newEvents = events.filter(e => new Date(e.timestamp) > lastEventCheck).length;
    } catch (err) {
        console.error('Failed to check events:', err);
    }
}

function clearBadge() {
    newEvents = 0;
    lastEventCheck = new Date();
}
</script>

<!-- In template -->
<a href="/events" on:click={clearBadge}>
    Events
    {#if newEvents > 0}
        <span class="badge">{newEvents}</span>
    {/if}
</a>
```

---

## 8. Remove/Fix Image Fetcher Stub

**Location:** `internal/images/fetcher.go`

**Option A - Remove entirely:**
```bash
rm internal/images/fetcher.go
```

**Option B - Implement as agent wrapper:**
```go
package images

import (
    "context"
    "github.com/chv/chv/internal/agentapi"
    "github.com/chv/chv/internal/agentclient"
)

// Fetcher handles downloading images via agent
type Fetcher struct {
    client *agentclient.Client
}

func NewFetcher(client *agentclient.Client) *Fetcher {
    return &Fetcher{client: client}
}

func (f *Fetcher) Download(ctx context.Context, imageID, url, destPath string) error {
    req := &agentapi.ImageImportRequest{
        ImageID:  imageID,
        SourceURL: url,
        DestPath: destPath,
    }
    _, err := f.client.DownloadImage(ctx, req)
    return err
}
```

---

## 9. Add VM Health Method to Service

**Location:** `internal/vm/service.go`

**Add:**
```go
// GetVMHealth retrieves health status from agent
func (s *Service) GetVMHealth(ctx context.Context, vmID string) (*agentapi.VMHealth, error) {
    if s.agentClient == nil {
        return nil, fmt.Errorf("agent not available")
    }
    
    vm, err := s.repo.GetVMByID(ctx, vmID)
    if err != nil {
        return nil, err
    }
    if vm == nil {
        return nil, fmt.Errorf("VM not found")
    }
    
    req := &agentapi.VMHealthRequest{
        VMID:      vmID,
        APISocket: filepath.Join(vm.WorkspacePath, "api.sock"),
    }
    
    resp, err := s.agentClient.GetVMHealth(ctx, req)
    if err != nil {
        return nil, err
    }
    
    return &resp.Health, nil
}
```

**Note:** Need to add `GetVMHealth` to agentclient if not exists.

---

## Quick Reference: File Changes

| Feature | Files to Modify | Files to Create |
|---------|-----------------|-----------------|
| DB Migration | `internal/db/sqlite.go` | - |
| VM Console | `internal/agent/services/vmconsole.go` | - |
| Status Endpoint | `internal/api/vms.go`, `internal/api/handler.go` | - |
| Progress Tracking | `internal/images/worker.go` | `internal/images/progress.go` |
| VM Restart | `internal/vm/service.go`, `internal/api/vms.go` | - |
| UI Polling | `ui/src/routes/vms/[id]/+page.svelte` | - |
| Event Badge | `ui/src/lib/components/Sidebar.svelte` | - |
| Remove Fetcher | - | Delete `internal/images/fetcher.go` |
