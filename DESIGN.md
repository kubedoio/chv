# Design System — CHV (Cloud Hypervisor Virtualization Platform)

## Product Context

**What this is:** A Linux-first, cloud-image-first virtualization platform for sovereign private cloud and edge cloud environments. CHV provides API-driven VM lifecycle management built on Cloud Hypervisor.

**Who it's for:** DevOps engineers, SREs, infrastructure teams, and platform engineers who need on-premise or edge virtualization that feels like a modern cloud provider.

**Space/industry:** Infrastructure/Virtualization — competing with VMware vSphere, Proxmox VE, OpenStack.

**Project type:** Web-based management console (dashboard) + CLI + API

---

## Aesthetic Direction

**Direction:** Enterprise Virtualization Console

A VMware vSphere / Proxmox VE inspired interface that signals "production-ready, enterprise-grade virtualization." This is the standard for on-premise infrastructure management — familiar to anyone who's managed ESXi or Proxmox clusters.

**Decoration level:** Minimal

No gradients, no decorative illustrations. Visual hierarchy comes from:
- Clear border definitions
- Status color coding (green/yellow/red)
- Typography scale
- Data density and structure

**Mood:** Trustworthy, functional, serious. This is infrastructure you bet your business on.

---

## Typography

**Interface:** Roboto — clean, neutral, enterprise-standard

**Data/Tables:** Roboto Mono — monospace for VM IDs, IPs, resource metrics

**Code/Terminal:** Roboto Mono — terminal output, logs

**Scale:**
| Level | Size | Usage |
|-------|------|-------|
| Page title | 20px | Main page headings |
| Section header | 16px | Card titles, section breaks |
| Body | 14px | Primary text, table data |
| Small | 12px | Labels, timestamps, metadata |

**Loading:** Google Fonts CDN or self-hosted

---

## Color

**Approach:** Balanced — color has meaning (status), not decoration

| Role | Hex | Usage |
|------|-----|-------|
| Primary | #0066CC | Links, buttons, active states, selection |
| Success | #54B435 | Running, healthy, online |
| Warning | #F0AB00 | Warning, degraded, maintenance |
| Error | #E60000 | Stopped, error, critical |
| Text Primary | #1A1A1A | Headings, primary content |
| Text Secondary | #666666 | Labels, descriptions, muted text |
| Border | #D0D0D0 | Dividers, input borders, table lines |
| Background Chrome | #F5F5F5 | Sidebar, toolbar, chrome areas |
| Background Content | #FFFFFF | Content cards, tables, forms |
| Hover/Selected | #E8F4FC | Row hover states |
| Selected | #CCE5F9 | Active selection |

**Dark mode:** Not default. Light mode is standard for enterprise virtualization consoles. Dark mode may be added later as user preference.

---

## Spacing

**Base unit:** 4px

**Scale:** 4, 8, 12, 16, 24, 32, 48

**Density:** Comfortable-Compact — more dense than consumer SaaS, but not cramped like legacy enterprise tools.

**Layout measurements:**
- Sidebar width: 240px
- Details panel width: 320px
- Content max-width: none (fills available space)
- Card padding: 16px
- Table cell padding: 8px 16px

---

## Layout

**Approach:** Three-pane grid-disciplined

```
┌─────────────┬──────────────────────────┬───────────────┐
│             │                          │               │
│  Sidebar    │      Content Area        │   Details     │
│  (240px)    │      (flexible)          │   (320px)     │
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

**Primary:** Solid blue (#0066CC), white text
**Secondary:** White background, gray border, dark text
**Danger:** White background, red border, red text (for destructive actions)

### Data Tables

- Header: #F0F0F0 background, uppercase labels
- Rows: White / #F8F8F8 alternating
- Hover: #E8F4FC
- Selected: #CCE5F9
- Status column: Icon + text (not just colored text)
- Monospace font for IDs, IPs, UUIDs

### Forms

- Labels above inputs (not inline)
- Section grouping with fieldsets
- Input borders: #CCCCCC
- Focus state: Blue border

### Status Indicators

| State | Icon | Color |
|-------|------|-------|
| Running | ● | Green #54B435 |
| Stopped | ● | Gray #999999 |
| Warning | ▲ | Amber #F0AB00 |
| Error | ◼ | Red #E60000 |

### Node Status

| State | Icon | Color | Description |
|-------|------|-------|-------------|
| Online | ● | Green #54B435 | Node is healthy and responsive |
| Offline | ● | Gray #999999 | Node is unreachable |
| Maintenance | 🛠 | Amber #F0AB00 | Node is in maintenance mode |
| Error | ◼ | Red #E60000 | Node has errors |

---

## Motion

**Approach:** Minimal-functional

- Transitions: 150ms ease-out for hovers
- No page load animations (instant)
- Data updates: Subtle flash on changed values
- Modal dialogs: 200ms ease-out

---

## Reference Implementations

**Primary inspiration:**
- VMware vSphere HTML5 Client
- Proxmox VE Web Interface
- Windows Admin Center

**Anti-patterns to avoid:**
- AWS Console (too cluttered)
- Vercel Dashboard (too modern/spacious)
- Generic SaaS dashboard aesthetics

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
| 2024-04-05 | Roboto font family | Enterprise-standard, highly readable, available via Google Fonts |
| 2024-04-05 | Three-pane layout | Industry standard for resource management since Windows Explorer |
| 2024-04-05 | Border-heavy UI | Creates clear separation between functional areas |
| 2026-04-10 | Node-scoped navigation | Multi-node support requires clear resource ownership in the UI |
| 2026-04-10 | Tree navigation with node hierarchy | Proxmox-style datacenter → node → resource drill-down pattern |

---

## Usage Guidelines

**Always read this file before making visual or UI decisions.**

All font choices, colors, spacing, and aesthetic direction are defined here.
Do not deviate without explicit user approval.

In QA mode, flag any code that doesn't match this design system.
