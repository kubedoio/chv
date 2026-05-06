# CHV Design Review Report

**Date**: 2026-05-07
**Scope**: WebUI (SvelteKit) + Controlplane API (Rust/Axum)
**Verdict**: Not production-ready. Functional core exists but architectural split-brain causes silent failures across 35+ endpoints.

---

## Executive Summary

The system has working auth (JWT + bcrypt), a solid Rust BFF layer, and good component architecture in the UI. But a legacy "stub" API layer and a newer BFF layer coexist without clear boundaries, causing the UI to silently display empty states when real data exists behind different URLs. The design system is violated in the most-visible components (sidebar, navigation), and dark mode is broken across 8+ domain views.

**Severity breakdown**: 7 Critical, 6 High, 5 Medium, 3 Low

---

## CRITICAL Findings

### C1: 35+ Dead/Broken Routes in api/client.ts

The UI's `api/client.ts` defines ~70 endpoints. Over half hit routes that either don't exist or use the wrong HTTP method.

**Routes that return NOT_IMPLEMENTED (no server handler):**

| Category | Dead Endpoints |
|----------|---------------|
| VM lifecycle | `/api/v1/vms/{id}/start`, `stop`, `restart`, `shutdown`, `force-stop`, `reset` |
| VM monitoring | `/api/v1/vms/{id}/console`, `metrics`, `status` |
| VM bulk ops | `/api/v1/vms/bulk/start`, `bulk/stop`, `bulk/delete` |
| VM snapshots | `/api/v1/vms/snapshots` (all 4 operations - wrong prefix) |
| VM cloud-init | `/api/v1/vms/{id}/cloud-init/apply` |
| Node ops | `/api/v1/nodes/{id}/maintenance`, `discover`, `vms`, `images`, `storage`, `networks`, `metrics` |
| Network VLAN/DHCP | `/api/v1/networks/{id}/vlans`, `dhcp`, `dhcp/start`, `dhcp/stop`, `dhcp/leases` |
| Firewall | `/api/v1/vms/{id}/firewall/rules` (GET, POST, DELETE) |
| Images | `/api/v1/images/{id}/progress`, `/api/v1/images/upload` |
| Auth | `/api/v1/tokens`, `/api/v1/login/validate` |

**Routes with wrong HTTP method (405 responses):**

| Call | Uses | Server Expects |
|------|------|----------------|
| `listQuotas()` | GET `/v1/quotas` | POST |
| `getUsage()` | GET `/v1/usage` | POST |

**Routes hitting stubs instead of BFF (empty array responses):**

| Call | Hits | Real Data At |
|------|------|-------------|
| `listVMs()` | GET `/api/v1/vms` (stub → `[]`) | POST `/v1/vms` (BFF) |
| `listNodes()` | GET `/api/v1/nodes` (stub → `[]`) | POST `/v1/nodes` (BFF) |
| `listNetworks()` | GET `/api/v1/networks` (stub → `[]`) | POST `/v1/networks` (BFF) |
| `listImages()` | GET `/api/v1/images` (stub → `[]`) | POST `/v1/images` (BFF) |

### C2: Install Endpoints Exposed Without Auth

**Location**: `crates/chv-controlplane-service/src/api/router.rs:119-124`

```rust
.route("/api/v1/install/bootstrap", post(stub::bootstrap_install_stub))
.route("/api/v1/install/repair", post(stub::repair_install_stub))
```

No auth middleware. Anyone who can reach the controlplane port can trigger bootstrap/repair.

### C3: Brand Identity Wrong in Navigation Shell

**Location**: `Sidebar.svelte`, `TreeNavigation.svelte`

| Component | Uses | Design System Says |
|-----------|------|--------------------|
| Sidebar background | `bg-[#0f172a]` (slate-900) | `--color-sidebar-bg: #1a1a2e` |
| Sidebar accent | `bg-indigo-600` | `--color-primary: #8f5a2a` (warm brown) |
| Sidebar text | `text-indigo-400` | `--color-primary-light: #b87a4a` |
| TreeNav background | `#252532` | `--color-sidebar-bg: #1a1a2e` |
| TreeNav accent | `#e57035` | `--color-primary: #8f5a2a` |
| TreeNav hover | `#ff9a65` | `--color-primary-light: #b87a4a` |

The most-visible UI components (present on every page) use completely wrong colors. Three different palettes in one app.

### C4: Response Shape Mismatches

**Node data:**
- TS expects: `{ id, name, status, ip_address, cpu_cores, memory_mb }`
- Rust returns: `{ node_id, hostname, display_name, status, cpu_threads, memory_bytes }`

**Backup jobs:**
- `api/client.ts` declares return `BackupJobResponse[]` (bare array)
- BFF `/v1/backup-jobs` returns `{ items: [...], page: {...}, filters: {...} }`

**Node-scoped vs global lists:**
- Node-scoped: `{ node_id, node_name, resources: T[], count }`
- Global: bare `T[]` (stubs) or `{ items: T[], page, filters }` (BFF)

### C5: Dual Token Storage Split-Brain

- `api/client.ts` reads/writes token in `localStorage` key `chv-api-token`
- Server-side `+page.server.ts` reads token from cookie `chv_session`
- `syncAuthCookieFromLocalStorage()` is a no-op (commented "not yet implemented")

**Impact**: All SvelteKit form actions (VM mutations, node operations, volume management) pass `cookies.get('chv_session')` to BFF calls. Since login only sets localStorage, cookie is always empty, server-side mutations fail silently.

### C6: BFF 401 Handling Missing Redirect

- `api/client.ts:259` redirects to `/login` on 401
- `bff/client.ts:105` clears token but does NOT redirect

Users on BFF-backed pages (nodes, VMs, volumes) whose session expires see the page stay visible with empty data. No toast, no redirect.

### C7: Three Incompatible Error Shapes

The TS client only handles Shape C, but the BFF emits Shape B:

| Shape | Source | Format |
|-------|--------|--------|
| A | `nodes.rs`, `operations.rs` | `{"error": "string"}` |
| B | BFF `BffError`, auth rejections | `{"message": "...", "code": "BAD_REQUEST"}` |
| C | `not_found_handler`, stubs | `{"error": {"code": "...", "message": "...", "retryable": bool}}` |

The TS client parses `payload?.error.code` and `payload?.error.message`. For Shape B responses, `payload.error` is `undefined` so error codes and messages are always lost. Users see generic "Unknown error" for all BFF failures.

---

## HIGH Findings

### H1: Dark Mode Broken in 8+ Components

Hardcoded `bg-white`, `text-slate-*`, `border-slate-200`:

1. `NodeHealthDashboard.svelte` (313 lines, fully light-only)
2. `AddNodeModal.svelte`
3. `StoragePoolCreateModal.svelte`
4. `NetworkCreateModal.svelte`
5. `CloudInitTemplateEditor.svelte`
6. `VMDetailView.svelte`
7. `BackupJobCard.svelte`
8. `QuotaManagement.svelte`

### H2: Accessibility Violations

| Issue | Location | WCAG |
|-------|----------|------|
| No `aria-sort` on sortable headers | `DataTable.svelte` | 4.1.2 |
| `div onclick` no keyboard handler | `TreeNavigation.svelte`, `Sidebar.svelte` | 2.1.1 |
| No skip-to-content link | `+layout.svelte` | 2.4.1 |
| No `+error.svelte` boundary | `routes/` | Best practice |
| Color-only state indication | `StateBadge.svelte` | 1.4.1 |

### H3: Error Swallowing in Route Loaders

All `+page.ts` catch blocks silently discard errors:

```typescript
// vms/[id]/+page.ts:113
} catch {
  return { detail: { state: 'error' }, requestedVmId: params.id };
}
```

Same in `networks/+page.ts`, `nodes/+page.ts`, `images/+page.ts`. No user notification, no logging beyond `console.error`.

### H4: Stub Module Contains Real Auth Logic

`crates/chv-controlplane-service/src/api/stub.rs` has the actual `login_handler` with bcrypt verification and JWT issuance. If stubs are cleaned up, auth breaks.

### H5: upload() Function Degraded Error Handling

`api/client.ts:304-334` — the `upload()` function:
- No structured error parsing
- No 401 handling (no token clear, no redirect)
- No toast on 5xx
- Affects image upload and VM import

### H6: Unbounded List Endpoints

| Endpoint | Issue |
|----------|-------|
| `list_nodes` (admin) | No LIMIT clause, full table scan |
| `list_operations` (admin) | Hardcoded LIMIT 100, no pagination params |
| 6 backup list handlers | Hardcoded `limit=1000, offset=0`, no pagination exposed |

No client can retrieve operations 101+ or backup items 1001+.

---

## MEDIUM Findings

### M1: Duplicate Route Registration

```rust
// crates/chv-webui-bff/src/router.rs
.route("/v1/quotas", post(list_quotas))
// 50 lines later...
.route("/api/v1/quotas", post(list_quotas))  // DUPLICATE
```

### M2: 29 Components Exceed 300-Line Limit

Largest: `VMDetailView.svelte` (680 lines), `CloudInitTemplateEditor.svelte` (450+), `TreeNavigation.svelte` (330), `NodeHealthDashboard.svelte` (313).

### M3: Dead Code

| File | Issue |
|------|-------|
| `ui/src/lib/stores/cache.svelte.ts` | Exports `nodeCache`, `vmCache` — imported nowhere |
| `ui/src/lib/components/shared/PostureCard.svelte` | Imported nowhere |
| `ui/src/lib/components/shared/MetricSparkline.svelte` | Duplicate of `primitives/Sparkline.svelte` |

### M4: Mixed State Management

Some stores use Svelte 4 `writable()`, others use Svelte 5 `$state` runes. No migration boundary documented.

### M5: Hardcoded Colors in Domain Components

`StateBadge.svelte` uses `#15803d`, `#a16207`, `#b91c1c` instead of `--color-success-dark`, `--color-warning-dark`, `--color-danger-dark`.

---

## LOW Findings

### L1: Font sizes using Tailwind classes instead of type scale tokens
### L2: Spacing mix of Tailwind utilities and `--space-*` tokens
### L3: Transition durations not standardized to `--duration-fast`/`--duration-normal`

---

## Remediation Plan

### Phase 1: API Consolidation (Critical, ~3 days)

**Goal**: Every UI call reaches a real handler and gets correctly-shaped data back.

| # | Task | Impact |
|---|------|--------|
| 1.1 | Delete `api/client.ts` entirely | Removes 35+ dead routes |
| 1.2 | Consolidate all calls into BFF client (`bff/client.ts` + domain modules) | Single source of truth |
| 1.3 | For calls that only exist in `api/client.ts` (backups, templates, quotas): migrate to BFF domain modules | All calls work |
| 1.4 | Remove stub routes that have BFF equivalents | Eliminate confusion |
| 1.5 | Move `login_handler` from `stub.rs` to `api/auth.rs` | Proper module location |
| 1.6 | Fix BFF 401 handling: add redirect to `/login` | Session expiry handled |
| 1.7 | Add `Authorization` header enforcement in `bffFetch()` when token exists | No silent unauth calls |

### Phase 2: Security (Critical, ~0.5 day)

| # | Task |
|---|------|
| 2.1 | Move `/api/v1/install/*` behind `admin_middleware` |
| 2.2 | Implement cookie-based auth (set `chv_session` on login) |
| 2.3 | Remove duplicate `/api/v1/quotas` registration |

### Phase 3: Brand Identity (Critical, ~1 day)

| # | Task | Change |
|---|------|--------|
| 3.1 | `Sidebar.svelte` | `bg-[#0f172a]` → `var(--color-sidebar-bg)`, `bg-indigo-600` → `var(--color-primary)` |
| 3.2 | `TreeNavigation.svelte` | `#252532` → `var(--color-sidebar-bg)`, `#e57035` → `var(--color-primary)` |
| 3.3 | `StateBadge.svelte` | Hex colors → `var(--color-success-dark)` etc. |
| 3.4 | Grep all `shell/*.svelte` for hex colors | Replace with tokens |

### Phase 4: Dark Mode (High, ~1.5 days)

| # | Task |
|---|------|
| 4.1 | Create `.bg-surface`, `.text-body`, `.border-subtle` utility classes |
| 4.2 | Fix all 8 components identified in H1 |
| 4.3 | Visual smoke test every route in dark mode |

### Phase 5: Accessibility (High, ~1 day)

| # | Task |
|---|------|
| 5.1 | Add `aria-sort` to `DataTable.svelte` sortable headers |
| 5.2 | Replace `div onclick` with `button` or add `role="button"` + `tabindex="0"` + keyboard handler |
| 5.3 | Add skip-to-content link in `+layout.svelte` |
| 5.4 | Create `+error.svelte` error boundary |
| 5.5 | Add icons alongside colors in `StateBadge.svelte` |

### Phase 6: Error Handling (High, ~1 day)

| # | Task |
|---|------|
| 6.1 | Replace all silent `catch {}` blocks with toast notifications |
| 6.2 | Add structured error logging in route loaders |
| 6.3 | Fix `upload()` to parse error envelope and handle 401 |

### Phase 7: Cleanup (Medium, ~1 day)

| # | Task |
|---|------|
| 7.1 | Delete `cache.svelte.ts` |
| 7.2 | Delete `PostureCard.svelte` |
| 7.3 | Delete `MetricSparkline.svelte` |
| 7.4 | Split oversized components (VMDetailView, CloudInitTemplateEditor) |
| 7.5 | Migrate remaining `writable()` stores to `$state` runes |

### Phase 8: Token Polish (Low, ongoing)

- Replace Tailwind text-size classes with CSS custom properties
- Standardize transitions
- Audit spacing

---

## Execution Order

```
Week 1: Phase 1 + Phase 2 (API works, security gaps closed)
Week 2: Phase 3 + Phase 4 (brand correct, dark mode works)
Week 3: Phase 5 + Phase 6 (accessible, errors visible)
Week 4: Phase 7 + Phase 8 (clean codebase, polished)
```

Phase 1 is highest-impact: fixing the API layer makes the product actually functional. Everything else is polish on top of a working system.

---

## Key Files for Remediation

| File | Lines | Role in Fix |
|------|-------|-------------|
| `ui/src/lib/api/client.ts` | ~800 | DELETE — replace with BFF modules |
| `ui/src/lib/bff/client.ts` | 119 | Add 401 redirect, toast, enforce auth header |
| `crates/chv-controlplane-service/src/api/router.rs` | 129 | Remove stub routes with BFF equivalents, gate install |
| `crates/chv-controlplane-service/src/api/stub.rs` | 359 | Extract auth to proper module, delete rest |
| `crates/chv-webui-bff/src/router.rs` | 422 | Remove duplicate registrations |
| `ui/src/lib/components/shell/Sidebar.svelte` | ~200 | Design token migration |
| `ui/src/lib/components/shell/TreeNavigation.svelte` | 330 | Design token migration |
| `ui/src/lib/components/shared/StateBadge.svelte` | ~60 | Design token + icon migration |
