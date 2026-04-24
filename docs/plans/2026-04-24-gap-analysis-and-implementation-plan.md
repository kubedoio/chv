# CHV Gap Analysis & Implementation Plan

> **Date:** 2026-04-24
> **Scope:** Full-stack review of implemented vs. postponed features across backend, agent, and UI.

---

## 1. Executive Summary

The CHV codebase has a solid Phase 1 foundation. Sprint 10 (snapshots, metrics, export/import, hypervisor settings) is **mostly complete** in the backend but has **UI and agent-runtime integration gaps**. Several Phase 2 items remain stubbed or unimplemented.

Key areas needing work:
- **Agent daemon integration:** Stord and NWD mock stubs cover many operations; real clients exist but several RPCs are unimplemented.
- **Serial console:** Backend is implemented; PTY lifecycle and token validation need hardening.
- **Hypervisor settings:** DB + BFF are done; orchestrator merge logic and agent `vm.create` injection are partially wired.
- **Network mutations:** BFF `mutate_network` returns `NotImplemented`.
- **UI production readiness:** Large Tailwind-first refactor and component reorganization are planned but not started.
- **Backup/DR:** Completely stubbed.
- **Quota enforcement:** CRUD exists; runtime enforcement at VM-create time is missing.

---

## 2. Gap Catalog

### 2.1 Backend / Control Plane

| # | Gap | Location | Status | Effort | Priority |
|---|-----|----------|--------|--------|----------|
| B1 | **Network mutations not implemented** | `bff_mutations.rs:412` returns `NotImplemented` | Stub | S | P1 |
| B2 | **Backup jobs & history stubbed** | `api/stub.rs:266-272` returns `[]` | Stub | M | P2 |
| B3 | **Quota enforcement at orchestrator** | No quota check in `create_vm` path | Missing | M | P1 |
| B4 | **ping_vmm unimplemented** | `server.rs:509` | Stub | S | P3 |
| B5 | **Hypervisor orchestrator merge** | Merge fallback to defaults may not cover all fields | Partial | S | P1 |
| B6 | **Auth RBAC beyond login/me** | No middleware enforcing role-based access on all routes | Partial | M | P2 |
| B7 | **Storage pool backend integration** | DB insert only; no LVM/dir validation or provisioning | Partial | M | P2 |
| B8 | **Image import validation** | May not validate qcow2 headers before copy | Partial | S | P2 |
| B9 | **VM template clone backend** | Needs audit for completeness | Partial | S | P2 |
| B10 | **API token auth for agent** | Tokens table exists; not used for gRPC auth | Partial | M | P2 |

### 2.2 Agent / Runtime

| # | Gap | Location | Status | Effort | Priority |
|---|-----|----------|--------|--------|----------|
| A1 | **Stord mock stubs unimplemented** | `reconcile.rs:948-1004` — health, resize, snapshot, clone, device policy | Mock only | M | P1 |
| A2 | **NWD mock stubs unimplemented** | `reconcile.rs:1046-1112` — health, firewall, NAT, DHCP, DNS, expose | Mock only | M | P1 |
| A3 | **Real daemon client parity** | Verify real gRPC clients wire all above methods | Needs audit | M | P1 |
| A4 | **Console token validation** | JWT expiry OK; one-time-use LRU cache and replay prevention are weak | Partial | S | P1 |
| A5 | **Console PTY resize** | `ioctl(TIOCSWINSZ)` not wired | Partial | S | P1 |
| A6 | **operation_id propagation** | `daemon_clients.rs:46,259` — TODO for metadata | TODO | S | P2 |
| A7 | **LVM device policy** | `lvm.rs:288` — ignored | Partial | S | P2 |
| A8 | **NWD LinuxExecutor** | `cmd/chv-nwd/main.rs:16` — TODO | TODO | S | P2 |
| A9 | **CPU percent delta** | Computes delta now; verify persistence across restarts | Partial | S | P2 |

### 2.3 Web UI

| # | Gap | Location | Status | Effort | Priority |
|---|-----|----------|--------|--------|----------|
| U1 | **UI Production Readiness refactor** | Tailwind-first, app.css strip, component split, folder reorg | Not started | L | P1 |
| U2 | **Command palette** | `TopCommandBar.svelte:12` | TODO | M | P2 |
| U3 | **Hypervisor settings page** | `/settings/hypervisor` may exist; needs audit | Partial | M | P1 |
| U4 | **VM create advanced tab** | Hypervisor overrides in `CreateVMModal` | Partial | M | P1 |
| U5 | **Design token drift** | `Button.svelte` orange shadows; `VMMetricsWidget` raw colors | Partial | S | P1 |
| U6 | **DataTable splitting** | 688 lines; needs extraction | Not started | M | P2 |
| U7 | **Overview page logic extraction** | 526 lines; helpers don't exist | Not started | S | P2 |
| U8 | **E2E tests** | No Playwright/Cypress | Missing | L | P3 |
| U9 | **Client-side caching** | No TanStack Query equivalent | Missing | M | P3 |
| U10 | **Dark mode** | P3 scope | Not started | L | P3 |

### 2.4 Infrastructure / DevEx

| # | Gap | Location | Status | Effort | Priority |
|---|-----|----------|--------|--------|----------|
| I1 | **Read-only database on deploy** | `nkudo-vm1` incident — root-owned DB | Bug | S | P0 |
| I2 | **Agent console port collision** | Old PID holds 8444 after restart | Bug | S | P0 |
| I3 | **Nginx WebSocket proxy** | `/ws/vms/` location may be missing | Partial | S | P1 |
| I4 | **CI/CD pipeline** | No GitHub Actions | Missing | M | P2 |
| I5 | **DB backup / rollback** | No pre-migration backup | Missing | M | P2 |
| I6 | **Release versioning** | Not automated | Missing | S | P2 |

---

## 3. Sprint Plan

### Sprint 11 — Stability & Hardening (1 week)
- [ ] **I1** — Fix DB ownership in `install.sh` (`chown chv:chv` after init)
- [ ] **I2** — systemd `KillMode=mixed` + `TimeoutStopSec=5` for agent
- [ ] **A4** — Harden console tokens: expiry + LRU replay cache
- [ ] **A5** — Wire `ioctl(TIOCSWINSZ)` for console resize
- [ ] **A1-A3** — Audit real daemon clients vs mocks; implement resize_volume, prepare_snapshot, set_firewall_policy, ensure_dhcp_scope
- [ ] **B1** — Implement `mutate_network` in BFF mutations
- [ ] **B5** — Complete orchestrator merge logic with full default fallbacks

### Sprint 12 — Hypervisor Settings E2E (1 week)
- [ ] Verify CHV `vm.create` payload includes all hypervisor fields
- [ ] **U3** — Build `/settings/hypervisor` page
- [ ] **U4** — Add Advanced accordion to `CreateVMModal`
- [ ] **U5** — Fix Button shadows and VMMetricsWidget tokens
- [ ] Optional: **B10** — API token auth for agents

### Sprint 13 — UI Production Readiness (2 weeks)
- [ ] **U1** — Rewrite DESIGN.md; strip app.css; migrate primitives/shell to Tailwind
- [ ] **U1** — Reorganize components into feature folders
- [ ] **U6** — Split DataTable into selection/sorting/visibility modules
- [ ] **U7** — Extract overview page logic to helper modules

### Sprint 14 — Quotas, Backups & Observability (1–2 weeks)
- [ ] **B3** — Enforce quotas at VM create time
- [ ] **B2** — Design backup schema; implement backend (not stubs)
- [ ] **B6** — RBAC middleware on all BFF routes
- [ ] **I4** — GitHub Actions: `cargo test`, `cargo clippy`, `npm run build`
- [ ] **I5** — Pre-migration SQLite backup hook

### Sprint 15 — Daemon Parity & Polish (1–2 weeks)
- [ ] **A1-A3 remaining** — get_volume_health, get_network_health, set_nat_policy, expose_service, etc.
- [ ] **A6** — Propagate operation_id via gRPC metadata
- [ ] **A7** — LVM device policy
- [ ] **A8** — NWD LinuxExecutor with ip/nft
- [ ] **U2** — Command palette modal
- [ ] **I3** — Nginx `/ws/vms/` proxy
- [ ] **I6** — Automated version bump

---

## 4. Dependency Graph

```
Sprint 11 (Stability)
  ├─ I1, I2 ──► deployment reliability
  ├─ A4, A5 ──► console hardening
  ├─ A1-A3 ──► daemon parity (foundational)
  └─ B1, B5 ──► backend completeness
       │
       ▼
Sprint 12 (Hypervisor Settings)
  ├─ depends on B5
  ├─ U3, U4, U5
  └─ CHV payload verification
       │
       ▼
Sprint 13 (UI Refactor)
  ├─ U1, U6, U7 (pure UI)
  └─ can parallel with Sprint 12
       │
       ▼
Sprint 14 (Quotas & Backups)
  ├─ B3, B2, B6
  └─ I4, I5
       │
       ▼
Sprint 15 (Daemon Parity & Polish)
  ├─ A1-A3 remaining
  ├─ A6-A8
  └─ U2, I3, I6
```

## 5. Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| UI refactor breaks routes | High | Medium | `svelte-check` after every sub-task |
| Daemon parity needs kernel net | Medium | High | Gate A8 behind feature flag |
| CHV rejects new hypervisor flags | Medium | Medium | Agent surfaces CHV error verbatim |
| SQLite FK issues recur | Low | High | Test every migration with FK=ON |
| Console token replay | Medium | High | Short expiry + LRU consumed cache |

## 6. Success Criteria

1. Zero P0/P1 stubs in BFF mutations, orchestrator, or agent reconcile.
2. All agent daemon RPCs have real implementations.
3. UI passes `svelte-check`, `npm run build`, `npm test` with zero errors.
4. No component file >300 lines (except generated).
5. `dev-install.sh` works on fresh VM without manual `chown`.
6. CI blocks PRs that fail tests or build.
