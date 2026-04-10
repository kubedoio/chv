# CHV Design System

A comprehensive design system for the CHV (Cloud Hypervisor Virtualization) platform.

## Design Principles

1. **Clarity First** - Every element communicates its purpose
2. **Efficiency** - Minimize clicks for common tasks
3. **Consistency** - Same patterns across the entire interface
4. **Accessibility** - WCAG 2.1 AA compliant by default
5. **Performance** - 60fps animations, instant feedback

## Color System

### Primary Colors (Proxmox Orange)

| Token | Hex | HSL | Usage |
|-------|-----|-----|-------|
| `--color-primary` | #e57035 | 17 75% 55% | Primary buttons, links, accents |
| `--color-primary-hover` | #ec7d45 | 20 76% 59% | Hover states |
| `--color-primary-active` | #d14a28 | 15 70% 48% | Active/pressed states |
| `--color-primary-light` | #fff5f0 | 25 100% 97% | Background tints |
| `--color-primary-dark` | #b93d20 | 12 70% 43% | Dark accents |

### Semantic Colors

| Token | Hex | Usage |
|-------|-----|-------|
| `--color-success` | #22c55e | Running, ready, success states |
| `--color-success-light` | #dcfce7 | Success backgrounds |
| `--color-warning` | #eab308 | Warning, transitioning states |
| `--color-warning-light` | #fef9c3 | Warning backgrounds |
| `--color-danger` | #ef4444 | Error, stopped, failed states |
| `--color-danger-light` | #fee2e2 | Error backgrounds |
| `--color-info` | #3b82f6 | Information, neutral states |
| `--color-info-light` | #dbeafe | Info backgrounds |

### Neutral Scale

| Token | Hex | Usage |
|-------|-----|-------|
| `--color-neutral-50` | #f8fafc | Page backgrounds |
| `--color-neutral-100` | #f1f5f9 | Card backgrounds |
| `--color-neutral-200` | #e2e8f0 | Borders, dividers |
| `--color-neutral-300` | #cbd5e1 | Disabled states |
| `--color-neutral-400` | #94a3b8 | Placeholder text |
| `--color-neutral-500` | #64748b | Secondary text |
| `--color-neutral-600` | #475569 | Body text |
| `--color-neutral-700` | #334155 | Headings |
| `--color-neutral-800` | #1e293b | Strong emphasis |
| `--color-neutral-900` | #0f172a | Primary text |

### Dark Theme (Sidebar)

| Token | Hex | Usage |
|-------|-----|-------|
| `--color-sidebar-bg` | #252532 | Sidebar background |
| `--color-sidebar-dark` | #1e1e28 | Header/footer sections |
| `--color-sidebar-border` | #2a2a38 | Borders in sidebar |
| `--color-sidebar-text` | #94a3b8 | Inactive menu items |
| `--color-sidebar-text-active` | #ffffff | Active menu items |

## Typography

### Font Families

```css
--font-sans: 'Inter', system-ui, -apple-system, BlinkMacSystemFont, sans-serif;
--font-mono: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace;
```

### Type Scale

| Token | Size | Line Height | Weight | Usage |
|-------|------|-------------|--------|-------|
| `--text-xs` | 0.75rem (12px) | 1rem | 400 | Captions, timestamps |
| `--text-sm` | 0.875rem (14px) | 1.25rem | 400 | Body small, buttons |
| `--text-base` | 1rem (16px) | 1.5rem | 400 | Body text |
| `--text-lg` | 1.125rem (18px) | 1.75rem | 500 | Subheadings |
| `--text-xl` | 1.25rem (20px) | 1.75rem | 600 | Section titles |
| `--text-2xl` | 1.5rem (24px) | 2rem | 600 | Page titles |
| `--text-3xl` | 1.875rem (30px) | 2.25rem | 700 | Major headings |

### Typography Patterns

```css
/* Page Title */
.page-title {
  font-size: var(--text-2xl);
  font-weight: 600;
  color: var(--color-neutral-900);
  letter-spacing: -0.01em;
}

/* Section Title */
.section-title {
  font-size: var(--text-lg);
  font-weight: 600;
  color: var(--color-neutral-800);
}

/* Body Text */
.body-text {
  font-size: var(--text-base);
  font-weight: 400;
  color: var(--color-neutral-600);
  line-height: 1.5;
}

/* Label/Caption */
.label {
  font-size: var(--text-xs);
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--color-neutral-500);
}

/* Monospace (technical data) */
.mono {
  font-family: var(--font-mono);
  font-size: 0.875em;
}
```

## Spacing System

### Base Unit: 4px

| Token | Value | Pixels |
|-------|-------|--------|
| `--space-1` | 0.25rem | 4px |
| `--space-2` | 0.5rem | 8px |
| `--space-3` | 0.75rem | 12px |
| `--space-4` | 1rem | 16px |
| `--space-5` | 1.25rem | 20px |
| `--space-6` | 1.5rem | 24px |
| `--space-8` | 2rem | 32px |
| `--space-10` | 2.5rem | 40px |
| `--space-12` | 3rem | 48px |

### Spacing Patterns

```css
/* Card Padding */
.card-padding { padding: var(--space-5); }

/* Section Gap */
.section-gap { gap: var(--space-6); }

/* Element Gap */
.element-gap { gap: var(--space-3); }

/* Tight Gap */
tight-gap { gap: var(--space-2); }
```

## Shadows & Elevation

| Token | Value | Usage |
|-------|-------|-------|
| `--shadow-sm` | 0 1px 2px rgba(0,0,0,0.05) | Subtle depth, inputs |
| `--shadow-md` | 0 4px 6px -1px rgba(0,0,0,0.1) | Cards, dropdowns |
| `--shadow-lg` | 0 10px 15px -3px rgba(0,0,0,0.1) | Modals, popovers |
| `--shadow-xl` | 0 20px 25px -5px rgba(0,0,0,0.1) | Full-screen overlays |
| `--shadow-glow` | 0 0 20px rgba(229,112,53,0.3) | Primary focus states |

## Border Radius

| Token | Value | Usage |
|-------|-------|-------|
| `--radius-sm` | 0.25rem (4px) | Buttons, inputs, badges |
| `--radius-md` | 0.5rem (8px) | Cards, modals |
| `--radius-lg` | 0.75rem (12px) | Large cards, panels |
| `--radius-xl` | 1rem (16px) | Feature cards |
| `--radius-full` | 9999px | Pills, avatars |

## Animation

### Durations

| Token | Value | Usage |
|-------|-------|-------|
| `--duration-instant` | 0ms | No animation |
| `--duration-fast` | 150ms | Micro-interactions |
| `--duration-normal` | 250ms | Standard transitions |
| `--duration-slow` | 350ms | Page transitions |

### Easing Functions

| Token | Value | Usage |
|-------|-------|-------|
| `--ease-default` | cubic-bezier(0.4, 0, 0.2, 1) | Standard transitions |
| `--ease-in` | cubic-bezier(0.4, 0, 1, 1) | Exit animations |
| `--ease-out` | cubic-bezier(0, 0, 0.2, 1) | Enter animations |
| `--ease-bounce` | cubic-bezier(0.34, 1.56, 0.64, 1) | Playful interactions |

### Standard Transitions

```css
/* Interactive Elements */
.interactive {
  transition: 
    background-color var(--duration-fast) var(--ease-default),
    border-color var(--duration-fast) var(--ease-default),
    box-shadow var(--duration-fast) var(--ease-default),
    transform var(--duration-fast) var(--ease-default);
}

/* Cards */
.card {
  transition: 
    box-shadow var(--duration-normal) var(--ease-default),
    transform var(--duration-normal) var(--ease-default);
}

.card:hover {
  box-shadow: var(--shadow-lg);
  transform: translateY(-2px);
}
```

## Component Specifications

### Button

**Variants:**
- `primary` - Main actions
- `secondary` - Alternative actions
- `ghost` - Low emphasis
- `danger` - Destructive actions

**Sizes:**
- `sm` - 32px height (compact UI)
- `md` - 40px height (default)
- `lg` - 48px height (hero sections)

**States:**
- Default
- Hover
- Active/Pressed
- Focus (visible ring)
- Disabled
- Loading (with spinner)

```css
.button-primary {
  background: linear-gradient(135deg, var(--color-primary) 0%, var(--color-primary-active) 100%);
  color: white;
  border-radius: var(--radius-sm);
  padding: 0.625rem 1.25rem;
  font-weight: 500;
  transition: all var(--duration-fast) var(--ease-default);
  box-shadow: 0 2px 8px rgba(229, 112, 53, 0.3);
}

.button-primary:hover {
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(229, 112, 53, 0.4);
}

.button-primary:active {
  transform: translateY(0);
}

.button-primary:focus-visible {
  outline: 2px solid var(--color-primary);
  outline-offset: 2px;
}
```

### Card

```css
.card {
  background: white;
  border: 1px solid var(--color-neutral-200);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-sm);
  transition: all var(--duration-normal) var(--ease-default);
}

.card:hover {
  box-shadow: var(--shadow-md);
  border-color: rgba(229, 112, 53, 0.3);
}
```

### Input

```css
.input {
  width: 100%;
  padding: 0.625rem 0.875rem;
  font-size: var(--text-sm);
  border: 1px solid var(--color-neutral-300);
  border-radius: var(--radius-sm);
  background: white;
  transition: all var(--duration-fast) var(--ease-default);
}

.input:hover {
  border-color: var(--color-neutral-400);
}

.input:focus {
  outline: none;
  border-color: var(--color-primary);
  box-shadow: 0 0 0 3px rgba(229, 112, 53, 0.15);
}

.input:disabled {
  background: var(--color-neutral-100);
  color: var(--color-neutral-400);
  cursor: not-allowed;
}
```

### Badge

**Variants:**
- `default` - Neutral information
- `success` - Positive status
- `warning` - Caution needed
- `danger` - Error/critical

```css
.badge {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 0.25rem 0.625rem;
  font-size: var(--text-xs);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  border-radius: var(--radius-full);
}

.badge-success {
  background: rgba(34, 197, 94, 0.15);
  color: #15803d;
  border: 1px solid rgba(34, 197, 94, 0.2);
}
```

## Layout

### Grid System

```css
/* Main Layout */
.app-layout {
  display: grid;
  grid-template-columns: 256px 1fr;
  min-height: 100vh;
}

/* Content Area */
.content-area {
  max-width: 1600px;
  margin: 0 auto;
  padding: var(--space-6);
}

/* Card Grid */
.card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: var(--space-4);
}
```

### Breakpoints

| Name | Width | Usage |
|------|-------|-------|
| `sm` | 640px | Mobile landscape |
| `md` | 768px | Tablet |
| `lg` | 1024px | Desktop |
| `xl` | 1280px | Large desktop |
| `2xl` | 1536px | Extra large |

## Accessibility

### Color Contrast

All text must meet WCAG 2.1 AA standards:
- Normal text: 4.5:1 minimum
- Large text (18px+): 3:1 minimum
- UI components: 3:1 minimum

### Focus Indicators

```css
/* Visible focus for keyboard navigation */
:focus-visible {
  outline: 2px solid var(--color-primary);
  outline-offset: 2px;
}

/* Remove default outline for mouse users */
:focus:not(:focus-visible) {
  outline: none;
}
```

### Reduced Motion

```css
@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
```

## Usage Examples

### Status Colors in Practice

```svelte
<!-- VM State Badge -->
<span class="badge" class:badge-success={vm.state === 'running'}>
  <span class="status-dot"></span>
  {vm.state}
</span>

<!-- Usage by state -->
<!-- running → badge-success (green) -->
<!-- stopped → badge-default (gray) -->
<!-- error → badge-danger (red) -->
<!-- starting/stopping → badge-warning (yellow) -->
```

### Card Hierarchy

```svelte
<div class="card">
  <div class="card-header">
    <h3 class="section-title">Virtual Machines</h3>
    <p class="text-sm text-neutral-500">Manage your compute resources</p>
  </div>
  <div class="card-body">
    <!-- Content -->
  </div>
</div>
```

### Form Layout

```svelte
<form class="space-y-4">
  <div class="form-group">
    <label class="label">VM Name</label>
    <input class="input" type="text" placeholder="Enter name" />
    <p class="form-hint">Unique identifier for this VM</p>
  </div>
  
  <div class="form-actions">
    <button class="button-secondary">Cancel</button>
    <button class="button-primary">Create VM</button>
  </div>
</form>
```

---

## Changelog

### v1.0.0 (Current)
- Initial design system
- Proxmox-inspired color palette
- 4px base spacing grid
- Inter + JetBrains Mono typography

