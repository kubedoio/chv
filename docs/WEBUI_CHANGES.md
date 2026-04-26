# WebUI Changes

> **Note:** This file is retained for historical reference. For the current state of the UI and recent changes, see [`CHANGELOG.md`](../CHANGELOG.md) and the [Gap Analysis & Implementation Plan](./plans/2026-04-24-gap-analysis-and-implementation-plan.md).

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

### Known UI Gaps

| Gap | Status | Priority |
|-----|--------|----------|
| UI Production Readiness refactor (Tailwind-first, component split) | Not started | P1 |
| Command palette | TODO | P2 |
| E2E tests (Playwright) | Missing | P3 |
| Client-side caching layer | Missing | P3 |
| Dark mode | Not started | P3 |
| DataTable component splitting (688 lines) | Not started | P2 |
| Overview page logic extraction (526 lines) | Not started | P2 |

See the [Gap Analysis](./plans/2026-04-24-gap-analysis-and-implementation-plan.md) for the full UI backlog and sprint schedule.
