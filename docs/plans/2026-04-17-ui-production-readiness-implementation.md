# CHV UI Production Readiness — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix design system drift, consolidate styling to Tailwind-first, reorganize components into feature folders, and split oversized components.

**Architecture:** Migrate `app.css` from a dual global-class + token file to a pure token file. Move global utility classes into Tailwind-based primitive components. Reorganize the flat `lib/components/` directory into feature folders. Extract logic from components over 300 lines into dedicated helper modules and subcomponents.

**Tech Stack:** Svelte 5, SvelteKit 2, Tailwind CSS 3, TypeScript 5, Vite 5

---

### Task 1: P0 — Rewrite DESIGN.md to match implemented tokens

**Files:**
- Modify: `ui/DESIGN.md`

**Step 1: Update color system section**

Replace the Proxmox-orange primary colors with the actual implemented palette:

| Token | New Value |
|-------|-----------|
| `--color-primary` | `#8f5a2a` |
| `--color-primary-hover` | `#9f6837` |
| `--color-primary-active` | `#76471f` |
| `--color-primary-light` | `#f5eadc` |
| `--color-primary-dark` | `#5e3513` |
| `--color-success` | `#3f6b45` |
| `--color-warning` | `#9a6a1f` |
| `--color-danger` | `#9b4338` |
| `--color-info` | `#49627d` |
| `--color-neutral-50` | `#f7f3ec` |
| `--color-neutral-900` | `#191612` |

**Step 2: Update typography section**

Replace Inter + JetBrains Mono with:
```css
--font-sans: 'IBM Plex Sans', system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
--font-mono: 'IBM Plex Mono', 'SFMono-Regular', 'Consolas', monospace;
```

**Step 3: Update all component spec examples**

Replace orange-tinted examples (`rgba(229, 112, 53, ...)`) with brown-tinted equivalents using the new primary color.

**Step 4: Commit**

```bash
git add ui/DESIGN.md
git commit -m "docs: sync DESIGN.md with implemented brown/amber palette"
```

---

### Task 2: P0 — Fix hardcoded orange shadows in Button.svelte

**Files:**
- Modify: `ui/src/lib/components/primitives/Button.svelte:139,144,148,151`

**Step 1: Replace hardcoded rgba values**

In `.btn-primary`, replace:
```css
box-shadow: 0 2px 8px rgba(229, 112, 53, 0.3);
```
with:
```css
box-shadow: 0 2px 8px var(--color-primary-glow);
```

In `.btn-primary:hover:not(:disabled)`, replace:
```css
box-shadow: 0 4px 12px rgba(229, 112, 53, 0.4);
```
with:
```css
box-shadow: 0 4px 12px rgba(143, 90, 42, 0.35);
```

In `.btn-primary:active:not(:disabled)`, replace:
```css
box-shadow: 0 1px 4px rgba(229, 112, 53, 0.3);
```
with:
```css
box-shadow: 0 1px 4px var(--color-primary-glow);
```

In the hover background gradient, replace `#e05a35` with `var(--color-primary-hover)`.

**Step 2: Verify build**

```bash
cd ui && npx svelte-check
```
Expected: PASS (no errors in Button.svelte)

**Step 3: Commit**

```bash
git add ui/src/lib/components/primitives/Button.svelte
git commit -m "fix: replace hardcoded orange shadows with primary tokens in Button"
```

---

### Task 3: P0 — Fix VMMetricsWidget.svelte to use design tokens

**Files:**
- Modify: `ui/src/lib/components/VMMetricsWidget.svelte`

**Step 1: Audit and replace raw Tailwind colors**

Replace all raw Tailwind color utilities with token-based equivalents:

| Current | Replacement |
|---------|-------------|
| `bg-white` | `bg-[var(--shell-surface)]` or `bg-white` (keep white if intentional) |
| `border-slate-200` | `border-[var(--shell-line)]` |
| `text-slate-900` | `text-[var(--shell-text)]` |
| `text-slate-700` | `text-[var(--shell-text-secondary)]` |
| `text-slate-600` | `text-[var(--shell-text-secondary)]` |
| `text-slate-500` | `text-[var(--shell-text-muted)]` |
| `bg-slate-100` | `bg-[var(--shell-surface-muted)]` |
| `bg-slate-50` | `bg-[var(--shell-surface-muted)]` |
| `text-slate-400` | `text-[var(--shell-text-muted)]` |
| `bg-green-500` | `bg-[var(--color-success)]` |
| `bg-green-50` | `bg-[var(--color-success-light)]` |
| `text-green-600` | `text-[var(--color-success)]` |
| `text-green-700` | `text-[var(--color-success-dark)]` |
| `text-red-500` | `text-[var(--color-danger)]` |
| `bg-red-500` | `bg-[var(--color-danger)]` |
| `text-blue-500` | `text-[var(--color-info)]` |
| `bg-blue-50` | `bg-[var(--color-info-light)]` |
| `text-blue-600` | `text-[var(--color-info)]` |
| `text-blue-700` | `text-[var(--color-info-dark)]` |
| `text-purple-500` | `text-[var(--color-info)]` |
| `text-orange-500` | `text-[var(--color-warning)]` |

**Step 2: Verify build**

```bash
cd ui && npx svelte-check
```

**Step 3: Commit**

```bash
git add ui/src/lib/components/VMMetricsWidget.svelte
git commit -m "fix: migrate VMMetricsWidget to design token colors"
```

---

### Task 4: P1 — Refactor app.css to pure tokens

**Files:**
- Modify: `ui/src/app.css`

**Step 1: Extract and preserve tokens**

Keep these sections intact:
- `@import` for Google Fonts
- `@tailwind` directives
- `:root` CSS custom properties (all `--color-*`, `--font-*`, `--text-*`, `--space-*`, `--shadow-*`, `--radius-*`, `--duration-*`, `--ease-*`)
- `*, *::before, *::after` box-sizing reset
- `body` base styles
- `*:focus-visible` outline styles
- `@media (prefers-reduced-motion: reduce)`
- `@media (prefers-contrast: high)`
- Custom scrollbar styles (if they use tokens)

**Step 2: Delete global utility classes**

Remove the following class definitions entirely:
- `.btn` and all variants (`.btn-primary`, `.btn-secondary`, `.btn-ghost`, `.btn-danger`)
- `.btn-sm`, `.btn-md`, `.btn-lg`
- `.btn.icon-only` and size variants
- `.card`, `.card-interactive`
- `.card-header`, `.card-body`, `.card-footer`
- `.input`, `.input:focus`, `.input:disabled`
- `.label`, `.form-hint`
- `.badge` and all variants (`.badge-success`, `.badge-warning`, `.badge-danger`, `.badge-info`)
- `.table`, `.table th`, `.table td`, `.table-container`
- `.skeleton` and shimmer keyframes (keep keyframes if used by Skeleton.svelte, move to component)
- `.page-container`, `.page-header`, `.page-title`, `.section-title`
- `.grid-cards`, `.grid-stats`
- Any other component-specific utility classes not listed above

**Step 3: Verify build**

```bash
cd ui && npx svelte-check
```
Expected: May show errors in components still using deleted classes. This is expected — we fix in Task 5.

**Step 4: Commit**

```bash
git add ui/src/app.css
git commit -m "refactor: strip app.css to pure design tokens, remove global utility classes"
```

---

### Task 5: P1 — Migrate primitive components to Tailwind

**Files:**
- Modify: `ui/src/lib/components/primitives/Button.svelte`
- Modify: `ui/src/lib/components/primitives/Input.svelte`
- Modify: `ui/src/lib/components/primitives/Card.svelte`
- Modify: `ui/src/lib/components/primitives/Badge.svelte`
- Modify: `ui/src/lib/components/primitives/Skeleton.svelte`

**Step 1: Button.svelte — migrate styles to Tailwind**

Replace the `<style>` block with Tailwind utility classes on the element:

```svelte
<button
  class="inline-flex items-center justify-center gap-2 rounded-sm border-none font-medium text-sm cursor-pointer transition-all duration-fast ease-default focus-visible:outline-2 focus-visible:outline-primary focus-visible:outline-offset-2 disabled:opacity-50 disabled:cursor-not-allowed"
  class:bg-gradient-to-br={variant === 'primary'}
  class:from-primary={variant === 'primary'}
  class:to-primary-active={variant === 'primary'}
  class:text-white={variant === 'primary' || variant === 'danger'}
  class:shadow-[0_2px_8px_var(--color-primary-glow)]={variant === 'primary'}
  class:hover:shadow-[0_4px_12px_rgba(143,90,42,0.35)]={variant === 'primary'}
  class:hover:-translate-y-px={variant === 'primary'}
  class:active:translate-y-0={variant === 'primary'}
  class:bg-white={variant === 'secondary'}
  class:text-neutral-700={variant === 'secondary'}
  class:border={variant === 'secondary'}
  class:border-neutral-300={variant === 'secondary'}
  class:bg-transparent={variant === 'ghost'}
  class:text-neutral-600={variant === 'ghost'}
  class:bg-danger={variant === 'danger'}
  class:p-1={isIconOnly && size === 'sm'}
  class:h-8={size === 'sm'}
  class:h-10={size === 'md'}
  class:h-12={size === 'lg'}
  class:text-base={size === 'lg'}
  ...
>
```

Keep the `<style>` block ONLY for:
- `@keyframes spin` (used by loading spinner)
- `.animate-spin` class
- `prefers-contrast: high` and `prefers-reduced-motion` media queries

**Step 2: Input.svelte — migrate to Tailwind**

Replace `<style>` with Tailwind utilities using `class="w-full py-2.5 px-3.5 text-sm border border-neutral-300 rounded-sm bg-white transition-all duration-fast ease-default hover:border-neutral-400 focus:outline-none focus:border-primary focus:shadow-[0_0_0_3px_var(--color-primary-glow)] disabled:bg-neutral-100 disabled:text-neutral-400 disabled:cursor-not-allowed"`

**Step 3: Card.svelte — migrate to Tailwind**

Replace `<style>` with Tailwind utilities using `class="bg-white border border-neutral-200 rounded-md shadow-sm transition-all duration-normal ease-default hover:shadow-md hover:border-primary/30"`

**Step 4: Badge.svelte — migrate to Tailwind**

Replace `<style>` with Tailwind utilities. Map variants to token-based classes:
- success: `bg-success/15 text-success-dark border border-success/20`
- warning: `bg-warning/15 text-warning-dark border border-warning/20`
- danger: `bg-danger/15 text-danger-dark border border-danger/20`
- default: `bg-neutral-100 text-neutral-600 border border-neutral-200`

**Step 5: Skeleton.svelte — migrate to Tailwind**

Keep the shimmer keyframe animation in `<style>` but replace structural styles with Tailwind: `class="bg-neutral-200 rounded animate-pulse"`. If the custom shimmer is different from `animate-pulse`, keep the keyframe but move the `.skeleton` class definition to the component's `<style>`.

**Step 6: Verify build**

```bash
cd ui && npx svelte-check
```
Expected: PASS (zero errors)

**Step 7: Commit**

```bash
git add ui/src/lib/components/primitives/
git commit -m "refactor: migrate primitives to Tailwind-first styling"
```

---

### Task 6: P1 — Migrate shell and system components to Tailwind

**Files:**
- Modify: `ui/src/lib/components/shell/AppShell.svelte`
- Modify: `ui/src/lib/components/shell/AppNav.svelte`
- Modify: `ui/src/lib/components/system/PageShell.svelte`
- Modify: `ui/src/lib/components/system/StateBanner.svelte`
- Modify: `ui/src/lib/components/EmptyState.svelte`
- Modify: `ui/src/lib/components/Modal.svelte`

**Step 1: AppShell.svelte**

Replace any usage of deleted global classes (`.page-container`, `.page-header`, etc.) with Tailwind equivalents. Verify no references to `.btn`, `.card`, `.input` remain.

**Step 2: AppNav.svelte**

Same — ensure all classes use Tailwind utilities or component-scoped `<style>`.

**Step 3: PageShell.svelte**

Check for `.page-title`, `.section-title` usage. Replace with Tailwind typography utilities.

**Step 4: StateBanner.svelte**

Check for `.badge-*` usage. Import and use the `Badge` primitive component instead of raw badge classes.

**Step 5: EmptyState.svelte**

Replace any deleted global class references. The current file uses `padding: 3rem 1.5rem` which is fine as component-scoped style, but consider using token values: `padding: var(--space-12) var(--space-6)`.

**Step 6: Modal.svelte**

Already uses mostly Tailwind. Verify no deleted global classes are referenced.

**Step 7: Verify build**

```bash
cd ui && npx svelte-check
```
Expected: PASS

**Step 8: Commit**

```bash
git add ui/src/lib/components/shell/ ui/src/lib/components/system/ ui/src/lib/components/EmptyState.svelte ui/src/lib/components/Modal.svelte
git commit -m "refactor: migrate shell and system components to Tailwind-first"
```

---

### Task 7: P1 — Reorganize components into feature folders

**Files:**
- Create/move: `ui/src/lib/components/data-display/`
- Create/move: `ui/src/lib/components/vms/`
- Create/move: `ui/src/lib/components/nodes/`
- Create/move: `ui/src/lib/components/modals/`
- Create/move: `ui/src/lib/components/feedback/`
- Create/move: `ui/src/lib/components/navigation/`
- Create/move: `ui/src/lib/components/forms/`
- Create/move: `ui/src/lib/components/charts/`
- Modify: All import statements in routes and components

**Step 1: Create directories**

```bash
cd ui/src/lib/components
mkdir -p data-display vms nodes modals feedback navigation forms charts
```

**Step 2: Move files**

| Source | Destination |
|--------|-------------|
| `DataTable.svelte` | `data-display/DataTable.svelte` |
| `SkeletonRow.svelte` | `data-display/SkeletonRow.svelte` |
| `ResourceTable.svelte` | `data-display/ResourceTable.svelte` |
| `VMCard.svelte` | `vms/VMCard.svelte` |
| `VMPowerMenu.svelte` | `vms/VMPowerMenu.svelte` |
| `VMMetricsWidget.svelte` | `vms/VMMetricsWidget.svelte` |
| `VMMetricsHistory.svelte` | `vms/VMMetricsHistory.svelte` |
| `VMSnapshotsPanel.svelte` | `vms/VMSnapshotsPanel.svelte` |
| `VMTimeline.svelte` | `vms/VMTimeline.svelte` |
| `NodeHealthStatus.svelte` | `nodes/NodeHealthStatus.svelte` |
| `Modal.svelte` | `modals/Modal.svelte` |
| `ConfirmAction.svelte` | `modals/ConfirmAction.svelte` |
| `CreateVMModal.svelte` | `modals/CreateVMModal.svelte` |
| `DeleteVMModal.svelte` | `modals/DeleteVMModal.svelte` |
| `CreateNetworkModal.svelte` | `modals/CreateNetworkModal.svelte` |
| `CreateStoragePoolModal.svelte` | `modals/CreateStoragePoolModal.svelte` |
| `ImportImageModal.svelte` | `modals/ImportImageModal.svelte` |
| `QuotaSettingsModal.svelte` | `modals/QuotaSettingsModal.svelte` |
| `AddNodeModal.svelte` | `modals/AddNodeModal.svelte` |
| `SearchModal.svelte` | `modals/SearchModal.svelte` |
| `EmptyState.svelte` | `feedback/EmptyState.svelte` |
| `Toast.svelte` | `feedback/Toast.svelte` |
| `ToastContainer.svelte` | `feedback/ToastContainer.svelte` |
| `StateBanner.svelte` | `feedback/StateBanner.svelte` |
| `InstallStatusPanel.svelte` | `feedback/InstallStatusPanel.svelte` |
| `MobileNav.svelte` | `navigation/MobileNav.svelte` |
| `TreeNavigation.svelte` | `navigation/TreeNavigation.svelte` |
| `SkipLink.svelte` | `navigation/SkipLink.svelte` |
| `CommandBar.svelte` | `shell/CommandBar.svelte` |
| `FormField.svelte` | `forms/FormField.svelte` |
| `CloudInitEditor.svelte` | `forms/CloudInitEditor.svelte` |
| `CloudInitPreview.svelte` | `forms/CloudInitPreview.svelte` |
| `CloudInitViewer.svelte` | `forms/CloudInitViewer.svelte` |
| `ChartJS.svelte` | `charts/ChartJS.svelte` |
| `MetricsChart.svelte` | `charts/MetricsChart.svelte` |
| `MetricsChartEnhanced.svelte` | `charts/MetricsChartEnhanced.svelte` |
| `Sparkline.svelte` | `charts/Sparkline.svelte` |

Components that don't clearly fit a feature folder (e.g., `Terminal.svelte`, `FirewallRuleEditor.svelte`, `StatsCard.svelte`, `ProgressBar.svelte`) can stay at the root of `lib/components/` for now or be placed in a `shared/` folder.

**Step 3: Update barrel exports (index.ts files)**

Update `ui/src/lib/components/primitives/index.ts` to ensure it still exports correctly.

Create or update `ui/src/lib/components/system/index.ts` if it references moved components.

**Step 4: Update imports across the codebase**

Use grep to find all imports of moved components and update paths:

```bash
cd ui/src
grep -rn "from '\$lib/components/DataTable" routes/ lib/ || true
grep -rn "from '\$lib/components/VMCard" routes/ lib/ || true
# ... repeat for each moved component
```

Update each import to the new path, e.g.:
```ts
// Before
import DataTable from '$lib/components/DataTable.svelte';

// After
import DataTable from '$lib/components/data-display/DataTable.svelte';
```

**Step 5: Verify build**

```bash
cd ui && npx svelte-check
```
Expected: PASS (zero errors — all imports resolved)

**Step 6: Commit**

```bash
git add ui/src/lib/components/
git commit -m "refactor: reorganize components into feature folders"
```

---

### Task 8: P1 — Extract overview page logic to helpers

**Files:**
- Create: `ui/src/lib/webui/overview-helpers.ts`
- Modify: `ui/src/routes/+page.svelte`

**Step 1: Create overview-helpers.ts**

Extract these pure functions from `+page.svelte`:

```typescript
// ui/src/lib/webui/overview-helpers.ts
import type { ShellTone } from '$lib/shell/app-shell';

export function severityTone(severity: string): ShellTone {
  switch (severity) {
    case 'critical': return 'failed';
    case 'warning': return 'warning';
    default: return 'unknown';
  }
}

export function statusTone(status: string): ShellTone {
  switch (status) {
    case 'running': return 'warning';
    case 'failed': return 'failed';
    case 'succeeded': return 'healthy';
    default: return 'unknown';
  }
}

export function formatTimeAgo(ms: number): string {
  const seconds = Math.max(Math.round((Date.now() - ms) / 1000), 0);
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.round(hours / 24)}d ago`;
}
```

**Step 2: Extract derived state builders**

```typescript
// ui/src/lib/webui/overview-derive.ts
import type { OverviewData } from '$lib/bff/types'; // or wherever the type lives

export interface PostureChip {
  label: string;
  value: number;
  variant?: 'degraded' | 'warning' | 'failed' | 'healthy';
}

export function buildPostureChips(overview: OverviewData): PostureChip[] {
  return [
    { label: 'Clusters', value: overview.clusters_total },
    { label: 'Nodes', value: overview.nodes_total },
    { label: 'VMs running', value: overview.vms_running },
    {
      label: 'Degraded',
      value: overview.clusters_degraded + overview.nodes_degraded,
      variant: overview.clusters_degraded + overview.nodes_degraded > 0 ? 'degraded' : undefined
    },
    {
      label: 'Tasks',
      value: overview.active_tasks,
      variant: overview.active_tasks > 0 ? 'warning' : undefined
    },
    {
      label: 'Alerts',
      value: overview.unresolved_alerts,
      variant: overview.unresolved_alerts > 0 ? 'failed' : undefined
    }
  ];
}

export interface AttentionItem {
  type: 'cluster' | 'node' | 'alert';
  title: string;
  detail: string;
  href: string;
}

export function buildAttentionItems(overview: OverviewData): AttentionItem[] {
  const items: AttentionItem[] = [
    ...(overview.clusters_degraded > 0 ? [{
      type: 'cluster' as const,
      title: `${overview.clusters_degraded} cluster${overview.clusters_degraded === 1 ? '' : 's'} degraded`,
      detail: 'Review cluster posture for pressure or version skew.',
      href: '/clusters'
    }] : []),
    ...(overview.nodes_degraded > 0 ? [{
      type: 'node' as const,
      title: `${overview.nodes_degraded} node${overview.nodes_degraded === 1 ? '' : 's'} degraded`,
      detail: 'Check node readiness and capacity pressure.',
      href: '/nodes'
    }] : []),
    ...(overview.unresolved_alerts > 0 ? [{
      type: 'alert' as const,
      title: `${overview.unresolved_alerts} unresolved alert${overview.unresolved_alerts === 1 ? '' : 's'}`,
      detail: 'Alerts require operator inspection or acknowledgement.',
      href: '/events'
    }] : [])
  ];
  return items.slice(0, 4);
}
```

**Step 3: Refactor +page.svelte**

Replace the inline function definitions and derived computations with imports:

```svelte
<script lang="ts">
  import { severityTone, statusTone, formatTimeAgo } from '$lib/webui/overview-helpers';
  import { buildPostureChips, buildAttentionItems } from '$lib/webui/overview-derive';
  // ... rest of imports

  let { data }: { data: PageData } = $props();
  const page = getPageDefinition('/');
  const overview = $derived(data.overview);
  const postureChips = $derived(buildPostureChips(overview));
  const attentionItems = $derived(buildAttentionItems(overview));
</script>
```

Remove the old inline `severityTone`, `statusTone`, `formatTimeAgo`, `postureChips`, and `attentionItems` definitions.

**Step 4: Verify build**

```bash
cd ui && npx svelte-check
```
Expected: PASS

**Step 5: Commit**

```bash
git add ui/src/lib/webui/overview-helpers.ts ui/src/lib/webui/overview-derive.ts ui/src/routes/+page.svelte
git commit -m "refactor: extract overview page logic to helper modules"
```

---

### Task 9: P1 — Split DataTable into focused subcomponents

**Files:**
- Create: `ui/src/lib/components/data-display/useTableSelection.ts`
- Create: `ui/src/lib/components/data-display/useTableSorting.ts`
- Create: `ui/src/lib/components/data-display/ColumnVisibilityDropdown.svelte`
- Modify: `ui/src/lib/components/data-display/DataTable.svelte`

**Step 1: Create useTableSelection.ts**

Extract selection logic from DataTable:

```typescript
// useTableSelection.ts
export interface UseTableSelectionOptions<T> {
  data: T[];
  rowId: (row: T) => string;
  selectedIds: string[];
  onSelect?: (ids: string[]) => void;
}

export interface UseTableSelectionResult {
  isRowSelected: (rowId: string) => boolean;
  isAllSelected: boolean;
  isIndeterminate: boolean;
  toggleRow: (rowId: string) => void;
  toggleAll: () => void;
  handleRowClick: (rowId: string, event: MouseEvent) => void;
}

export function useTableSelection<T>(options: UseTableSelectionOptions<T>): UseTableSelectionResult {
  // Extract logic from DataTable lines 94-170 (selection handling)
  // ... implementation
}
```

**Step 2: Create useTableSorting.ts**

Extract sorting logic:

```typescript
// useTableSorting.ts
export interface UseTableSortingOptions {
  sortColumn: string | null;
  sortDirection: 'asc' | 'desc' | null;
  onSort?: (column: string, direction: 'asc' | 'desc' | null) => void;
}

export interface UseTableSortingResult {
  handleSort: (columnKey: string) => void;
  getSortIcon: (columnKey: string) => 'asc' | 'desc' | null;
}

export function useTableSorting(options: UseTableSortingOptions): UseTableSortingResult {
  // Extract logic from DataTable sort handling
}
```

**Step 3: Create ColumnVisibilityDropdown.svelte**

Extract the column visibility toggle UI from DataTable:

```svelte
<script lang="ts">
  interface Props {
    columns: Array<{ key: string; title: string }>;
    visibleColumns: Set<string>;
    onToggle: (key: string) => void;
  }
  let { columns, visibleColumns, onToggle }: Props = $props();
</script>

<!-- Extracted from DataTable lines ~170-220 -->
```

**Step 4: Refactor DataTable.svelte**

Import the extracted modules. Remove the inline selection/sorting/visibility logic. DataTable should focus on:
- Rendering the table structure
- Column header rendering
- Row rendering
- Delegating selection, sorting, and visibility to the extracted modules

Target size: under 350 lines.

**Step 5: Verify build**

```bash
cd ui && npx svelte-check
```
Expected: PASS

**Step 6: Commit**

```bash
git add ui/src/lib/components/data-display/
git commit -m "refactor: split DataTable into selection, sorting, and visibility submodules"
```

---

### Task 10: Final verification

**Step 1: Run full type check**

```bash
cd ui && npx svelte-check --tsconfig ./tsconfig.json
```
Expected: zero errors, zero warnings

**Step 2: Run build**

```bash
cd ui && npm run build
```
Expected: clean build with no errors

**Step 3: Run existing tests**

```bash
cd ui && npm test
```
Expected: All existing tests pass (20 test files)

**Step 4: Final commit if any fixes needed**

If svelte-check or build caught issues, fix them and commit:

```bash
git add -A
git commit -m "fix: resolve type errors after refactor"
```

---

## Completion Checklist

- [ ] DESIGN.md matches app.css tokens
- [ ] No hardcoded colors in any component
- [ ] app.css contains only tokens and base resets
- [ ] All primitive components use Tailwind utilities
- [ ] All shell/system components use Tailwind utilities
- [ ] Components organized in feature folders
- [ ] All imports updated and resolving
- [ ] Overview page logic extracted to helpers
- [ ] DataTable split into submodules
- [ ] `svelte-check` passes
- [ ] `npm run build` succeeds
- [ ] `npm test` passes
