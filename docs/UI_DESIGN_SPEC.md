# CHV UI Design Specification

## Document Purpose

This document defines the design goals, component specifications, and interaction patterns for the CHV (Cloud Hypervisor Virtualization) web interface. It serves as the source of truth for frontend implementation.

---

## 1. Design Goals

### 1.1 Primary Goals

| Goal | Description | Success Criteria |
|------|-------------|------------------|
| **Operator Efficiency** | Enable rapid VM lifecycle management with minimal clicks | Common tasks complete in < 3 clicks |
| **Trust & Clarity** | Present system state honestly with clear visual indicators | Users can diagnose issues without logs |
| **Familiarity** | Follow enterprise virtualization console conventions | VMware/Proxmox users feel at home |
| **Responsiveness** | Interface feels snappy and responsive | Page load < 1s, actions < 200ms feedback |

### 1.2 Design Principles

1. **Data Density Over Whitespace** - Show more data, less padding. Operators need to see state at a glance.
2. **Status-First Visualization** - State indicators are the most important visual element.
3. **Progressive Disclosure** - Hide complexity until needed. Simple list → Detail → Edit flow.
4. **Action Affordance** - Buttons look clickable, disabled states are obvious.
5. **Error Transparency** - API errors surface directly in context, not just in logs.

---

## 2. Component Specifications

### 2.1 Layout Shell

```
┌─────────────────────────────────────────────────────────────┐
│ CHV Logo    Dashboard    Inventory    Operations    [User] │  Header (56px)
├──────────┬──────────────────────────────────────────────────┤
│          │                                                  │
│ Sidebar  │              Content Area                        │
│ (240px)  │              (flexible)                          │
│          │                                                  │
│          │                                                  │
└──────────┴──────────────────────────────────────────────────┘
```

**Header Specifications:**
- Height: 56px
- Background: #FFFFFF
- Border-bottom: 1px solid #D0D0D0
- Logo: 32x32px, left-aligned with 16px padding
- Navigation links: 14px, #666666, hover #0066CC
- User menu: right-aligned, dropdown on click

**Sidebar Specifications:**
- Width: 240px
- Background: #F5F5F5
- Border-right: 1px solid #D0D0D0
- Section headers: 11px uppercase, #666666, tracking 0.16em
- Nav items: 14px, #1A1A1A, 40px height, 16px padding
- Active state: left border 3px #0066CC, background #E8F4FC
- Hover state: background #E8F4FC

**Content Area:**
- Background: #FFFFFF
- Padding: 24px
- Max content width: none (fill available space)

### 2.2 Data Tables

**Table Header:**
```
┌─────────────────────────────────────────────────────────────┐
│ COLUMN 1      COLUMN 2      COLUMN 3      COLUMN 4      ... │
├─────────────────────────────────────────────────────────────┤
```
- Background: #F0F0F0
- Text: 11px uppercase, #666666, tracking 0.08em
- Padding: 12px 16px
- Border-bottom: 1px solid #D0D0D0

**Table Rows:**
- Odd rows: #FFFFFF
- Even rows: #F8F8F8
- Hover: #E8F4FC
- Selected: #CCE5F9
- Padding: 8px 16px
- Border-bottom: 1px solid #D0D0D0

**Table Cells:**
- Default text: 14px, #1A1A1A
- Monospace text (IDs, IPs): Roboto Mono, 13px, #1A1A1A
- Secondary text: 12px, #666666

### 2.3 Status Badges

| State | Appearance | Usage |
|-------|------------|-------|
| **Running** | Green dot (#54B435) + "running" text | VM is active |
| **Stopped** | Gray dot (#999999) + "stopped" text | VM is halted |
| **Starting** | Yellow dot (#F0AB00) + "starting" text | VM boot in progress |
| **Error** | Red dot (#E60000) + "error" text | VM failed |
| **Ready** | Green dot + "ready" text | Resource available |
| **Pending** | Yellow dot + "pending" text | Operation queued |
| **Active** | Green dot + "active" text | Network operational |

**Badge Component:**
- Dot size: 8px diameter
- Gap between dot and text: 8px
- Text: 12px, color matches dot
- Container padding: 4px 8px
- Border-radius: 4px (subtle background)

### 2.4 Buttons

**Primary Button:**
- Background: #0066CC
- Text: #FFFFFF, 14px, font-weight 500
- Padding: 8px 16px
- Border-radius: 4px
- Hover: #0052A3
- Active: #003D7A
- Disabled: #B3D7F7 background, #FFFFFF text

**Secondary Button:**
- Background: #FFFFFF
- Border: 1px solid #D0D0D0
- Text: #1A1A1A, 14px
- Padding: 8px 16px
- Border-radius: 4px
- Hover: #F5F5F5
- Active: #E8E8E8

**Danger Button:**
- Background: #FFFFFF
- Border: 1px solid #E60000
- Text: #E60000, 14px
- Hover: #FFF0F0

**Icon Button:**
- Size: 32px x 32px
- Background: transparent
- Icon color: #666666
- Hover: #E8F4FC background, #0066CC icon

### 2.5 Forms

**Input Fields:**
- Height: 36px
- Border: 1px solid #CCCCCC
- Border-radius: 4px
- Padding: 8px 12px
- Font: 14px Roboto
- Focus: border-color #0066CC, outline 2px #E8F4FC
- Error: border-color #E60000

**Labels:**
- Position: Above input
- Font: 12px, #666666, font-weight 500
- Margin-bottom: 4px

**Helper Text:**
- Font: 12px, #666666
- Margin-top: 4px

**Error Messages:**
- Font: 12px, #E60000
- Icon: Alert triangle (optional)
- Margin-top: 4px

### 2.6 Cards

**Table Card:**
- Background: #FFFFFF
- Border: 1px solid #D0D0D0
- Border-radius: 4px
- Header padding: 16px 24px
- Header border-bottom: 1px solid #D0D0D0
- Content: table directly inside

**Info Card:**
- Background: #FFFFFF
- Border: 1px solid #D0D0D0
- Border-radius: 4px
- Padding: 16px
- Shadow: none (flat design)

### 2.7 Modals

**Modal Container:**
- Background: #FFFFFF
- Border-radius: 8px
- Width: 480px (default), 640px (wide)
- Max-height: 80vh
- Shadow: 0 4px 16px rgba(0,0,0,0.15)

**Modal Header:**
- Height: 56px
- Padding: 16px 24px
- Border-bottom: 1px solid #D0D0D0
- Title: 16px, #1A1A1A, font-weight 600
- Close button: top-right, 24px

**Modal Body:**
- Padding: 24px
- Overflow-y: auto

**Modal Footer:**
- Padding: 16px 24px
- Border-top: 1px solid #D0D0D0
- Actions: right-aligned, primary on right
- Gap between buttons: 8px

**Modal Backdrop:**
- Background: rgba(0,0,0,0.5)
- Click to close (configurable)

---

## 3. Page Specifications

### 3.1 Networks Page

**Layout:**
```
┌─────────────────────────────────────────────────────────────┐
│ Networks                                    [+ Create]     │  Header
├─────────────────────────────────────────────────────────────┤
│                                                             │
│ ┌───────────────────────────────────────────────────────┐   │
│ │ Name    Bridge    CIDR    Gateway    Managed   Status │   │  Table
│ │ ...     ...       ...     ...        ...       ...    │   │
│ └───────────────────────────────────────────────────────┘   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Create Network Modal:**

| Field | Type | Required | Default | Validation |
|-------|------|----------|---------|------------|
| Name | text | Yes | - | Unique, lowercase, alphanumeric + hyphen |
| Mode | select | Yes | bridge | Locked to "bridge" for MVP-1 |
| Bridge Name | text | Yes | chvbr0 | Must match host bridge |
| CIDR | text | Yes | 10.0.0.0/24 | Valid CIDR notation |
| Gateway IP | text | Yes | 10.0.0.1 | Must be in CIDR range |

**Create Button Logic:**
- Disabled until all required fields valid
- On submit: POST /api/v1/networks
- On success: Close modal, refresh table, show toast
- On error: Show error in modal, keep open

### 3.2 Storage Page

**Layout:**
```
┌─────────────────────────────────────────────────────────────┐
│ Storage                                     [+ Create]     │  Header
├─────────────────────────────────────────────────────────────┤
│                                                             │
│ ┌─────────────────────────────────────────────────────┐     │
│ │ Name    Type    Path    Default    Status          │     │  Table
│ │ ...     ...     ...     ...        ...             │     │
│ └─────────────────────────────────────────────────────┘     │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Create Storage Pool Modal:**

| Field | Type | Required | Default | Validation |
|-------|------|----------|---------|------------|
| Name | text | Yes | - | Unique, lowercase, alphanumeric + hyphen |
| Type | select | Yes | localdisk | Locked to "localdisk" for MVP-1 |
| Path | text | Yes | - | Absolute path, must exist or be creatable |
| Capacity | number | No | - | Bytes, for display only |

**Field Behaviors:**
- Path field: Show helper text "Absolute path on host filesystem"
- Type field: Read-only in MVP-1 (future: NFS, Ceph options)
- Capacity: Optional, purely informational

### 3.3 VM List Page

**Layout:**
```
┌─────────────────────────────────────────────────────────────────────┐
│ Virtual Machines                              [+ Create VM]        │
├─────────────────────────────────────────────────────────────────────┤
│ ┌─────┐ ┌─────┐ ┌─────┐                                            │
│ │Total│ │Runn-│ │Stopp│    Stats cards                             │
│ │  12 │ │  8  │ │  4  │                                            │
│ └─────┘ └─────┘ └─────┘                                            │
│                                                                     │
│ ┌───────────────────────────────────────────────────────────────┐  │
│ │ Name │ Image │ Pool │ Network │ vCPU │ Memory │ IP │ Status  │  │
│ │ ...  │ ...   │ ...  │ ...     │ ...  │ ...    │ ...│ ...     │  │
│ └───────────────────────────────────────────────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

**Stats Cards:**
- Background: #F5F5F5
- Border: 1px solid #D0D0D0
- Padding: 16px
- Title: 11px uppercase, #666666
- Value: 32px, #1A1A1A, font-weight 600

---

## 4. Interaction Patterns

### 4.1 Toast Notifications

**Success Toast:**
- Background: #F0F9F0 (light green)
- Border-left: 4px #54B435
- Icon: Checkmark
- Text: 14px, #1A1A1A
- Auto-dismiss: 5 seconds
- Position: top-right, 16px from edges

**Error Toast:**
- Background: #FFF0F0 (light red)
- Border-left: 4px #E60000
- Icon: Alert octagon
- Text: 14px, #1A1A1A
- Requires manual close
- Position: top-right

**Info Toast:**
- Background: #E8F4FC (light blue)
- Border-left: 4px #0066CC
- Auto-dismiss: 5 seconds

### 4.2 Loading States

**Button Loading:**
- Show spinner icon (16px) left of text
- Disable button
- Spinner animation: 1s linear infinite rotation

**Table Loading:**
- Show 5 skeleton rows
- Skeleton: animated gradient background
- Pulse animation: 1.5s ease-in-out infinite

**Page Loading:**
- Skeleton layout matching content structure
- Header and sidebar render immediately
- Content area shows skeleton

### 4.3 Empty States

**Empty Table:**
- Icon: Large (48px) resource-specific icon, #D0D0D0
- Title: 16px, #666666, "No [resources] yet"
- Description: 14px, #999999
- CTA: Primary button to create first resource

**Example:**
```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│                    [Network Icon]                          │
│                                                             │
│              No networks yet                               │
│                                                             │
│    Create a network to connect your VMs                    │
│                                                             │
│              [Create Network]                              │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 4.4 Confirmation Dialogs

**Destructive Actions:**
- Title: "Delete [Resource]?"
- Description: Explain consequences
- Primary action: Red "Delete" button
- Secondary action: "Cancel"
- Require explicit confirmation

**Non-Destructive:**
- No confirmation for:
  - Creating resources
  - Starting/stopping VMs
  - Updating configurations
- Toast confirms action success

---

## 5. Responsive Behavior

### 5.1 Breakpoints

| Breakpoint | Width | Layout Changes |
|------------|-------|----------------|
| Desktop | > 1280px | Full three-pane layout |
| Laptop | 1024-1280px | Collapse details panel |
| Tablet | 768-1024px | Hide sidebar, hamburger menu |
| Mobile | < 768px | Single column, stacked tables |

### 5.2 Mobile Adaptations

- Sidebar becomes slide-out drawer
- Tables become cards (horizontal scroll for wide tables)
- Modals become full-screen
- Stats cards stack vertically

---

## 6. Accessibility Requirements

### 6.1 Keyboard Navigation

- All interactive elements focusable
- Tab order follows visual layout
- Modal: Tab traps within, Escape to close
- Table: Arrow keys navigate cells

### 6.2 Screen Reader

- Tables: `aria-label` describing content
- Status badges: Include text, not just color
- Modals: `aria-modal="true"`, `role="dialog"`
- Live regions for toast notifications

### 6.3 Visual

- Minimum contrast: 4.5:1 for text
- Focus indicators: 2px outline, visible
- Color not sole indicator of status

---

## 7. Implementation Notes

### 7.1 CSS Architecture

```css
/* Design tokens */
:root {
  /* Colors */
  --color-primary: #0066CC;
  --color-success: #54B435;
  --color-warning: #F0AB00;
  --color-error: #E60000;
  --color-text-primary: #1A1A1A;
  --color-text-secondary: #666666;
  --color-border: #D0D0D0;
  --color-bg-chrome: #F5F5F5;
  --color-bg-content: #FFFFFF;
  --color-hover: #E8F4FC;
  --color-selected: #CCE5F9;
  
  /* Typography */
  --font-sans: 'Roboto', sans-serif;
  --font-mono: 'Roboto Mono', monospace;
  
  /* Spacing */
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-6: 24px;
  --space-8: 32px;
  
  /* Layout */
  --sidebar-width: 240px;
  --header-height: 56px;
}
```

### 7.2 Component Library

Use Tailwind CSS with custom configuration mapping to design tokens:

```javascript
// tailwind.config.js
module.exports = {
  theme: {
    extend: {
      colors: {
        primary: '#0066CC',
        success: '#54B435',
        warning: '#F0AB00',
        error: '#E60000',
        'text-primary': '#1A1A1A',
        'text-secondary': '#666666',
        line: '#D0D0D0',
        chrome: '#F5F5F5',
      },
      fontFamily: {
        sans: ['Roboto', 'sans-serif'],
        mono: ['Roboto Mono', 'monospace'],
      },
      spacing: {
        '18': '72px',
        '15': '60px',
      }
    }
  }
}
```

---

## 8. Future Considerations

### 8.1 MVP-2 Additions
- Dark mode toggle
- User preferences persistence
- Custom dashboard widgets
- Bulk operations (multi-select)
- Advanced filtering

### 8.2 Post-MVP
- Real-time WebSocket updates
- Drag-and-drop VM organization
- Terminal/console integration
- Metrics charts and graphs

---

## Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2024-04-08 | Modals for create flows | Keeps context, doesn't navigate away from list |
| 2024-04-08 | No delete confirmations for MVP | Reduce friction; add when users request |
| 2024-04-08 | Locked mode/type fields | MVP-1 only supports bridge/localdisk |
| 2024-04-08 | Toast notifications | Non-blocking feedback, standard pattern |

---

## Related Documents

- `DESIGN.md` - Visual design system
- `README.md` - Product overview
- `/internal/api/` - Backend API contracts
