# UI Implementation Plan: CHV Operator Console

## Overview

Implement the CHV web UI according to the design specification in `docs/UI_DESIGN_SPEC.md`. This plan covers all sections: layout shell, component library, interaction patterns, and page implementations. Work is organized in vertical slices (feature-by-feature) rather than horizontal layers.

## Architecture Decisions

1. **SvelteKit with TypeScript** - Existing stack, server-side rendering for fast initial load
2. **Tailwind CSS for styling** - Utility-first, consistent with design token approach
3. **Component composition over configuration** - Flexible, maintainable components
4. **Vertical feature slicing** - Each task delivers working, testable functionality
5. **API client already exists** - Extend `$lib/api/client.ts` with new endpoints

## Current State Assessment

**Existing:**
- Basic SvelteKit setup with Tailwind
- Sidebar navigation component
- API client with authentication
- Networks list page (read-only)
- Storage list page (read-only)
- StateBadge component

**Missing:**
- Toast notification system
- Modal component
- Form components with validation
- Create Network/Storage modal flows
- Loading skeletons
- Empty states
- Confirmation dialogs
- Stats cards

---

## Task List

### Phase 1: Foundation Components

#### Task 1: Toast Notification System
**Description:** Implement a global toast notification system for success, error, and info messages. Includes Toast component, toast store, and auto-dismiss functionality.

**Acceptance criteria:**
- [ ] Toast component renders with correct styling per design spec (green/red/blue variants)
- [ ] Toast store manages queue of notifications
- [ ] Auto-dismiss after 5 seconds (configurable)
- [ ] Manual close button works
- [ ] Positioned top-right with 16px offset
- [ ] Multiple toasts stack vertically with 8px gap

**Verification:**
- [ ] Unit tests: `npm test -- --grep "Toast"`
- [ ] Storybook/visual: Trigger sample toasts
- [ ] Build: `npm run build` succeeds

**Dependencies:** None

**Files likely touched:**
- `ui/src/lib/components/Toast.svelte`
- `ui/src/lib/stores/toast.ts`
- `ui/src/routes/+layout.svelte` (add Toast container)

**Estimated scope:** Small (2-3 files)

---

#### Task 2: Modal Component
**Description:** Create a reusable Modal component with header, body, footer slots. Supports close on backdrop click, ESC key, and focus trap.

**Acceptance criteria:**
- [ ] Modal opens/closes with smooth 200ms transition
- [ ] Backdrop click closes modal (configurable)
- [ ] ESC key closes modal
- [ ] Focus trapped within modal when open
- [ ] Header slot with title and close button
- [ ] Body slot for content
- [ ] Footer slot for actions (right-aligned)
- [ ] Width variants: default (480px), wide (640px)

**Verification:**
- [ ] Unit tests: `npm test -- --grep "Modal"`
- [ ] Accessibility: Tab navigation, screen reader testing
- [ ] Build: `npm run build` succeeds

**Dependencies:** None

**Files likely touched:**
- `ui/src/lib/components/Modal.svelte`

**Estimated scope:** Small (1-2 files)

---

#### Task 3: Form Input Components
**Description:** Create reusable form components: Input, Select, FormField (label + input + error). Includes validation state styling.

**Acceptance criteria:**
- [ ] Input component with all states (default, focus, error, disabled)
- [ ] Select component with dropdown styling
- [ ] FormField wraps label, input, helper text, error message
- [ ] Label positioned above input (12px, gray)
- [ ] Error state: red border, red error message below
- [ ] Helper text: gray, below input
- [ ] Proper spacing (4px between label/input, 4px for helper/error)

**Verification:**
- [ ] Unit tests for components
- [ ] Visual inspection in Storybook or test page
- [ ] Build: `npm run build` succeeds

**Dependencies:** None

**Files likely touched:**
- `ui/src/lib/components/Input.svelte`
- `ui/src/lib/components/Select.svelte`
- `ui/src/lib/components/FormField.svelte`

**Estimated scope:** Small (3 files)

---

### Checkpoint: Foundation
- [ ] All tests pass: `cd ui && npm test`
- [ ] Build succeeds: `cd ui && npm run build`
- [ ] Visual check: Toast, Modal, and Form components render correctly

---

### Phase 2: Data Display Components

#### Task 4: Loading Skeletons
**Description:** Implement skeleton loading states for tables and cards. Animated pulse effect matching content structure.

**Acceptance criteria:**
- [ ] SkeletonRow component for table rows
- [ ] SkeletonCard component for card layouts
- [ ] Pulse animation: 1.5s ease-in-out infinite
- [ ] Background gradient animation
- [ ] Configurable number of rows
- [ ] Matches table column structure

**Verification:**
- [ ] Visual check: Skeletons look like content placeholders
- [ ] Animation smooth at 60fps
- [ ] Build: `npm run build` succeeds

**Dependencies:** None

**Files likely touched:**
- `ui/src/lib/components/SkeletonRow.svelte`
- `ui/src/lib/components/SkeletonCard.svelte`

**Estimated scope:** Small (2 files)

---

#### Task 5: Empty State Component
**Description:** Reusable empty state with icon, title, description, and CTA button.

**Acceptance criteria:**
- [ ] Centered layout with 48px icon
- [ ] Icon prop accepts Lucide icon component
- [ ] Title: 16px, gray
- [ ] Description: 14px, lighter gray
- [ ] Optional CTA button slot
- [ ] Used in: Networks, Storage, VMs, Images, Operations pages

**Verification:**
- [ ] Component renders with all props
- [ ] Visual inspection matches spec
- [ ] Build: `npm run build` succeeds

**Dependencies:** None

**Files likely touched:**
- `ui/src/lib/components/EmptyState.svelte`

**Estimated scope:** XS (1 file)

---

#### Task 6: Stats Card Component
**Description:** Stats cards for dashboard-style displays (used on VM list page).

**Acceptance criteria:**
- [ ] Card with background #F5F5F5, border #D0D0D0
- [ ] Title: 11px uppercase, gray
- [ ] Value: 32px, dark, font-weight 600
- [ ] Optional icon or trend indicator
- [ ] Responsive: stack on mobile

**Verification:**
- [ ] Visual matches design spec
- [ ] Build: `npm run build` succeeds

**Dependencies:** None

**Files likely touched:**
- `ui/src/lib/components/StatsCard.svelte`

**Estimated scope:** XS (1 file)

---

### Checkpoint: Data Display
- [ ] All tests pass
- [ ] Skeleton loading visible on slow connections
- [ ] Empty states render for all list pages

---

### Phase 3: Create Network Flow

#### Task 7: Extend API Client for Network Creation
**Description:** Add `createNetwork()` method to API client with proper types.

**Acceptance criteria:**
- [ ] `createNetwork(data: CreateNetworkInput)` method added
- [ ] Type `CreateNetworkInput` defined
- [ ] Returns `Promise<Network>`
- [ ] Error handling with message extraction
- [ ] Exports new types from `$lib/api/types.ts`

**Verification:**
- [ ] TypeScript compiles without errors
- [ ] Unit test for API client method
- [ ] Build: `npm run build` succeeds

**Dependencies:** None (backend already has POST /api/v1/networks)

**Files likely touched:**
- `ui/src/lib/api/client.ts`
- `ui/src/lib/api/types.ts`

**Estimated scope:** Small (2 files)

---

#### Task 8: Create Network Modal
**Description:** Modal form for creating networks with validation.

**Acceptance criteria:**
- [ ] Modal opens from Networks page "+ Create" button
- [ ] Form fields: Name (text), Mode (select, locked to "bridge"), Bridge Name (text), CIDR (text), Gateway IP (text)
- [ ] Field validation:
  - Name: required, unique, lowercase alphanumeric + hyphen
  - Mode: locked, no validation needed
  - Bridge Name: required
  - CIDR: required, valid CIDR format
  - Gateway: required, valid IP
- [ ] Submit button disabled until all fields valid
- [ ] On submit: POST to API, close modal, refresh list
- [ ] On error: Show error in modal, keep open
- [ ] Success toast on completion

**Verification:**
- [ ] Form validation works for all fields
- [ ] API integration successful
- [ ] Error handling displays messages
- [ ] E2E: Create network flow works end-to-end

**Dependencies:** 
- Task 2 (Modal component)
- Task 3 (Form components)
- Task 1 (Toast notifications)
- Task 7 (API client extension)

**Files likely touched:**
- `ui/src/lib/components/CreateNetworkModal.svelte`
- `ui/src/routes/networks/+page.svelte` (add button and modal)

**Estimated scope:** Medium (3-4 files)

---

### Checkpoint: Network Creation
- [ ] Can create network from UI
- [ ] Validation prevents invalid submissions
- [ ] Success/error feedback works

---

### Phase 4: Create Storage Pool Flow

#### Task 9: Extend API Client for Storage Pool Creation
**Description:** Add `createStoragePool()` method to API client.

**Acceptance criteria:**
- [ ] `createStoragePool(data: CreateStoragePoolInput)` method added
- [ ] Type `CreateStoragePoolInput` defined
- [ ] Returns `Promise<StoragePool>`
- [ ] Error handling

**Verification:**
- [ ] TypeScript compiles
- [ ] Build succeeds

**Dependencies:** None (backend already has POST /api/v1/storage-pools)

**Files likely touched:**
- `ui/src/lib/api/client.ts`
- `ui/src/lib/api/types.ts`

**Estimated scope:** XS (1-2 files)

---

#### Task 10: Create Storage Pool Modal
**Description:** Modal form for creating storage pools.

**Acceptance criteria:**
- [ ] Modal opens from Storage page "+ Create" button
- [ ] Form fields: Name (text), Type (select, locked to "localdisk"), Path (text), Capacity (number, optional)
- [ ] Field validation:
  - Name: required, unique
  - Type: locked
  - Path: required, absolute path
- [ ] Helper text for Path: "Absolute path on host filesystem"
- [ ] Submit button disabled until valid
- [ ] On submit: POST to API, close modal, refresh list
- [ ] Success/error toast feedback

**Verification:**
- [ ] Form validation works
- [ ] API integration successful
- [ ] E2E flow works

**Dependencies:**
- Task 2, 3, 1 (Modal, Forms, Toast)
- Task 9 (API client)

**Files likely touched:**
- `ui/src/lib/components/CreateStoragePoolModal.svelte`
- `ui/src/routes/storage/+page.svelte`

**Estimated scope:** Small (2-3 files)

---

### Checkpoint: Storage Creation
- [ ] Can create storage pool from UI
- [ ] Locked type field indicates MVP-1 limitation

---

### Phase 5: Enhanced VM List Page

#### Task 11: VM List Stats Cards
**Description:** Add stats cards to VM list page showing totals.

**Acceptance criteria:**
- [ ] Stats cards row at top of VM list
- [ ] Cards: Total VMs, Running, Stopped, Other
- [ ] Data computed from VM list
- [ ] Cards use StatsCard component
- [ ] Responsive: horizontal on desktop, vertical stack on mobile

**Verification:**
- [ ] Stats update when VM list changes
- [ ] Visual matches design spec
- [ ] Build succeeds

**Dependencies:**
- Task 6 (StatsCard component)

**Files likely touched:**
- `ui/src/routes/vms/+page.svelte`

**Estimated scope:** Small (1-2 files)

---

#### Task 12: VM List Loading State
**Description:** Add skeleton loading to VM list page.

**Acceptance criteria:**
- [ ] Show skeleton rows while loading VMs
- [ ] 5 skeleton rows with realistic column structure
- [ ] Smooth transition from skeleton to data
- [ ] Empty state when no VMs

**Verification:**
- [ ] Skeleton visible on slow connections
- [ ] Empty state shows when list is empty
- [ ] Data renders correctly when loaded

**Dependencies:**
- Task 4 (Skeleton components)
- Task 5 (EmptyState component)

**Files likely touched:**
- `ui/src/routes/vms/+page.svelte`

**Estimated scope:** Small (1 file)

---

### Checkpoint: VM List Enhancement
- [ ] Stats cards display correctly
- [ ] Loading state smooth

---

### Phase 6: Confirmation Dialogs

#### Task 13: Confirmation Dialog Component
**Description:** Reusable confirmation dialog for destructive actions.

**Acceptance criteria:**
- [ ] Title prop for action description
- [ ] Description prop for consequences
- [ ] Primary action: configurable text and variant (danger default)
- [ ] Secondary action: "Cancel"
- [ ] Returns Promise<boolean> for confirm/cancel
- [ ] Focus on primary action button

**Verification:**
- [ ] Dialog opens and closes correctly
- [ ] Promise resolves true on confirm, false on cancel
- [ ] Visual matches design spec

**Dependencies:**
- Task 2 (Modal component)

**Files likely touched:**
- `ui/src/lib/components/ConfirmDialog.svelte`

**Estimated scope:** Small (1-2 files)

---

#### Task 14: Add Confirmation to Delete Actions
**Description:** Wire up confirmation dialogs to delete actions (future: VM delete).

**Acceptance criteria:**
- [ ] ConfirmDialog shown before destructive actions
- [ ] Title: "Delete [Resource]?"
- [ ] Description explains consequences
- [ ] Only proceeds on explicit confirmation
- [ ] Cancel leaves resource untouched

**Verification:**
- [ ] Dialog appears on delete attempt
- [ ] Cancel prevents deletion
- [ ] Confirm proceeds with deletion

**Dependencies:**
- Task 13 (ConfirmDialog)

**Files likely touched:**
- Future VM detail page (or wherever delete exists)

**Estimated scope:** Small (depends on existing delete actions)

---

### Checkpoint: Confirmation Flows
- [ ] Confirmation dialogs work
- [ ] Cancel prevents action

---

### Phase 7: Polish & Integration

#### Task 15: Loading States on All List Pages
**Description:** Add skeleton loading to Networks, Storage, Images, Operations pages.

**Acceptance criteria:**
- [ ] Networks page shows skeleton while loading
- [ ] Storage page shows skeleton while loading
- [ ] Images page shows skeleton while loading
- [ ] Operations page shows skeleton while loading
- [ ] Empty states for all pages

**Verification:**
- [ ] All list pages handle loading gracefully
- [ ] Empty states show when appropriate

**Dependencies:**
- Task 4 (Skeleton)
- Task 5 (EmptyState)

**Files likely touched:**
- `ui/src/routes/networks/+page.svelte`
- `ui/src/routes/storage/+page.svelte`
- `ui/src/routes/images/+page.svelte`
- `ui/src/routes/operations/+page.svelte`

**Estimated scope:** Medium (4 files)

---

#### Task 16: Error Handling Integration
**Description:** Ensure all API errors surface in UI with appropriate feedback.

**Acceptance criteria:**
- [ ] Network errors show error toast
- [ ] Validation errors show inline in forms
- [ ] 401 errors redirect to login
- [ ] Error messages are user-friendly (not raw API errors)
- [ ] Error logging for debugging

**Verification:**
- [ ] Test with API offline: error toast shows
- [ ] Test with invalid token: redirect to login
- [ ] Test form validation: inline errors show

**Dependencies:** All previous tasks

**Files likely touched:**
- `ui/src/lib/api/client.ts` (error handling)
- Various page components

**Estimated scope:** Small (2-3 files)

---

### Checkpoint: Polish Complete
- [ ] All pages have loading states
- [ ] All pages have empty states
- [ ] Error handling works consistently

---

## Final Verification

### Pre-Completion Checklist
- [ ] All 16 tasks complete
- [ ] All tests pass: `cd ui && npm test`
- [ ] Build succeeds: `cd ui && npm run build`
- [ ] No console errors in browser
- [ ] Accessibility: keyboard navigation works
- [ ] Responsive: works at 320px, 768px, 1024px, 1440px
- [ ] Design spec compliance: visual inspection against `docs/UI_DESIGN_SPEC.md`

### E2E Test Scenarios
1. **Create Network Flow:** Click "+ Create" → Fill form → Submit → See new network in list → Toast confirms
2. **Create Storage Flow:** Click "+ Create" → Fill form → Submit → See new pool in list → Toast confirms
3. **Validation:** Submit empty form → See validation errors → Fill required fields → Submit succeeds
4. **Loading States:** Throttle connection → See skeleton → Data loads → Skeleton disappears
5. **Error Handling:** Stop backend → Load page → Error toast → Start backend → Refresh → Data loads

---

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Modal focus trap complexity | Medium | Use proven library or Svelte action, test thoroughly |
| Form validation edge cases | Medium | Use Zod for schema validation, comprehensive tests |
| API error message formatting | Low | Normalize errors in API client, user-friendly defaults |
| Responsive layout complexity | Low | Mobile-first CSS, test at all breakpoints |
| Browser compatibility | Low | Target modern browsers, use autoprefixer |

---

## Open Questions

1. **Zod for validation?** - Should we add Zod for form validation, or use basic HTML5 validation?
2. **Storybook?** - Do we want Storybook for component documentation, or is code sufficient?
3. **E2E testing?** - Should we add Playwright tests for critical flows?

---

## Timeline Estimate

| Phase | Tasks | Est. Effort |
|-------|-------|-------------|
| Phase 1: Foundation | 3 | 1 session |
| Phase 2: Data Display | 3 | 1 session |
| Phase 3: Network Flow | 2 | 1 session |
| Phase 4: Storage Flow | 2 | 1 session |
| Phase 5: VM List | 2 | 1 session |
| Phase 6: Confirmation | 2 | 1 session |
| Phase 7: Polish | 2 | 1 session |
| **Total** | **16** | **7 sessions** |

*Sessions are approximate. Some phases may be combined or split based on actual progress.*
