# Design System — CHV (Cloud Hypervisor Virtualization Platform)

## Product Context

**What this is:** A Linux-first, cloud-image-first virtualization platform for sovereign private cloud and edge cloud environments. CHV provides API-driven VM lifecycle management built on Cloud Hypervisor.

**Who it's for:** DevOps engineers, SREs, infrastructure teams, and platform engineers who need on-premise or edge virtualization that feels like a modern cloud provider.

**Space/industry:** Infrastructure/Virtualization — competing with VMware vSphere, Proxmox VE, OpenStack.

**Project type:** Web-based management console (dashboard) + CLI + API

---

## Aesthetic Direction

**Direction:** Warm Earthy Enterprise Console

A refined, warm-toned interface that signals "production-ready infrastructure" without the cold sterility of legacy enterprise tools. The palette draws from natural, earthy tones — warm browns, ambers, and cream — giving the console a distinctive character while remaining entirely functional.

**Decoration level:** Minimal

No gradients, no decorative illustrations. Visual hierarchy comes from:
- Clear border definitions using warm neutral tones
- Semantic color coding for status (muted forest green / amber / terracotta)
- Typography scale with IBM Plex Sans
- Data density and structure

**Mood:** Trustworthy, functional, grounded. Infrastructure you bet your business on — with a palette that's warm rather than clinical.

---

## Typography

**Interface:** IBM Plex Sans — clean, technical, modern enterprise

**Data/Tables:** IBM Plex Mono — monospace for VM IDs, IPs, resource metrics

**Code/Terminal:** IBM Plex Mono — terminal output, logs

**Loading:** Google Fonts CDN (`IBM+Plex+Sans` and `IBM+Plex+Mono`)

**Scale:**
| Token | Size | Pixels | Usage |
|-------|------|--------|-------|
| `--text-xs` | 0.6875rem | ~11px | Labels, timestamps, metadata |
| `--text-sm` | 0.8125rem | ~13px | Secondary text, captions |
| `--text-base` | 0.875rem | 14px | Primary body text, table data |
| `--text-lg` | 1rem | 16px | h4, section subheadings |
| `--text-xl` | 1.125rem | 18px | h3, card titles |
| `--text-2xl` | 1.5rem | 24px | h2, section headers |
| `--text-3xl` | 1.75rem | 28px | h1, page titles |

**Weight usage:** 400 regular, 500 medium, 600 semibold, 700 bold (headings use 600)

---

## Color

**Approach:** Semantic and restrained — color carries meaning, not decoration. The palette is warm and earthy rather than the conventional cool blue/gray enterprise palette.

### Primary

| Role | CSS Variable | Hex | Usage |
|------|-------------|-----|-------|
| Primary | `--color-primary` | #8f5a2a | Links, buttons, active states, focus rings |
| Primary Hover | `--color-primary-hover` | #9f6837 | Button hover state |
| Primary Active | `--color-primary-active` | #76471f | Button active/pressed state |
| Primary Light | `--color-primary-light` | #f5eadc | Subtle primary tint backgrounds |
| Primary Dark | `--color-primary-dark` | #5e3513 | High-contrast primary text |

### Semantic

| Role | CSS Variable | Hex | Usage |
|------|-------------|-----|-------|
| Success | `--color-success` | #3f6b45 | Running, healthy, online |
| Success Light | `--color-success-light` | #edf4ee | Success background tint |
| Warning | `--color-warning` | #9a6a1f | Warning, degraded, maintenance |
| Warning Light | `--color-warning-light` | #f8efd9 | Warning background tint |
| Danger | `--color-danger` | #9b4338 | Stopped, error, critical |
| Danger Light | `--color-danger-light` | #faece8 | Danger background tint |
| Info | `--color-info` | #49627d | Informational states |
| Info Light | `--color-info-light` | #edf1f6 | Info background tint |

### Neutral Scale (warm cream → near-black)

| Token | Hex | Usage |
|-------|-----|-------|
| `--color-neutral-50` | #f7f3ec | Page background, lightest surface |
| `--color-neutral-100` | #efe9df | Sidebar background, alternate rows |
| `--color-neutral-200` | #ddd5c8 | Borders, dividers |
| `--color-neutral-300` | #c7bcac | Muted borders, disabled states |
| `--color-neutral-400` | #9d917f | Placeholder text, scrollbar |
| `--color-neutral-500` | #75695b | Secondary text, labels |
| `--color-neutral-600` | #5e5449 | Muted text |
| `--color-neutral-700` | #423b33 | Secondary headings |
| `--color-neutral-800` | #29241f | Primary text |
| `--color-neutral-900` | #191612 | Headings, highest contrast text |

### Shell / Chrome Tokens

| Token | Value | Usage |
|-------|-------|-------|
| `--shell-bg` | #f3efe7 | Application background |
| `--shell-surface` | rgba(252,249,244,0.96) | Card surfaces, dropdowns |
| `--shell-surface-muted` | rgba(247,242,234,0.96) | Hover state surfaces |
| `--shell-line` | rgba(128,112,93,0.18) | Subtle borders |
| `--shell-line-strong` | rgba(128,112,93,0.30) | Prominent borders |
| `--shell-text` | #1f262d | Body text |
| `--shell-text-secondary` | #4b5865 | Secondary/muted text |
| `--shell-accent` | #8f5a2a | Accent color (same as primary) |
| `--shell-accent-soft` | #f0dfcb | Soft accent tint |

### Status Badge Tokens

| State | Background | Border | Text |
|-------|-----------|--------|------|
| Healthy | `--status-healthy-bg` #edf4ee | rgba(63,107,69,0.18) | `--status-healthy-text` #27462d |
| Warning | `--status-warning-bg` #f8efd9 | rgba(154,106,31,0.18) | `--status-warning-text` #744d0f |
| Degraded | `--status-degraded-bg` #eef2f4 | rgba(73,98,125,0.18) | `--status-degraded-text` #304255 |
| Failed | `--status-failed-bg` #faece8 | rgba(155,67,56,0.18) | `--status-failed-text` #6e2d25 |
| Unknown | `--status-unknown-bg` #f1ede6 | rgba(117,105,91,0.18) | `--status-unknown-text` #5e5449 |

**Dark mode:** Not default. Light mode is standard for enterprise virtualization consoles. Dark mode may be added later as user preference.

---

## Spacing

**Base unit:** 4px (0.25rem)

**Scale (CSS variables):**
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

**Density:** Comfortable-Compact — more dense than consumer SaaS, but not cramped like legacy enterprise tools.

**Layout measurements:**
- Sidebar width: `--sidebar-width` = 15.5rem (~248px)
- Header height: `--header-height` = 2.5rem (40px)
- Content max-width: `--content-max-width` = 1600px
- Card padding: `--space-4` (16px)
- Table cell padding: `--space-2` × `--space-4` (8px 16px)

---

## Layout

**Approach:** Three-pane grid-disciplined

```
┌─────────────┬──────────────────────────┬───────────────┐
│             │                          │               │
│  Sidebar    │      Content Area        │   Details     │
│  (248px)    │      (flexible)          │   (320px)     │
│             │                          │               │
└─────────────┴──────────────────────────┴───────────────┘
```

**Grid:** 12-column within content area

**Navigation:**
- Accordion sidebar with sections: Inventory, Monitoring, Administration
- Tree view for hierarchical resources (datacenters → nodes → VMs)
- Breadcrumb navigation in toolbar
- Context-sensitive actions

**Content patterns:**
- Data tables with zebra striping for resource lists
- Property grids (label | value) for object details
- Tabs for object detail views: Summary | Console | Settings | Logs
- Stats cards at top of list views

**Node-scoped navigation:**
- Datacenter overview shows aggregate stats across all nodes
- Node detail pages show node-specific resources only
- Resource tables filtered by node context
- Breadcrumbs show: Datacenter > Node Name > Resource Type

---

## Components

### Buttons

**Primary:** `--shell-accent` (#8f5a2a) background, white text, `--radius-sm` border radius
**Secondary:** `--shell-surface` background, `--shell-text` color, `--shell-line` border
**Danger:** Use `--color-danger` (#9b4338) for destructive actions

**Sizes:**
- Default: 0.4rem 0.75rem padding, `--text-sm` font size
- Small (`.btn-sm`): 0.25rem 0.5rem padding, `--text-xs` font size

**Transition:** `--duration-fast` (150ms) with `--ease-default`

### Data Tables

- Header: `--color-neutral-100` (#efe9df) background, uppercase labels
- Rows: `--shell-surface` / `--color-neutral-50` alternating
- Hover: `--shell-surface-muted`
- Selected: `--color-primary-light` (#f5eadc)
- Status column: Icon + text (not just colored text)
- Monospace font (`--font-mono`) for IDs, IPs, UUIDs

### Forms

- Labels above inputs (not inline)
- Section grouping with fieldsets
- Input borders: `--shell-line` / `--color-neutral-200`
- Focus state: `--color-primary` (#8f5a2a) border, `--radius-sm`
- Font: inherits `--font-sans`; size `--text-sm`

### Status Indicators

| State | Icon | Color | Hex |
|-------|------|-------|-----|
| Running / Healthy | ● | Forest green | #3f6b45 |
| Stopped / Offline | ● | Neutral gray | #9d917f |
| Warning | ▲ | Warm amber | #9a6a1f |
| Error / Failed | ◼ | Terracotta red | #9b4338 |
| Degraded | ● | Steel blue | #49627d |

### Node Status

| State | Icon | Color | Description |
|-------|------|-------|-------------|
| Online | ● | #3f6b45 (forest green) | Node is healthy and responsive |
| Offline | ● | #9d917f (neutral) | Node is unreachable |
| Maintenance | 🛠 | #9a6a1f (amber) | Node is in maintenance mode |
| Error | ◼ | #9b4338 (terracotta) | Node has errors |

### Border Radius

| Token | Value | Usage |
|-------|-------|-------|
| `--radius-sm` | 0.25rem | Buttons, inputs, small chips |
| `--radius-md` | 0.5rem | Cards, panels |
| `--radius-lg` | 0.75rem | Modals, large cards |
| `--radius-xl` | 1rem | Feature panels |
| `--radius-full` | 9999px | Pills, badges |

---

## Motion

**Approach:** Minimal-functional

| Token | Value | Usage |
|-------|-------|-------|
| `--duration-instant` | 0ms | No animation |
| `--duration-fast` | 150ms | Hovers, button states |
| `--duration-normal` | 250ms | Modal transitions |
| `--duration-slow` | 350ms | Page-level transitions |

**Easing:**
- `--ease-default`: cubic-bezier(0.4, 0, 0.2, 1) — standard smooth
- `--ease-out`: cubic-bezier(0, 0, 0.2, 1) — elements entering
- `--ease-bounce`: cubic-bezier(0.34, 1.56, 0.64, 1) — playful emphasis (use sparingly)

**Rules:**
- No page load animations (instant)
- Data updates: Subtle flash on changed values
- Respects `prefers-reduced-motion` — all transitions collapse to 0.01ms

---

## Reference Implementations

**Primary inspiration:**
- VMware vSphere HTML5 Client (layout patterns, data density)
- Proxmox VE Web Interface (tree navigation, resource management)
- Windows Admin Center (three-pane layout)

**Anti-patterns to avoid:**
- AWS Console (too cluttered, information overload)
- Vercel Dashboard (too modern/spacious for infrastructure tooling)
- Generic SaaS dashboard aesthetics (cold blue palette, excessive whitespace)
- Excessive use of bright/saturated status colors — use the muted semantic palette

---

## Preview

See `docs/design-preview.html` for a live HTML preview of this design system applied to CHV's VM management interface.

To view:
```bash
open docs/design-preview.html
```

---

## Decisions Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2024-04-05 | VMware/Proxmox style chosen | User explicitly requested this direction for enterprise credibility |
| 2024-04-05 | Light mode as default | Matches enterprise virtualization console conventions |
| 2024-04-05 | Three-pane layout | Industry standard for resource management since Windows Explorer |
| 2024-04-05 | Border-heavy UI | Creates clear separation between functional areas |
| 2026-04-10 | Node-scoped navigation | Multi-node support requires clear resource ownership in the UI |
| 2026-04-10 | Tree navigation with node hierarchy | Proxmox-style datacenter → node → resource drill-down pattern |
| 2026-04-19 | Warm earthy palette (#8f5a2a primary, cream neutrals) replaces cool blue (#0066CC) | CSS implementation diverged from original spec; aligning docs to match actual implementation |
| 2026-04-19 | IBM Plex Sans / IBM Plex Mono replaces Roboto | CSS implementation uses IBM Plex family; docs aligned to match |
| 2026-04-19 | Muted semantic colors (forest green, amber, terracotta) replace saturated originals | Earthy palette requires desaturated status colors for harmony |

---

## Usage Guidelines

**Always read this file before making visual or UI decisions.**

All font choices, colors, spacing, and aesthetic direction are defined here.
Do not deviate without explicit user approval.

In QA mode, flag any code that doesn't match this design system.
