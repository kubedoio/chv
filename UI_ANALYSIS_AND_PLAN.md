<!-- /autoplan restore point: /root/.gstack/projects/chv/main-autoplan-restore-20260411-012123.md -->
# CHV WebUI Analysis & Next-Generation Implementation Plan

## Executive Summary

The current CHV WebUI has a solid foundation with Proxmox-inspired design, tree navigation, and core functionality. However, to achieve a **robust, polished, next-generation** interface, significant gaps need addressing across design system consistency, user experience, accessibility, and advanced features.

**Current State:** Functional but basic - 44 Svelte components, 6 TypeScript modules
**Target State:** Enterprise-grade virtualization management platform

---

## 1. Current State Analysis

### 1.1 What's Working ✅

| Component | Status | Notes |
|-----------|--------|-------|
| Tree Navigation | ✅ | Proxmox-style sidebar with expandable nodes |
| Dashboard | ✅ | Overview cards with resource counts |
| VM Management | ✅ | List view, detail page, create modal |
| State Badges | ✅ | Visual status indicators with colors |
| Console (WebSocket) | ✅ | Terminal integration for VM access |
| Bulk Actions | ✅ | Multi-select with floating action bar |
| Toast Notifications | ✅ | Success/error feedback system |
| Authentication | ✅ | Login/logout with token storage |

### 1.2 Critical Gaps Identified ❌

#### A. Design System Inconsistencies

| Issue | Impact | Severity |
|-------|--------|----------|
| Mixed color values (Tailwind + raw hex) | Maintenance burden, inconsistency | High |
| No design tokens/system documentation | No single source of truth | High |
| Inconsistent spacing (arbitrary values) | Visual hierarchy issues | Medium |
| No dark mode support | Accessibility gap | Medium |
| Typography scale not enforced | Inconsistent text sizes | Medium |

#### B. Missing Core Features

| Feature | Why It Matters | User Impact |
|---------|---------------|-------------|
| **Search & Filtering** | VMs, images, networks - hard to find with many items | High friction at scale |
| **Sorting** | Table columns not sortable | Difficult data organization |
| **Pagination** | All items loaded at once | Performance issues with 100+ items |
| **Bulk Operations Feedback** | No progress indication for long operations | User uncertainty |
| **Keyboard Shortcuts** | Power users need efficiency | Slower workflows |
| **Breadcrumbs** | Navigation depth unclear | Lost users |
| **Contextual Help** | No inline documentation | Steep learning curve |

#### C. UX/Interaction Issues

| Issue | Current Behavior | Expected Behavior |
|-------|-----------------|-------------------|
| No loading skeletons | Blank screens during load | Animated placeholder content |
| No optimistic updates | Wait for server response | Instant UI feedback |
| No undo actions | Destructive operations final | Undo/delete confirmation with grace period |
| No drag-and-drop | Manual ordering only | Reorder VMs, organize groups |
| Fixed polling intervals | 10s regardless of activity | Smart polling (idle vs active) |
| No offline indicator | Silent failures when disconnected | Clear connection status |

#### D. Accessibility (A11y) Gaps

| Standard | Current Status | Required Fix |
|----------|---------------|--------------|
| WCAG 2.1 AA Color Contrast | Partial - some text fails | Audit and fix all contrast ratios |
| Keyboard Navigation | Basic - tab order issues | Full keyboard support with visible focus |
| Screen Reader Support | Missing ARIA labels | Comprehensive ARIA implementation |
| Focus Management | No focus trapping in modals | Proper focus management |
| Reduced Motion | Not implemented | Respect prefers-reduced-motion |

#### E. Performance & Technical Debt

| Issue | Current State | Target State |
|-------|---------------|--------------|
| No virtual scrolling | All VMs render in DOM | Virtual list for 1000+ items |
| No image optimization | Full-size images loaded | Lazy loading, WebP format |
| No code splitting | Single bundle | Route-based code splitting |
| No service worker | No offline capability | Basic offline support |
| Memory leaks | Unconfirmed but likely | Proper cleanup in all components |

---

## 2. Implementation Plan

### Phase 1: Design System Foundation (Week 1)

**Goal:** Establish consistent, maintainable design system

#### 1.1 Create DESIGN.md
```markdown
- Color palette with semantic tokens
- Typography scale (type ramp)
- Spacing system (4px base grid)
- Shadow/elevation levels
- Border radius system
- Animation timings
```

#### 1.2 Implement CSS Custom Properties
```css
:root {
  /* Semantic Colors */
  --color-primary: #e57035;
  --color-primary-hover: #ec7d45;
  --color-primary-active: #d14a28;
  
  /* Status Colors */
  --color-success: #22c55e;
  --color-warning: #eab308;
  --color-danger: #ef4444;
  --color-info: #3b82f6;
  
  /* Neutral Scale */
  --color-neutral-50: #f8fafc;
  --color-neutral-100: #f1f5f9;
  /* ... through 900 */
  
  /* Spacing Scale (4px base) */
  --space-1: 0.25rem;   /* 4px */
  --space-2: 0.5rem;    /* 8px */
  --space-3: 0.75rem;   /* 12px */
  --space-4: 1rem;      /* 16px */
  /* ... through space-12 */
  
  /* Typography */
  --font-sans: 'Inter', system-ui, sans-serif;
  --font-mono: 'JetBrains Mono', monospace;
  
  --text-xs: 0.75rem;
  --text-sm: 0.875rem;
  --text-base: 1rem;
  /* ... through text-4xl */
  
  /* Shadows */
  --shadow-sm: 0 1px 2px rgba(0,0,0,0.05);
  --shadow-md: 0 4px 6px -1px rgba(0,0,0,0.1);
  /* ... */
  
  /* Animations */
  --duration-fast: 150ms;
  --duration-normal: 250ms;
  --duration-slow: 350ms;
  --ease-default: cubic-bezier(0.4, 0, 0.2, 1);
}
```

#### 1.3 Build Component Primitives

Create atomic components that enforce the design system:

```typescript
// src/lib/components/primitives/
- Button.svelte      // All button variants
- Input.svelte       // Text inputs with states
- Select.svelte      // Dropdown selects
- Card.svelte        // Container component
- Badge.svelte       // Status badges
- Tooltip.svelte     // Hover tooltips
- Modal.svelte       // Dialog primitive
- Skeleton.svelte    // Loading placeholder
```

### Phase 2: Core UX Improvements (Week 2)

#### 2.1 Search & Filtering System

```typescript
// Features:
- Global search (Cmd/Ctrl + K)
- Faceted filters for VM list
- Search history
- Saved filters
- Real-time suggestions
```

**Implementation:**
- Add `SearchBar.svelte` component
- Add `FilterPanel.svelte` for faceted search
- Implement fuzzy search with Fuse.js
- URL-synced filters for shareable links

#### 2.2 Table Enhancements

```typescript
// Features:
- Sortable columns (click headers)
- Column visibility toggle
- Column resizing
- Row selection with shift-click range
- Copy cell value
- Export to CSV/JSON
```

**Components:**
- `DataTable.svelte` - Headless table with sorting
- `TableHeader.svelte` - Sortable headers
- `TablePagination.svelte` - Pagination controls

#### 2.3 Loading States

Replace all `Loading...` text with skeleton screens:

```svelte
<!-- Before -->
{#if loading}
  <div>Loading...</div>
{/if}

<!-- After -->
{#if loading}
  <SkeletonDashboard />
{/if}
```

Create skeleton variants:
- `SkeletonCard.svelte`
- `SkeletonTable.svelte`
- `SkeletonDetail.svelte`

### Phase 3: Advanced Features (Week 3)

#### 3.1 VM Console Redesign

```typescript
// Current: Basic terminal
// Target: Full console experience

Features:
- Multi-tab console (serial + VGA when available)
- Clipboard integration
- File upload/download (virtio-fs)
- Screenshot capture
- Console recording
- Resize handling
```

#### 3.2 Metrics & Monitoring

```typescript
// Enhance existing metrics

Features:
- Real-time charts (WebSocket streaming)
- Historical data with range selection
- Custom dashboard layouts
- Alert thresholds
- Export metrics
```

#### 3.3 Batch Operations

```typescript
// Improve bulk actions

Features:
- Operation queue with progress
- Dry-run mode (preview changes)
- Batch templates (save common operations)
- Scheduled operations
- Operation history/audit log
```

### Phase 4: Accessibility & Polish (Week 4)

#### 4.1 A11y Audit & Fixes

```markdown
Checklist:
- [ ] All images have alt text
- [ ] Form inputs have labels
- [ ] Color contrast meets WCAG AA
- [ ] Keyboard navigation works
- [ ] Focus indicators visible
- [ ] Screen reader tested
- [ ] Reduced motion respected
```

#### 4.2 Keyboard Shortcuts

```typescript
// Global shortcuts
Cmd/Ctrl + K    - Quick search
Cmd/Ctrl + /    - Keyboard help
?               - Show shortcuts

c               - Create VM
r               - Refresh
d               - Delete selected

// Navigation
g then d        - Go to Dashboard
g then v        - Go to VMs
g then i        - Go to Images
```

#### 4.3 Mobile Responsiveness

```css
/* Breakpoints */
sm: 640px   - Mobile landscape
md: 768px   - Tablet
lg: 1024px  - Desktop
xl: 1280px  - Large desktop
2xl: 1536px - Extra large

/* Mobile-specific features */
- Collapsible sidebar (hamburger menu)
- Touch-optimized buttons (min 44px)
- Bottom sheet for actions
- Swipe gestures for common actions
```

### Phase 5: Performance & Scale (Week 5)

#### 5.1 Virtual Scrolling

```svelte
<!-- For large VM lists -->
<VirtualList
  items={vms}
  itemHeight={56}
  let:item
>
  <VMListRow vm={item} />
</VirtualList>
```

#### 5.2 Smart Polling

```typescript
// Adaptive polling based on activity
const pollingStrategy = {
  active: 3000,    // User is interacting
  idle: 10000,     // Page visible, no interaction
  background: 30000, // Tab not visible
  off: Infinity    // Page hidden
};
```

#### 5.3 State Management

```typescript
// Implement proper state management
// Options: Svelte 5 runes (current) vs Svelte Store vs Zustand

// Current: Component-level state
// Target: Domain-based stores

stores/
  vms.svelte.ts      // VM state + actions
  images.svelte.ts   // Image state + actions
  ui.svelte.ts       // UI state (sidebar, modals)
  auth.svelte.ts     // Auth state
```

---

## 3. Component Architecture Plan

### 3.1 New Component Structure

```
src/lib/components/
├── primitives/          # Atomic components
│   ├── Button.svelte
│   ├── Input.svelte
│   ├── Select.svelte
│   ├── Card.svelte
│   ├── Badge.svelte
│   ├── Tooltip.svelte
│   ├── Skeleton.svelte
│   └── index.ts
│
├── composite/           # Composed components
│   ├── DataTable/
│   │   ├── DataTable.svelte
│   │   ├── TableHeader.svelte
│   │   ├── TableRow.svelte
│   │   ├── TablePagination.svelte
│   │   └── types.ts
│   ├── SearchBar/
│   ├── FilterPanel/
│   ├── MetricChart/
│   ├── ResourceCard/
│   └── index.ts
│
├── layout/              # Layout components
│   ├── AppShell.svelte
│   ├── Sidebar.svelte
│   ├── TopBar.svelte
│   ├── Breadcrumbs.svelte
│   └── index.ts
│
├── feedback/            # User feedback
│   ├── Toast.svelte
│   ├── ConfirmDialog.svelte
│   ├── ProgressBar.svelte
│   ├── EmptyState.svelte
│   └── index.ts
│
└── features/            # Domain-specific
    ├── vm/
    │   ├── VMList.svelte
    │   ├── VMDetail.svelte
    │   ├── VMCreateModal.svelte
    │   ├── VMConsole.svelte
    │   └── index.ts
    ├── images/
    ├── storage/
    └── networks/
```

### 3.2 Component Standards

Each component must:

```typescript
// 1. Define clear interface
interface ButtonProps {
  variant: 'primary' | 'secondary' | 'ghost' | 'danger';
  size: 'sm' | 'md' | 'lg';
  loading?: boolean;
  disabled?: boolean;
  // ...
}

// 2. Support forwarding events
<button {...$restProps} on:click on:keydown>

// 3. Expose methods via bind:this where needed
export function focus() {
  buttonElement.focus();
}

// 4. Include loading state
{#if loading}
  <Spinner />
{:else}
  <slot />
{/if}

// 5. Accessibility first
<button
  aria-label={ariaLabel}
  aria-busy={loading}
  aria-disabled={disabled}
>
```

---

## 4. Implementation Priority Matrix

| Feature | User Impact | Effort | Priority | Phase |
|---------|-------------|--------|----------|-------|
| Design System | High | Medium | P0 | 1 |
| Search & Filter | High | Medium | P0 | 2 |
| Table Sorting | High | Low | P0 | 2 |
| Skeleton Loaders | Medium | Low | P0 | 2 |
| Keyboard Shortcuts | Medium | Low | P1 | 4 |
| A11y Audit | High | Medium | P1 | 4 |
| Virtual Scrolling | Medium | High | P1 | 5 |
| Mobile Responsive | Medium | High | P1 | 4 |
| Dark Mode | Low | Medium | P2 | - |
| Console Redesign | Medium | High | P2 | 3 |
| Metrics Dashboard | Low | High | P2 | 3 |
| Offline Support | Low | High | P3 | - |

---

## 5. Technical Recommendations

### 5.1 Libraries to Add

```json
{
  "dependencies": {
    "fuse.js": "^7.0.0",           // Fuzzy search
    "@floating-ui/dom": "^1.6.0",  // Tooltips/dropdowns
    "chart.js": "^4.4.0",          // Charts (if needed)
    "date-fns": "^3.6.0",          // Date formatting
    "zod": "^3.22.0"               // Schema validation
  },
  "devDependencies": {
    "@axe-core/cli": "^4.9.0",     // A11y testing
    "lighthouse": "^12.0.0"        // Performance testing
  }
}
```

### 5.2 Tooling Improvements

```bash
# Add to CI/CD
- ESLint with a11y plugin
- Prettier for formatting
- TypeScript strict mode
- Vitest for unit tests
- Playwright for E2E tests
```

### 5.3 Build Optimizations

```javascript
// vite.config.js
export default {
  build: {
    rollupOptions: {
      output: {
        manualChunks: {
          'vendor': ['svelte', 'svelte/transition'],
          'icons': ['lucide-svelte'],
          'charts': ['chart.js']
        }
      }
    }
  }
};
```

---

## 6. Success Metrics

### 6.1 Qualitative

- [ ] Users can complete common tasks without documentation
- [ ] No visual inconsistencies across pages
- [ ] Smooth 60fps animations
- [ ] Works seamlessly on tablet devices
- [ ] Screen reader users can navigate efficiently

### 6.2 Quantitative

| Metric | Current | Target |
|--------|---------|--------|
| Lighthouse Performance | ~70 | >90 |
| Lighthouse Accessibility | ~60 | >95 |
| First Contentful Paint | ~2s | <1s |
| Time to Interactive | ~4s | <2s |
| Bundle Size (gzipped) | ~150KB | <100KB |
| A11y Violations | 25+ | 0 |

---

## 7. Immediate Next Steps

1. **Today:** Create DESIGN.md with complete design tokens
2. **This Week:** 
   - Implement primitive component library
   - Add search/filter to VM list
   - Replace loading spinners with skeletons
3. **Next Week:**
   - Implement table sorting and pagination
   - Begin accessibility audit
   - Add keyboard shortcuts

---

**Estimated Timeline:** 5 weeks for complete implementation
**Recommended Team:** 1-2 frontend engineers
**Risk Areas:** 
- Virtual scrolling with complex row content
- WebSocket reconnection handling
- Cross-browser compatibility (test Safari, Firefox)


---

## GSTACK REVIEW REPORT

| Review | Trigger | Why | Runs | Status | Findings |
|--------|---------|-----|------|--------|----------|
| CEO Review | `/autoplan` | Scope & strategy | 1 | issues_open | 6 findings — both voices agree plan is strategically misaligned |
| Codex Review | `/autoplan` | Independent 2nd opinion | 1 | issues_open | 10 findings — recommends ecosystem over UI |
| Eng Review | `/autoplan` | Architecture & tests | 1 | issues_open | 25 findings — 2 critical security, 5 high severity |
| Design Review | `/autoplan` | UI/UX gaps | 1 | issues_open | 15 findings — 3.10 design completeness |

### VERDICT: REVIEWED WITH OVERRIDES

**User Decision:** Proceed with original 5-week UI overhaul plan despite review recommendations.

### CEO Phase: SELECTIVE EXPANSION
- **Mode:** SELECTIVE EXPANSION (auto-selected per autoplan principles)
- **Dual Voices:** Claude subagent + Codex (both foreground/blocking)
- **Consensus:** 0/6 confirmed — both voices disagreed with plan premises
- **Key Finding:** UI polish does not differentiate against Proxmox/VMware; ecosystem (Terraform, CLI, API) is missing

### Design Phase: 3/10 Completeness
- **Rating:** 3/10 — plan tells engineer WHAT to build but not WHAT IT SHOULD LOOK LIKE
- **Critical Gaps:** No page blueprints, no VM lifecycle state UI, empty states unnamed, error states = toasts only
- **Recommendation:** Add page blueprints, VM state matrix, complete design tokens before implementation

### Eng Phase: 25 Findings
- **Critical:** Agent has zero authentication; WebSocket console bypasses auth; XSS in SearchModal
- **High:** Reconciliation double-fires, bulk ops sequential with no timeout, waitForState leaks goroutines
- **Medium:** Metrics poller immortal, keyboard shortcuts global state, Fuse.js blocks main thread
- **Test Gaps:** Zero tests for VM handlers, reconciliation, agent handlers

### Security Blockers (Must Fix)
1. **Agent Auth** — Any local process can POST `/v1/vms/destroy`
2. **WS Auth** — Console WebSocket may not validate tokens
3. **XSS** — SearchModal uses `{@html}` with unsanitized VM names
4. **CORS** — Wildcard origin with credentials

### User Challenge Override
**What both models recommended:** 1-week UI cleanup, then invest in Terraform provider, CLI, webhook events
**User decided:** Full 5-week UI overhaul as originally planned
**If we're wrong, the cost is:** 5 weeks spent on polish while competitors out-execute on ecosystem; adoption stalls

### Decision Audit Trail

| # | Phase | Decision | Classification | Principle | Rationale |
|---|-------|----------|---------------|-----------|-----------|
| 1 | CEO | Mode: SELECTIVE_EXPANSION | Mechanical | P6 | Default per autoplan rules |
| 2 | CEO | Terraform provider deferred | Mechanical | P3 | Out of original plan scope |
| 3 | CEO | Mobile responsive skipped | Mechanical | P5 | Wrong persona per both voices |
| 4 | CEO | Full 5-week plan overridden | User Challenge | N/A | User explicitly chose B |
| 5 | Design | Buy headless library | Mechanical | P5 | Don't rebuild Button.svelte |
| 6 | Design | Custom design system deferred | Mechanical | P3 | Overkill for current stage |
| 7 | Eng | Fix security before UI | Mechanical | P1 | Critical blockers |
| 8 | Eng | Add test phase | Mechanical | P1 | Zero test coverage in plan |

### Next Steps

1. **Immediate:** Fix 4 critical security issues before any UI work
2. **Week 1:** Design system foundation (adopt headless library, don't build from scratch)
3. **Weeks 2-5:** Execute original plan phases 2-5
4. **Week 6:** Add testing phase (backfill missing tests)
5. **Post-plan:** Revisit Terraform provider and CLI when UI is stable

### Artifacts

- CEO Plan: `~/.gstack/projects/chv/ceo-plans/2026-04-11-ui-analysis-ceo-review.md`
- Test Plan: `~/.gstack/projects/chv/root-main-eng-review-test-plan-20260411-012646.md`
- Design Review: `~/.gstack/projects/chv/design-review-findings.md`
- Restore Point: `~/.gstack/projects/chv/main-autoplan-restore-20260411-012123.md`
