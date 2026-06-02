# CHV Specification vs Implementation Gap Analysis

**Date:** 2026-05-29  
**Version:** 0.1.0  
**Scope:** Backend (Rust), Agent, UI (SvelteKit), Infrastructure  
**Method:** Cross-reference of ADRs 001–013, component specs, ARCHITECTURE.md, DESIGN.md, and PHASED_IMPLEMENTATION_PLAN.md against the actual codebase.

---

## Executive Summary

| Category | Total Gaps | P0 | P1 | P2 | P3 |
|----------|-----------|----|----|----|----|
| Backend / Control Plane | 3 | 0 | 1 | 2 | 0 |
| Agent / Node Runtime | 1 | 0 | 1 | 0 | 0 |
| UI / Web Frontend | 3 | 0 | 0 | 2 | 1 |
| Infrastructure / Deployment | 2 | 0 | 1 | 1 | 0 |
| **Total** | **9** | **0** | **3** | **5** | **1** |

**Previously reported gaps that are now resolved:**
- Partition policy (ADR-006) is fully implemented via `ConnectivityTracker`, `flush_pending_messages`, and agent-side RPC rejection.
- VM resize is wired end-to-end (BFF → desired state → agent reconcile → Cloud Hypervisor).
- Network mutations (`start`/`stop`/`restart`) are wired through BFF → control plane → agent → nwd.
- svelte-check reports **0 errors and 0 warnings**.
- Playwright E2E tests run in CI (`.github/workflows/ci.yml` `e2e` job).
- Toast component uses design-system CSS variables (no hardcoded hex).
- TopologyCanvas, SidebarNav, and settings/users page are all under 300 lines after refactor.
- There are **zero** `unimplemented!()` stubs in production Rust code.

---

## Legend

- **P0** — Safety or data-loss risk; blocks production usage
- **P1** — Required for MVP completeness; user-facing broken promise
- **P2** — Quality / maintainability; degrades operator experience
- **P3** — Nice to have; future enhancement
- **Evidence** — File path and line number where the gap is observable

---

## 1. Backend / Control Plane Gaps

### 1.1 Disk Migration Dirty Sync and Final Flush Incomplete
- **Spec:** ADR-012, `chv-stord-spec.md`, live-migration-spec.md
- **Gap:** Control-plane migration orchestration, mTLS, backpressure, flow control, rollback, and the `MigrationReaper` are all implemented. The remaining gap is in the stord sender: dirty sync rounds, convergence reporting to the control plane, and the paused final dirty flush are not yet performed. This means live migration works for bulk copy but does not yet do the iterative dirty-block sync required for zero-downtime migration of write-heavy workloads.
- **Status:** Partial (bulk copy and orchestration complete; dirty sync / final flush missing)
- **Evidence:** `crates/chv-stord-core/src/migration/sender.rs`, `docs/specs/component/chv-stord-spec.md`
- **Priority:** P1

### 1.2 Backup Jobs: Partial Execution Engine, DR Semantics Incomplete
- **Spec:** ARCHITECTURE.md, PHASED_IMPLEMENTATION_PLAN.md Phase 3
- **Gap:** Backup tables (`backup_jobs`, `backup_schedules`, `backup_restores`), repositories (`BackupRepository`), BFF REST handlers, and a control-plane `BackupWorker` exist. The remaining gap is production Backup/DR semantics: off-host artifact shipping, restore execution/validation, retention enforcement, integrity checks, and documented DR runbooks are not complete.
- **Status:** Partial (scheduler/executor exists; DR workflow incomplete)
- **Evidence:** `crates/chv-controlplane-service/src/backup_worker.rs`, `crates/chv-webui-bff/src/handlers/backups.rs`
- **Priority:** P2

### 1.3 iSCSI and Ceph RBD Storage Backend Adapters Planned but Not Production-Complete
- **Spec:** ADR-004, `chv-stord-spec.md`
- **Gap:** `chv-stord-backends` contains `iscsi.rs` (949 lines) and `ceph.rs` (900 lines) with substantial adapter code, but these backends are not integrated into the active stord handler path as production-complete options. The active backend focus remains local file/qcow2 and LVM.
- **Status:** Partial (adapter code exists; not wired as production default)
- **Evidence:** `crates/chv-stord-backends/src/iscsi.rs`, `crates/chv-stord-backends/src/ceph.rs`, `crates/chv-stord-core/src/handlers.rs`
- **Priority:** P2

---

## 2. Agent / Node Runtime Gaps

### 2.1 chv-stord-spec Security Requirements Not Fully Implemented
- **Spec:** `chv-stord-spec.md` (dedicated service account, restricted socket permissions, explicit device/path allowlists, capability drop)
- **Gap:** The spec requires `chv-stord` to run under a dedicated service account with restricted filesystem visibility and device/path allowlists. The current implementation runs as the generic `chv` user with broad `/var/lib/chv/storage/localdisk` access. No allowlist enforcement or capability dropping is present.
- **Status:** Not started
- **Evidence:** `docs/examples/systemd/chv-stord.service`, `crates/chv-stord-core/src/`
- **Priority:** P1

---

## 3. UI / Web Frontend Gaps

### 3.1 Components Over 300 Lines
- **Spec:** CLAUDE.md / CONTRIBUTING.md: "Keep Svelte components under ~300 lines"
- **Gap:** 5 components/pages still exceed 300 lines:
  - `vms/[id]/+page.svelte` — 467 lines
  - `CreateVMModal.svelte` — 580 lines
  - `DataTable.svelte` — still the primary table component (extracted sub-modules exist but main file may still be large)
- **Status:** Partially addressed (TopologyCanvas, SidebarNav, settings/users, and Dashboard all refactored below 300 lines)
- **Evidence:** `wc -l` across `ui/src/lib/components/` and `ui/src/routes/`
- **Priority:** P2

### 3.2 InventoryListPage Uses `any` Types
- **Spec:** CONTRIBUTING.md: "Use TypeScript strictly; avoid `any`"
- **Gap:** `InventoryListPage.svelte` props are typed as `any[]` and `any`, defeating table type-safety across all list views.
- **Status:** Not started
- **Evidence:** `ui/src/lib/components/shell/InventoryListPage.svelte:32-35`
- **Priority:** P2

### 3.3 "awaiting-operator-input" Task State Not Implemented
- **Spec:** ADR-004-WebUI: Required task states include `awaiting-operator-input` (reserved for later)
- **Gap:** The UI task list and task detail components only show: `queued`, `running`, `succeeded`, `failed`, `cancelled`. The `awaiting-operator-input` state has no UI representation.
- **Status:** Not started
- **Evidence:** `ui/src/routes/tasks/+page.svelte`, `ui/src/lib/components/events/EventList.svelte`
- **Priority:** P3 (reserved for later per spec)

---

## 4. Infrastructure / Deployment Gaps

### 4.1 Multi-Node WebSocket Routing Not Implemented
- **Spec:** PHASED_IMPLEMENTATION_PLAN.md Phase 3: "Nginx Routing: configure multi-node WebSocket routing (`/ws/vms/`) using a dynamic upstream based on `node_id`"
- **Gap:** The nginx config at `docs/examples/nginx/chv-ui.conf` hardcodes `proxy_pass http://127.0.0.1:8444/vms/` for WebSocket console access. In a multi-node deployment, console WebSockets must route to the correct hypervisor host based on the VM's node assignment.
- **Status:** Not started
- **Evidence:** `docs/examples/nginx/chv-ui.conf`
- **Priority:** P1

### 4.2 Docker Compose Incomplete for Production Use
- **Spec:** DEPLOYMENT.md, CONTRIBUTING.md (Docker optional)
- **Gap:** `docker-compose.yml` exists and configures all four daemons plus nginx for local development, but it is not documented as production-ready. Bridge setup, KVM device passthrough, and host networking requirements for `chv-nwd` make containerized production deployment non-trivial. The compose file is primarily a dev stack.
- **Status:** Partial (dev stack works; production container orchestration undocumented)
- **Evidence:** `docker-compose.yml`, `Dockerfile`
- **Priority:** P2

---

## 5. Already Implemented (No Gap)

These areas were previously flagged as gaps but are now complete:

| Area | Evidence |
|------|----------|
| **Partition policy (ADR-006)** | `crates/chv-agent-core/src/connectivity.rs` — `ConnectivityTracker` with `Connected`/`Disconnected`/`Reconnecting` states; `agent_server.rs:554` rejects `CreateVm` when disconnected; `agent_server.rs:1817` rejects `MigrateVm` when disconnected; `control_plane.rs:276` `flush_pending_messages` drains `NodeCache::pending_control_plane_messages` on reconnect |
| **VM resize end-to-end** | `crates/chv-webui-bff/src/handlers/vms.rs:733-830` — BFF updates `vm_desired_state` cpu_count/memory_bytes and bumps generation; `crates/chv-agent-core/src/reconcile.rs:1318-1339` — agent detects drift and calls `vm_runtime.resize_vm()` |
| **Network mutations end-to-end** | `crates/chv-webui-bff/src/handlers/networks.rs` — BFF lifecycle handlers; `crates/chv-agent-core/src/reconcile.rs:515-609` — agent calls `set_firewall_policy`, `set_nat_policy`, `ensure_dhcp_scope`, `ensure_dns_scope` via nwd client |
| **svelte-check warnings** | `npm run check` reports **0 errors and 0 warnings** |
| **Playwright E2E in CI** | `.github/workflows/ci.yml` has dedicated `e2e` job running Playwright against a mock BFF server |
| **Toast design tokens** | `ui/src/lib/components/primitives/Toast.svelte` uses `var(--color-success)`, `var(--color-danger)`, `var(--color-info)` |
| **TopologyCanvas refactor** | Now 299 lines (was 986) |
| **SidebarNav refactor** | Now 259 lines (was 688) |
| **settings/users refactor** | Now 231 lines (was 815) |
| **No production `unimplemented!()` stubs** | `grep -rn "unimplemented!()" crates/` returns only test-mock implementations |
| **Operation ID propagation** | `crates/chv-agent-core/src/daemon_clients.rs:21-223` — `with_operation_id` wraps all stord/nwd RPCs with `x-operation-id` metadata and tracing spans |
| **Console token replay prevention** | `crates/chv-agent-core/src/console_server.rs:24-151` — 2048-entry LRU cache + 2-second rate limiter per VM |
| **Quota enforcement** | `crates/chv-webui-bff/src/handlers/vms.rs:390-988` — `enforce_user_quota` checks max_vms, max_cpu, max_memory_bytes, max_storage_bytes |
| **RBAC middleware** | `crates/chv-webui-bff/src/router.rs:35-400` — Three-tier routing (viewer / operator / admin) with `require_operator_or_admin` and `require_admin` gates |
| **Node state machine library** | `crates/chv-agent-core/src/cache.rs:268-276` — `StateMachine` with `transition_node_state` and transition validation |
| **nginx WebSocket proxy** | `docs/examples/nginx/chv-ui.conf` — `/ws/vms/` location with upgrade headers |
| **systemd services** | `docs/examples/systemd/*.service` — All 4 daemons with `KillMode=mixed` and `TimeoutStopSec=5` |
| **Pre-migration DB backup** | Automatic DB backup before migrations with 10-backup rotation |
| **Version bump automation** | `scripts/bump-version.sh` + `Makefile` target |
| **Dark mode** | Full implementation with `UserMenu` toggle and `[data-theme="dark"]` tokens |
| **Command palette** | `ui/src/lib/components/shell/CommandPalette.svelte` — fuzzy-search with 16 commands |
| **Design token alignment** | Earthy palette applied to `app.css`, `tailwind.config.cjs`, and all components |
| **Error handling alignment** | `crates/chv-webui-bff/src/error.rs` — no `unreachable!()` panic paths |
| **Logging alignment** | `crates/chv-config/src/lib.rs` — `tracing` replaces `eprintln!` |
| **Async runtime safety** | `crates/chv-agent-core/src/console_server.rs`, `crates/chv-controlplane-service/src/container.rs` — `tokio::sync::Mutex` in async paths |
| **Circuit breaker** | `crates/chv-controlplane-service/src/circuit_breaker.rs` — `Closed` → `Open` → `HalfOpen` with configurable thresholds |
| **Deep health checks** | `GET /health/deep` returns database, agent socket, and agent connectivity status |
| **Migration reaper** | `crates/chv-controlplane-service/src/migration_reaper.rs` — scans every 60s for migrations stuck beyond 2h |
| **Drain evacuation** | `crates/chv-agent-core/src/reconcile.rs` — `Draining` state triggers migration requests |
| **VXLAN teardown** | `crates/chv-nwd-core/src/executor.rs` — `delete_topology` cleans up VXLAN interfaces and FDB entries |
| **FDB cleanup on VM detach** | Implemented in `chv-nwd-core` executor and reconcile modules |

---

## Appendix A: Gap-to-Spec Cross-Reference

| Gap | ADR | Component Spec | Plan Phase |
|-----|-----|---------------|------------|
| 1.1 Disk migration dirty sync/final flush | ADR-012 | chv-stord-spec | Phase 3 |
| 1.2 Backup no execution engine | — | — | Phase 3 |
| 1.3 iSCSI / Ceph adapters | ADR-004 | chv-stord-spec | Phase 2 |
| 2.1 stord security hardening | — | chv-stord-spec | — |
| 3.1 Components >300 lines | CLAUDE.md | — | Phase 3 |
| 3.2 InventoryListPage `any` types | CONTRIBUTING.md | — | — |
| 3.3 awaiting-operator-input | ADR-004-WebUI | — | — |
| 4.1 Multi-node WS routing | — | — | Phase 3 |
| 4.2 Docker compose production | DEPLOYMENT.md | — | — |

---

## Appendix B: Files Examined

**Specs:** `docs/ARCHITECTURE.md`, `docs/DEPLOYMENT.md`, `docs/OPERATIONS.md`, `PHASED_IMPLEMENTATION_PLAN.md`, `DESIGN.md`, `CLAUDE.md`, `CONTRIBUTING.md`, all ADRs 001–013, all component specs.

**Backend:** `crates/chv-controlplane-service/src/lifecycle.rs`, `bff_mutations.rs`, `orchestrator.rs`, `reconcile.rs`, `server.rs`, `circuit_breaker.rs`, `migration_reaper.rs`; `crates/chv-webui-bff/src/handlers/*.rs`, `router.rs`, `mutations.rs`, `error.rs`.

**Agent:** `crates/chv-agent-core/src/reconcile.rs`, `agent_server.rs`, `vm_runtime.rs`, `console_server.rs`, `cache.rs`, `daemon_clients.rs`, `supervisor.rs`, `connectivity.rs`, `control_plane.rs`; `crates/chv-nwd-core/src/executor.rs`, `reconcile.rs`, `handlers.rs`; `crates/chv-stord-core/src/handlers.rs`, `migration/sender.rs`.

**UI:** `ui/src/routes/**/*.svelte`, `ui/src/lib/components/**/*.svelte`, `ui/src/lib/api/client.ts`, `ui/tests/e2e/*.ts`.

**Infra:** `.github/workflows/ci.yml`, `docs/examples/nginx/chv-ui.conf`, `docs/examples/systemd/*.service`, `Dockerfile`, `docker-compose.yml`, `scripts/*.sh`.
