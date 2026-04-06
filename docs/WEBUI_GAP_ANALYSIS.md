# WebUI Gap Analysis

## Executive Summary

The WebUI has several buttons and features that are non-functional due to missing backend API endpoints or unimplemented UI components. This document identifies all gaps and provides implementation recommendations.

---

## Critical Issues (Breaking Functionality)

### 1. Missing DELETE Endpoints

The WebUI calls these endpoints but they return **404 Not Found**:

| Endpoint | UI Location | Priority |
|----------|-------------|----------|
| `DELETE /api/v1/networks/{id}` | Networks view | High |
| `DELETE /api/v1/storage-pools/{id}` | Storage view | High |
| `DELETE /api/v1/images/{id}` | Images view | High |

**Impact:** Users cannot delete resources after creation.

### 2. Port Configuration Mismatch

The WebUI is being accessed at `10.5.199.83:8081` but the controller now runs on port `8082`.

**Root Cause:** Browser cache or old container still running.

**Solution:** Clear browser cache and restart services.

---

## Feature Gaps

### 3. Create Network Button (Non-Functional)

**Location:** `/networks` view - "Create Network" button

**Issue:** Button exists but has no click handler or modal.

**Required Implementation:**
- Create Network modal/form
- Form fields: Name, Bridge Name, CIDR, Gateway IP, DNS Servers
- Connect to `POST /api/v1/networks` (already implemented)

### 4. Create Storage Pool Button (Non-Functional)

**Location:** `/storage` view - "Add Storage" button

**Issue:** Button exists but has no click handler or modal.

**Required Implementation:**
- Create Storage Pool modal/form
- Form fields: Name, Type (local/NFS), Path/Export
- Connect to `POST /api/v1/storage-pools` (already implemented)

### 5. Import Image Button (Non-Functional)

**Location:** `/images` view - "Import Image" button

**Issue:** Button exists but has no click handler or modal.

**Required Implementation:**
- Import Image modal/form
- Form fields: Name, OS Family, Source URL, Format, Architecture
- Connect to `POST /api/v1/images/import` (already implemented)

### 6. Register Node Button (Non-Functional)

**Location:** `/nodes` view - "Register Node" button

**Issue:** Button exists but has no click handler or modal.

**Required Implementation:**
- Register Node modal/form
- Form fields: Hostname, Management IP, CPU Cores, RAM MB
- Connect to `POST /api/v1/nodes/register` (already implemented)

### 7. Create VM Button (Non-Functional)

**Location:** Dashboard Quick Actions & VMs view

**Issue:** Button links to `/vms` but there's no VM creation form.

**Required Implementation:**
- Create VM wizard/modal
- Form fields:
  - Name
  - CPU cores
  - Memory (MB)
  - Image selection
  - Disk size
  - Network selection
- Connect to `POST /api/v1/vms` (already implemented)

---

## Missing Features

### 8. VM Console Access

**Location:** VMs view - needs "Console" button per VM

**Backend Status:** ✅ Implemented (`GET /api/v1/vms/{id}/console`)

**UI Gap:** No console button or terminal component.

**Required Implementation:**
- "Console" button on each VM row
- WebSocket terminal component (xterm.js)
- Connection to WebSocket endpoint

### 9. VM Actions (Partial)

**Implemented:**
- ✅ Start VM
- ✅ Stop VM  
- ✅ Reboot VM
- ✅ Delete VM

**Missing:**
- ❌ Resize disk (UI button exists but not wired)
- ❌ VM details view

### 10. Operations/Activity Log

**Location:** Dashboard "Recent Activity" card

**Issue:** Shows mock data instead of real operations.

**Backend Status:** ✅ Implemented (`GET /api/v1/operations`)

**Required Implementation:**
- Connect dashboard to operations API
- Real-time updates or polling

### 11. System Health (Static Data)

**Location:** Dashboard "System Health" card

**Issue:** Shows hardcoded/mock data.

**Backend Status:** ✅ Implemented (`GET /health`, `GET /metrics`)

**Required Implementation:**
- Fetch real health data from `/health`
- Show actual storage usage from `/metrics`

---

## Backend Gaps

### 12. Missing Endpoints

These endpoints need to be implemented in the backend:

```go
// networks.go - Add:
func (h *Handler) deleteNetwork(w http.ResponseWriter, r *http.Request)

// storage.go - Add:
func (h *Handler) deleteStoragePool(w http.ResponseWriter, r *http.Request)

// images.go - Add:
func (h *Handler) deleteImage(w http.ResponseWriter, r *http.Request)
```

Also need store methods:
```go
store.DeleteNetwork(ctx, id)
store.DeleteStoragePool(ctx, id)
store.DeleteImage(ctx, id)
```

---

## UI/UX Improvements

### 13. Loading States

Many actions don't show loading indicators:
- VM start/stop/reboot
- Network list loading
- Storage pool list loading

### 14. Error Handling

Error messages are generic. Need:
- Specific error messages from API
- Toast notifications for success/error
- Retry mechanisms

### 15. Empty States

Views show "No X found" but could be improved with:
- Call-to-action buttons
- Help text explaining how to create resources

### 16. Confirmation Dialogs

Destructive actions (delete) need confirmation:
- VM delete
- Network delete
- Storage pool delete
- Image delete

---

## Priority Matrix

| Priority | Feature | Effort | Impact |
|----------|---------|--------|--------|
| P0 | Fix DELETE endpoints | Low | Critical |
| P0 | Port configuration | Low | Critical |
| P1 | Create VM form | High | High |
| P1 | VM Console | Medium | High |
| P2 | Create Network form | Medium | Medium |
| P2 | Create Storage form | Medium | Medium |
| P2 | Import Image form | Medium | Medium |
| P2 | Register Node form | Medium | Medium |
| P3 | Operations/Activity | Low | Medium |
| P3 | System Health | Low | Low |
| P3 | Confirmation dialogs | Low | Medium |
| P3 | Toast notifications | Medium | Medium |

---

## Implementation Roadmap

### Phase 1: Critical Fixes (1-2 days)
1. Implement DELETE endpoints in backend
2. Verify port configuration
3. Add confirmation dialogs for delete

### Phase 2: Core Features (1 week)
1. Create VM wizard
2. VM console component
3. Wire up resize disk button

### Phase 3: Resource Management (3-4 days)
1. Create Network form
2. Create Storage form
3. Import Image form
4. Register Node form

### Phase 4: Polish (2-3 days)
1. Operations/Activity feed
2. System Health real data
3. Loading states
4. Toast notifications
5. Empty state improvements

---

## API Endpoint Status

| Endpoint | Method | Status | WebUI Usage |
|----------|--------|--------|-------------|
| `/health` | GET | ✅ | Dashboard |
| `/metrics` | GET | ✅ | Dashboard |
| `/api/v1/tokens` | POST | ✅ | Login |
| `/api/v1/nodes` | GET | ✅ | Nodes list |
| `/api/v1/nodes/register` | POST | ✅ | Register node |
| `/api/v1/nodes/{id}` | GET | ✅ | Node details |
| `/api/v1/nodes/{id}/maintenance` | POST | ✅ | Maintenance mode |
| `/api/v1/networks` | GET | ✅ | Networks list |
| `/api/v1/networks` | POST | ✅ | Create network |
| `/api/v1/networks/{id}` | GET | ✅ | Network details |
| `/api/v1/networks/{id}` | DELETE | ❌ | Delete network |
| `/api/v1/storage-pools` | GET | ✅ | Storage list |
| `/api/v1/storage-pools` | POST | ✅ | Create storage |
| `/api/v1/storage-pools/{id}` | GET | ✅ | Storage details |
| `/api/v1/storage-pools/{id}` | DELETE | ❌ | Delete storage |
| `/api/v1/images` | GET | ✅ | Images list |
| `/api/v1/images/import` | POST | ✅ | Import image |
| `/api/v1/images/{id}` | GET | ✅ | Image details |
| `/api/v1/images/{id}` | DELETE | ❌ | Delete image |
| `/api/v1/vms` | GET | ✅ | VMs list |
| `/api/v1/vms` | POST | ✅ | Create VM |
| `/api/v1/vms/{id}` | GET | ✅ | VM details |
| `/api/v1/vms/{id}` | DELETE | ✅ | Delete VM |
| `/api/v1/vms/{id}/start` | POST | ✅ | Start VM |
| `/api/v1/vms/{id}/stop` | POST | ✅ | Stop VM |
| `/api/v1/vms/{id}/reboot` | POST | ✅ | Reboot VM |
| `/api/v1/vms/{id}/resize-disk` | POST | ✅ | Resize disk |
| `/api/v1/vms/{id}/console` | GET | ✅ | VM console |
| `/api/v1/operations` | GET | ✅ | Activity log |
| `/api/v1/operations/{id}` | GET | ✅ | Operation details |
| `/api/v1/operations/{id}/logs` | GET | ✅ | Operation logs |

---

## Store Implementation Status

| Store | Method | Status | Used By |
|-------|--------|--------|---------|
| Networks | fetchNetworks | ✅ | NetworksView |
| Networks | createNetwork | ✅ | (not wired) |
| Storage | fetchStoragePools | ✅ | StorageView |
| Storage | createStoragePool | ✅ | (not wired) |
| Images | fetchImages | ✅ | ImagesView |
| Images | importImage | ✅ | (not wired) |
| VMs | fetchVMs | ✅ | VMsView, Dashboard |
| VMs | createVM | ✅ | (not wired) |
| VMs | start/stop/reboot/delete | ✅ | VMsView |
| Nodes | fetchNodes | ✅ | NodesView, Dashboard |
| Nodes | registerNode | ✅ | (not wired) |
| Dashboard | fetchStats | ✅ | Dashboard |
| Dashboard | fetchRecentActivity | ❌ | Dashboard (mock) |

---

## Recommendations

1. **Immediate:** Implement DELETE endpoints to allow resource cleanup
2. **Short-term:** Create VM form is highest impact feature
3. **Medium-term:** VM console for debugging
4. **Long-term:** Polish UX with loading states and notifications
