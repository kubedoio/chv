# WebUI Changes

> **Note:** This file is retained for historical reference. For the current state of the UI and recent changes, see [`CHANGELOG.md`](../CHANGELOG.md) and the [`PHASED_IMPLEMENTATION_PLAN.md`](../PHASED_IMPLEMENTATION_PLAN.md).

## Historical Sprint Summary (Pre-2026-04-14)

### Components Added

- **ProgressBar.svelte** — Visual progress indicator for downloads and long-running operations
- **StatusIndicator.svelte** — Animated status indicator for real-time state changes with pulse animation

### Pages Enhanced

- **Dashboard** — Stats cards (VMs, Images, Pools, Networks), 10-second auto-poll, recent events widget
- **VM Detail** — State-aware polling (3s transient / 10s stable), status spinner, PID display, manual refresh
- **Events** — 10-second auto-refresh, new-event badge with auto-clear
- **Images** — Import-aware polling (3s during import / 30s idle), status indicators

### Technical Improvements

- Lifecycle-respecting polling with `onDestroy` cleanup
- Dynamic interval adjustment based on resource state
- Shared API client instance
- Full TypeScript support for all new components

## Current UI State (as of 2026-04-26)

The WebUI has grown substantially since the above sprint. Current capabilities include:

- **VM Management** — List, detail, create (with basic and advanced tabs), start/stop/reboot/delete, serial console
- **Storage** — Volumes, storage pools, snapshots, image import/export
- **Networking** — Networks list, firewall rule viewer
- **Infrastructure** — Nodes list, enrollment status, hypervisor settings (partial)
- **Tasks & Events** — Task list, event stream with filtering
- **Settings** — User management, API tokens, hypervisor settings page (partial)
- **Metrics** — VM metrics widgets using Chart.js

### Resolved UI Gaps (Post-2026-04-26)

| Gap | Resolution |
|-----|------------|
| UI Production Readiness refactor (Tailwind-first, component split) | **Done** — 10 feature folders created, DataTable sub-modules extracted, Dashboard refactored to 292 lines |
| Command palette | **Done** — `CommandPalette.svelte` with fuzzy-search and 16 commands |
| E2E tests (Playwright) | **Done** — 3 test files covering navigation, VMs, and settings; runs in CI |
| Client-side caching layer | **Done** — `api-cache.svelte.ts` with TTL, stale-while-revalidate, and mutation invalidation |
| Dark mode | **Done** — Full `[data-theme="dark"]` token system with `UserMenu` toggle |
| DataTable component splitting | **Done** — Selection, Sorting, Visibility extracted to `shared/datatable/` |
| Overview page logic extraction | **Done** — Dashboard helpers and store extracted; page reduced from 635 → 292 lines |
| svelte-check warnings | **Done** — 0 errors, 0 warnings |
| Toast hardcoded colors | **Done** — Uses design-system CSS variables |

### Remaining UI Gaps

| Gap | Status | Priority |
|-----|--------|----------|
| `vms/[id]/+page.svelte` still 467 lines | Partial | P2 |
| `CreateVMModal.svelte` still 580 lines | Partial | P2 |
| `InventoryListPage` uses `any[]` types | Not started | P2 |
| `awaiting-operator-input` task state | Reserved for later | P3 |
| A11y suppressions on modal backdrops | Not started | P2 |

See the [`PHASED_IMPLEMENTATION_PLAN.md`](../PHASED_IMPLEMENTATION_PLAN.md) for the full UI backlog and sprint schedule.
