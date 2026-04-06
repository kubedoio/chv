# Console + Basic Settings Implementation Design

## Overview
Implement functional VM Console and VM Settings features for CHV platform.

**Approach:** Minimal viable implementation (Approach A)
**Estimated effort:** 5-6 hours
**Scope:** Wire up existing console component + add basic VM settings editing

---

## Console Integration

### Current State
- `VMConsole.vue` component fully implemented with xterm.js
- WebSocket backend for console proxy exists
- Currently shows placeholder in VMsView

### Implementation
**File:** `chv-ui/src/views/VMsView.vue`

**Changes:**
1. Import VMConsole component
2. Add `showConsole` boolean ref
3. Replace Console tab placeholder:
   - Show "Open Console" button when VM is running
   - Show message when VM is stopped
4. Render VMConsole component with vmId and visible props

**UI Flow:**
- User clicks "Console" tab
- Sees "Open Console" button (enabled if VM running)
- Click opens VMConsole modal
- Modal has terminal with connection status

**Edge cases:**
- VM stopped: show "Start VM to access console" message
- Connection error: handled by VMConsole component

---

## VM Settings Form

### Current State
- Shows "future release" placeholder
- No update API endpoint exists

### Implementation

#### Frontend: Settings Form
**File:** `chv-ui/src/views/VMsView.vue`

**Fields:**
| Field | Type | Validation |
|-------|------|------------|
| CPU | number (1-64) | Required, min 1, max 64 |
| Memory | number (MB) | Required, min 512, max 262144 |
| Boot Mode | select | Required: cloud_image, direct_kernel |

**UI States:**
- Form populated with current VM spec on tab switch
- Warning banner: "VM must be stopped to apply changes"
- Save button disabled if VM running
- Cancel button resets to original values

**Error handling:**
- API errors shown as toast notification
- Validation errors inline per field

#### Backend: Update VM API
**File:** `internal/api/vms.go`

**Endpoint:** `PUT /api/v1/vms/{id}`

**Request:**
```json
{
  "spec": {
    "cpu": 4,
    "memory_mb": 8192,
    "boot": {
      "mode": "cloud_image"
    }
  }
}
```

**Logic:**
1. Parse VM ID from URL
2. Fetch VM from store
3. Validate VM exists (404 if not)
4. Validate VM is stopped (409 if running)
5. Parse and validate new spec
6. Merge with existing spec (preserve disks, networks)
7. Save to database
8. Return updated VM

**Error responses:**
- 404: VM not found
- 409: VM is running
- 400: Invalid spec
- 500: Database error

#### Store: Update VM
**File:** `internal/store/vms.go` (if needed)

Add `UpdateVM(ctx, vm *models.VirtualMachine) error` if not exists.

---

## API Contract

### GET /api/v1/vms/{id}
Already exists - returns VM with current spec.

### PUT /api/v1/vms/{id}
**New endpoint** for updating VM configuration.

**Request headers:**
```
Authorization: Bearer {token}
Content-Type: application/json
```

**Success response (200):**
```json
{
  "id": "uuid",
  "name": "vm-name",
  "actual_state": "stopped",
  "spec": { /* updated spec */ }
}
```

**Error responses:**
- 404 Not Found: VM doesn't exist
- 409 Conflict: VM is running
- 400 Bad Request: Invalid spec fields

---

## UI/UX Decisions

### Console Tab
- Show VMConsole in modal (existing behavior)
- Don't embed inline (simpler, no layout changes)
- Console button only enabled when VM running

### Settings Tab
- Form layout: label above input (consistent with design system)
- Warning about stopped VM requirement
- Save/Cancel buttons at bottom
- Success toast on save

### Styling
- Use existing CSS variables from main.css
- Follow DESIGN.md spacing (4px base unit)
- PrimeVue components for form elements

---

## Testing Checklist

### Console
- [ ] Console modal opens from Console tab
- [ ] Connects when VM is running
- [ ] Shows message when VM is stopped
- [ ] Disconnects when modal closed
- [ ] Reconnect button works

### Settings
- [ ] Form populates with current VM spec
- [ ] Warning shown when VM running
- [ ] Save disabled when VM running
- [ ] Validation works (invalid CPU/memory)
- [ ] Save updates VM in database
- [ ] Success/error toasts shown
- [ ] Cancel resets form

---

## Files to Modify

| File | Changes |
|------|---------|
| `chv-ui/src/views/VMsView.vue` | Add console modal trigger, settings form |
| `internal/api/vms.go` | Add updateVM handler |
| `internal/api/handler.go` | Register PUT route |

## Files to Verify (no changes needed)

| File | Status |
|------|--------|
| `chv-ui/src/components/VMConsole.vue` | Already complete |
| `internal/hypervisor/console.go` | Backend already complete |
| `internal/api/console.go` | WebSocket endpoint exists |

---

## Open Questions

None - design approved by user.

---

## Approval

Design reviewed and approved. Proceed to implementation planning.
