# CHV UI Production Readiness — P0 + P1 Design

## Context

The CHV web UI (`ui/`) has grown to ~80 Svelte components across 25 routes. A gap analysis against production frontend engineering standards revealed significant drift between documentation and implementation, an inconsistent dual styling system, and components that have grown beyond maintainable size.

This design covers the P0 (critical fixes) and P1 (structural improvements) items needed to bring the UI to a production-quality baseline.

## Goals

1. Make the design system documentation an accurate source of truth
2. Eliminate visual inconsistencies (hardcoded colors, raw Tailwind bypassing tokens)
3. Consolidate to a single, maintainable styling approach
4. Split components over 300 lines into focused, testable units
5. Reorganize the flat component folder into a scalable feature-based structure

## Non-Goals

- Adding new features or pages
- Adding E2E tests (P2 scope)
- Adding client-side data caching (P2 scope)
- Mobile-first responsive refactor (P3 scope)
- Dark mode implementation (P3 scope)

## P0 — Design System & Color Fixes

### 1. Sync DESIGN.md with implementation

The current `ui/DESIGN.md` documents a Proxmox-orange palette (`#e57035`) and Inter fonts. The actual implementation in `app.css` uses a warm brown/amber palette (`#8f5a2a`) and IBM Plex fonts.

**Decision:** Update DESIGN.md to match the implemented system. The brown palette is already in production and the code is the ground truth.

| Token | Current Value (app.css) | DESIGN.md Claim |
|-------|------------------------|-----------------|
| `--color-primary` | `#8f5a2a` | `#e57035` |
| `--font-sans` | IBM Plex Sans | Inter |
| `--font-mono` | IBM Plex Mono | JetBrains Mono |
| `--color-success` | `#3f6b45` | `#22c55e` |
| `--color-danger` | `#9b4338` | `#ef4444` |
| `--color-warning` | `#9a6a1f` | `#eab308` |
| `--color-info` | `#49627d` | `#3b82f6` |

Update all sections of DESIGN.md: color system, typography, spacing, shadows, component specs, and usage examples.

### 2. Fix hardcoded colors in Button.svelte

`Button.svelte` contains hardcoded `rgba(229, 112, 53, ...)` shadow values on lines 139, 144, 148, and 151. These are Proxmox-orange remnants that do not match the actual primary color.

Replace with token-based values derived from `--color-primary`.

### 3. Fix VMMetricsWidget.svelte token usage

`VMMetricsWidget.svelte` uses raw Tailwind utility classes (`bg-slate-50`, `text-blue-500`, `bg-green-500`, `text-slate-600`) that bypass the design token system entirely.

Rewrite to use the project's CSS custom properties via Tailwind's arbitrary value syntax or mapped utility classes.

## P1 — Styling System Consolidation

### Decision: Tailwind-first

The project currently uses three overlapping styling mechanisms:
1. Tailwind CSS utility classes
2. Global CSS classes in `app.css` (`.btn`, `.card`, `.input`, etc.)
3. Component-level `<style>` blocks

**Decision:** Consolidate to Tailwind-first. `app.css` becomes a pure token file. Global utility classes are removed. Components use Tailwind utilities referencing CSS custom properties.

### Refactor app.css

Strip `app.css` down to:
- CSS custom properties (colors, fonts, spacing, shadows, radius, animation tokens)
- `@tailwind` directives
- Global reset / base styles only
- `prefers-reduced-motion` and `prefers-contrast: high` media queries

Delete these global utility classes:
- `.btn`, `.btn-primary`, `.btn-secondary`, `.btn-ghost`, `.btn-danger`
- `.card`, `.card-interactive`
- `.input`, `.label`, `.form-hint`
- `.badge`, `.badge-success`, `.badge-warning`, `.badge-danger`
- `.table`, `.table-container`
- `.skeleton`
- `.page-container`, `.page-header`, `.page-title`, `.section-title`
- `.grid-cards`, `.grid-stats`

### Migrate primitive components

Update primitive components to use Tailwind utilities with token references:

```svelte
<!-- Button.svelte -->
<button class="inline-flex items-center justify-center gap-2 rounded-sm font-medium text-sm transition-all ...">
```

Keep component `<style>` blocks only for:
- Complex keyframe animations (e.g., skeleton shimmer)
- Pseudo-element styling that Tailwind can't express
- `prefers-reduced-motion` overrides specific to a component

## P1 — Component Architecture

### Folder reorganization

Move from flat `lib/components/` to feature-based folders:

```
lib/components/
  primitives/           (existing — Button, Input, Card, Badge, Skeleton, Tooltip)
  shell/                (existing — AppShell, AppNav, CommandBar, etc.)
  system/               (existing — PageShell, ResourceTable, FilterPanel, etc.)
  data-display/         (DataTable + extracted subcomponents)
  vms/                  (VMMetricsWidget, VMCard, VMPowerMenu, VMMetricsHistory, etc.)
  nodes/                (NodeHealthStatus, etc.)
  modals/               (Modal, ConfirmAction, CreateVMModal, DeleteVMModal, etc.)
  feedback/             (EmptyState, Toast, ToastContainer, StateBanner, InstallStatusPanel)
  navigation/           (MobileNav, TreeNavigation, SearchModal, AppNav, etc.)
  forms/                (FormField, CloudInitEditor, CloudInitPreview, CloudInitViewer)
  charts/               (ChartJS, MetricsChart, MetricsChartEnhanced, Sparkline)
```

Update all imports in routes and components after moving files.

### Component splitting

**DataTable.svelte (688 lines)**
Extract into:
- `DataTable.svelte` — rendering and layout only
- `useTableSelection.ts` — selection logic (shift-click ranges, toggle)
- `useTableSorting.ts` — sort state and toggle logic
- `ColumnVisibilityDropdown.svelte` — column show/hide UI

**Overview +page.svelte (526 lines)**
Extract into:
- `$lib/webui/overview-helpers.ts` — `formatTimeAgo`, `severityTone`, `statusTone`
- `$lib/webui/overview-derive.ts` — `postureChips` and `attentionItems` derived state helpers
- Keep `+page.svelte` as a thin presentation layer

**CreateVMModal.svelte (506 lines)**
Extract into:
- `CreateVMModal.svelte` — shell, state orchestration, submit handling
- `VMFormBasicSection.svelte` — name, description, node selection
- `VMFormResourceSection.svelte` — CPU, memory, storage
- `VMFormNetworkSection.svelte` — network attachment

## Testing & Verification

- Run `npx svelte-check` after each batch of changes
- Run `npm run build` (vite build) to verify no bundler errors
- No new tests added in this PR — testing improvements are P2 scope
- Verify no visual regressions by checking key pages compile and render

## Rollout Sequence

1. **P0 color fixes** — DESIGN.md rewrite, Button.svelte shadows, VMMetricsWidget tokens
2. **app.css refactor** — strip global classes, keep tokens only
3. **Primitive component migration** — Button, Input, Card, Badge, Skeleton to Tailwind
4. **Folder reorganization** — move files to feature folders, update imports
5. **Component splitting** — DataTable, overview page, CreateVMModal

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Import breakage during folder reorg | Do folder moves in one commit, run svelte-check immediately after |
| Visual regression from app.css changes | Migrate primitives first (isolated), then shell components |
| Tailwind class bloat | Use `@apply` in component styles if a repeated pattern emerges, or extract to a reusable component |
| Scope creep | Strictly limit to files in gap analysis. No new features. |

## Success Criteria

- [ ] `svelte-check` passes with zero errors
- [ ] `vite build` completes successfully
- [ ] DESIGN.md accurately describes the implemented palette and fonts
- [ ] No hardcoded colors remain in components (all use tokens)
- [ ] No component file exceeds 300 lines (except DataTable which may remain large until full split)
- [ ] Components are organized in feature folders, not flat
