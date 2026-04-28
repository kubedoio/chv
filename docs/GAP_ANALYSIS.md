# CHV Specification vs Implementation Gap Analysis

**Date:** 2026-04-26  
**Version:** 0.0.0.2  
**Scope:** Backend (Rust), Agent, UI (SvelteKit), Infrastructure  
**Method:** Cross-reference of ADRs 001–010, component specs, ARCHITECTURE.md, DESIGN.md, and PHASED_IMPLEMENTATION_PLAN.md against the actual codebase.

---

## Executive Summary

| Category | Total Gaps | P0 | P1 | P2 | P3 |
|----------|-----------|----|----|----|----|
| Backend / Control Plane | 4 | 0 | 2 | 2 | 0 |
| Agent / Node Runtime | 5 | 1 | 3 | 1 | 0 |
| UI / Web Frontend | 7 | 0 | 2 | 4 | 1 |
| Infrastructure / Deployment | 3 | 0 | 2 | 1 | 0 |
| **Total** | **19** | **1** | **9** | **8** | **1** |

**Critical finding:** The agent has **no implementation of partition policy (ADR-006)**. During a control-plane outage, the agent will continue accepting new VM creation requests and destructive topology mutations, violating a core safety invariant.

---

## Legend

- **P0** — Safety or data-loss risk; blocks production usage
- **P1** — Required for MVP completeness; user-facing broken promise
- **P2** — Quality / maintainability; degrades operator experience
- **P3** — Nice to have; future enhancement
- **Evidence** — File path and line number where the gap is observable

---

## 1. Backend / Control Plane Gaps

### 1.1 VM Resize Accepted but Not Executed
- **Spec:** ADR-002 (desired-state reconciliation), `chv-agent-spec.md` (VM lifecycle)
- **Gap:** `resize_vm` in `crates/chv-controlplane-service/src/lifecycle.rs:710` records the intent and returns an ACK, but the comment explicitly states: "Record intent accepted; actual resize is orchestrated by the reconciler dispatching to the agent's resize_vm RPC." The reconciler dispatch path is incomplete — the resize parameters (`desired_vcpus`, `desired_memory_bytes`) are accepted but not propagated into the desired-state fragment that the agent reconciles.
- **Status:** Partially implemented (API surface exists, orchestration incomplete)
- **Evidence:** `crates/chv-controlplane-service/src/lifecycle.rs:687-712`
- **Priority:** P1

### 1.2 Backup Jobs: Database + API Only, No Execution Engine
- **Spec:** ARCHITECTURE.md ("Backup jobs & history stubbed"), PHASED_IMPLEMENTATION_PLAN.md Phase 3
- **Gap:** Backup tables (`backup_jobs`, `backup_schedules`, `backup_restores`), repositories (`BackupRepository`), and BFF REST handlers all exist. However, there is **no backup execution engine** — no worker that actually takes VM snapshots, ships them to target storage, or validates restoration. The handlers return DB rows, but no backup ever progresses past `queued` status.
- **Status:** Stubbed (data layer complete, execution layer missing)
- **Evidence:** `crates/chv-webui-bff/src/handlers/backups.rs` (read-only handlers), no backup worker found in `crates/chv-controlplane-service/src/`
- **Priority:** P2

### 1.3 Network Mutation End-to-End Incomplete
- **Spec:** ARCHITECTURE.md ("Network mutations (`mutate_network` returns `NotImplemented`)"), ADR-005
- **Gap:** The BFF `mutate_network` handler and control-plane `bff_mutations.rs` service both exist and support `start`, `stop`, `restart` actions. However, the agent-side `apply_network_desired_state` in `reconcile.rs` and `agent_server.rs` may not fully wire the network desired-state schema fields (`firewall_rules_json`, `nat_rules_json`, `dhcp_scope_json`, `dns_enabled`, `dns_scope_json`) into actual `chv-nwd` RPC calls.
- **Status:** Partially implemented (BFF → control plane wired, agent → nwd schema wired but enforcement gaps)
- **Evidence:** `crates/chv-webui-bff/src/handlers/networks.rs:538-563`, `crates/chv-controlplane-service/src/bff_mutations.rs:593-700`
- **Priority:** P1

### 1.4 chv-stord-spec Security Requirements Not Fully Implemented
- **Spec:** `chv-stord-spec.md` (dedicated service account, restricted socket permissions, explicit device/path allowlists, capability drop)
- **Gap:** The spec requires `chv-stord` to run under a dedicated service account with restricted filesystem visibility and device/path allowlists. The current implementation runs as the generic `chv` user with broad `/var/lib/chv/storage/localdisk` access. No allowlist enforcement or capability dropping is present.
- **Status:** Not started
- **Evidence:** `docs/examples/systemd/chv-stord.service`, `crates/chv-stord-core/src/`
- **Priority:** P2

---

## 2. Agent / Node Runtime Gaps

### 2.1 Partition Policy (ADR-006) Not Implemented
- **Spec:** ADR-006 — During control-plane outage: preserve runtime, allow local self-heal, allow stop/reboot, **deny new VM creation, deny migrations, deny destructive topology mutations**.
- **Gap:** There is **zero evidence** in `crates/chv-agent-core/src/` of partition detection or partition-safe behavior. The agent's reconcile loop polls desired state from the control plane indefinitely. If the control plane is unreachable, the agent will likely error-loop but does not explicitly:
  - Detect partition vs. control-plane crash
  - Enter a partition-safe mode
  - Reject `CreateVm`, `MigrateVm`, or topology-mutating RPCs
  - Allow `StopVm`, `RebootVm` for existing VMs
  - Converge back to desired state upon reconnection
- **Status:** Not started
- **Evidence:** `crates/chv-agent-core/src/agent_server.rs`, `crates/chv-agent-core/src/reconcile.rs` — no partition-related code found
- **Priority:** P0 — This is a safety invariant violation.

### 2.2 Reconcile Test Mocks: 14 Unimplemented RPCs
- **Spec:** `chv-agent-spec.md` (idempotent reconcile loop, test coverage), `chv-stord-spec.md`, `chv-nwd-spec.md`
- **Gap:** The test mocks in `reconcile.rs` return `Status::unimplemented("")` for 14 advanced RPCs:
  - Stord: `get_volume_health`, `resize_volume`, `prepare_snapshot`, `prepare_clone`, `restore_snapshot`, `delete_snapshot`, `set_device_policy` (7)
  - Nwd: `get_network_health`, `set_firewall_policy`, `set_nat_policy`, `ensure_dhcp_scope`, `ensure_dns_scope`, `expose_service`, `withdraw_service_exposure` (7)
- **Status:** Stubbed (tests cannot verify these code paths)
- **Evidence:** `crates/chv-agent-core/src/reconcile.rs:1282-1462`
- **Priority:** P1

### 2.3 VM Resize Agent-Side Incomplete
- **Spec:** ADR-002 (desired-state reconciliation)
- **Gap:** The agent's `agent_server.rs:793` has a `resize_vm` RPC handler, and `vm_runtime.rs:142` has a `resize_vm` method. However, the reconcile loop in `reconcile.rs` may not generate resize operations when the desired-state fragment changes `vcpu` or `memory_mb` for an existing VM.
- **Status:** Partially implemented (RPC handler exists, reconcile loop integration unclear)
- **Evidence:** `crates/chv-agent-core/src/agent_server.rs:793`, `crates/chv-agent-core/src/vm_runtime.rs:142`
- **Priority:** P1

### 2.4 Node State Machine: Missing Transition Enforcement
- **Spec:** ADR-003 — Explicit states and transitions; only `TenantReady` nodes receive new VMs.
- **Gap:** `cache.rs` has a `StateMachine` and `transition_node_state` method, but the reconcile loop does not appear to enforce scheduling rules based on node state. A node in `Degraded` or `Draining` may still receive new VM desired-state fragments.
- **Status:** Partially implemented (state machine library exists, enforcement gaps)
- **Evidence:** `crates/chv-agent-core/src/cache.rs:268-276`, `crates/chv-agent-core/src/reconcile.rs`
- **Priority:** P1

### 2.5 chv-nwd DHCP/DNS Enforcement Gap
- **Spec:** `chv-nwd-spec.md` (DHCP, DNS), PHASED_IMPLEMENTATION_PLAN.md Phase 2
- **Gap:** `chv-nwd-core/src/executor.rs` has `ensure_dhcp_scope` and `ensure_dns_scope` methods that start `dnsmasq`, but the reconcile loop may not call these methods when the desired state changes. The PHASED plan explicitly notes: "DHCP and DNS scopes currently log they are accepted but are not enforced."
- **Status:** Partially implemented (executor methods exist, reconcile loop wiring incomplete)
- **Evidence:** `crates/chv-nwd-core/src/executor.rs:57-120`, `crates/chv-agent-core/src/reconcile.rs`
- **Priority:** P2

---

## 3. UI / Web Frontend Gaps

### 3.1 svelte-check: 104 Warnings
- **Spec:** PHASED_IMPLEMENTATION_PLAN.md success criteria: "`svelte-check` reports 0 errors and 0 warnings."
- **Gap:** `npm run check` reports 0 errors but **104 warnings**, primarily:
  - Unused CSS selectors (e.g., `.action-btn.start:hover`, `.action-btn.danger:hover` in `templates/+page.svelte`)
  - A11y label associations
- **Status:** Partially implemented (type-safe, but warnings pollute CI and mask real issues)
- **Evidence:** `npm run check` output
- **Priority:** P2

### 3.2 Components Over 300 Lines
- **Spec:** CLAUDE.md / CONTRIBUTING.md: "Keep Svelte components under ~300 lines"
- **Gap:** 29 components/pages exceed 300 lines. The worst offenders:
  - `TopologyCanvas.svelte` — 986 lines
  - `SidebarNav.svelte` — 688 lines
  - `settings/users/+page.svelte` — 815 lines
  - `vms/[id]/+page.svelte` — 791 lines
  - `CreateVMModal.svelte` — 580 lines
- **Status:** Not started (no refactoring effort underway)
- **Evidence:** `wc -l` across `ui/src/lib/components/` and `ui/src/routes/`
- **Priority:** P2

### 3.3 Playwright E2E Tests Not Run in CI
- **Spec:** PHASED_IMPLEMENTATION_PLAN.md Phase 3: "Set up automated CI (`cargo test`, `cargo clippy`, `npm run build`, Playwright e2e tests)"
- **Gap:** `.github/workflows/ci.yml` runs Rust check/clippy/test and UI build/check, but **does not run Playwright e2e tests**. The `navigation.spec.ts` test for `Ctrl+K` command palette may also be flaky.
- **Status:** Partially implemented (tests exist, CI integration missing)
- **Evidence:** `.github/workflows/ci.yml`, `ui/tests/e2e/navigation.spec.ts`
- **Priority:** P1

### 3.4 Toast Component Hardcoded Colors
- **Spec:** DESIGN.md: "Toast / Notifications: Must use design system semantic colors"
- **Gap:** `Toast.svelte` uses hardcoded hex values (`#54B435`, `#E60000`, `#0066CC`) that do not match the warm earthy palette (`#3f6b45` success, `#9b4338` danger, `#49627d` info).
- **Status:** Not started
- **Evidence:** `ui/src/lib/components/shared/Toast.svelte` (confirmed in DESIGN.md anti-pattern section)
- **Priority:** P2

### 3.5 "awaiting-operator-input" Task State Not Implemented
- **Spec:** ADR-004-WebUI: Required task states include `awaiting-operator-input` (reserved for later)
- **Gap:** The UI task list and task detail components only show: `queued`, `running`, `succeeded`, `failed`, `cancelled`. The `awaiting-operator-input` state has no UI representation.
- **Status:** Not started
- **Evidence:** `ui/src/routes/tasks/+page.svelte`, `ui/src/lib/components/events/EventList.svelte`
- **Priority:** P3 (reserved for later per spec)

### 3.6 InventoryListPage Uses `any` Types
- **Spec:** CONTRIBUTING.md: "Use TypeScript strictly; avoid `any`"
- **Gap:** `InventoryListPage.svelte` props are typed as `any[]` and `any`, defeating table type-safety across all list views.
- **Status:** Not started
- **Evidence:** `ui/src/lib/components/shell/InventoryListPage.svelte`
- **Priority:** P2

### 3.7 A11y Suppressions on Modal Backdrops
- **Spec:** DESIGN.md accessibility expectations, ADR-001-WebUI (predictable mutation UX)
- **Gap:** 6 components use `<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->` on modal backdrop divs, suppressing keyboard accessibility without proper `onkeydown` handlers or native `<dialog>` elements.
- **Status:** Not started
- **Evidence:** `ui/src/lib/components/primitives/Modal.svelte`, `ui/src/lib/components/shell/SearchModal.svelte`, `ui/src/lib/components/shell/CommandPalette.svelte`, `ui/src/lib/components/shell/MobileNav.svelte`, `ui/src/lib/components/shared/QuickActions.svelte`, `ui/src/lib/components/shared/KeyboardShortcutsHelp.svelte`
- **Priority:** P2

---

## 4. Infrastructure / Deployment Gaps

### 4.1 CI Missing Playwright E2E Tests
- **Spec:** PHASED_IMPLEMENTATION_PLAN.md Phase 3: "Set up automated CI (`cargo test`, `cargo clippy`, `npm run build`, Playwright e2e tests)"
- **Gap:** GitHub Actions `.github/workflows/ci.yml` has no Playwright job. The `ui` job only builds and runs `svelte-check`.
- **Status:** Not started
- **Evidence:** `.github/workflows/ci.yml`
- **Priority:** P1

### 4.2 Multi-Node WebSocket Routing Not Implemented
- **Spec:** PHASED_IMPLEMENTATION_PLAN.md Phase 3: "Nginx Routing: configure multi-node WebSocket routing (`/ws/vms/`) using a dynamic upstream based on `node_id`"
- **Gap:** The nginx config at `docs/examples/nginx/chv-ui.conf` hardcodes `proxy_pass http://127.0.0.1:8444/vms/` for WebSocket console access. In a multi-node deployment, console WebSockets must route to the correct hypervisor host based on the VM's node assignment.
- **Status:** Not started
- **Evidence:** `docs/examples/nginx/chv-ui.conf`
- **Priority:** P1

### 4.3 Docker Compose Incomplete
- **Spec:** DEPLOYMENT.md, CONTRIBUTING.md (Docker optional)
- **Gap:** `Dockerfile` exists but `docker-compose.yml` may not configure all four daemons with proper networking, volume mounts, and bridge setup. The Dockerfile comment says "Usage: docker compose up -d" but the compose file may be a basic template.
- **Status:** Partially implemented (Dockerfile present, compose orchestration gaps)
- **Evidence:** `Dockerfile`, `docker-compose.yml`
- **Priority:** P2

---

## 5. Already Implemented (No Gap)

These areas were previously flagged as gaps but are now complete:

| Area | Evidence |
|------|----------|
| **Operation ID propagation** | `crates/chv-agent-core/src/daemon_clients.rs:21-223` — `with_operation_id` wraps all stord/nwd RPCs with `x-operation-id` metadata and tracing spans |
| **Console token replay prevention** | `crates/chv-agent-core/src/console_server.rs:24-151` — 2048-entry LRU cache + 2-second rate limiter per VM |
| **Quota enforcement** | `crates/chv-webui-bff/src/handlers/vms.rs:390-988` — `enforce_user_quota` checks max_vms, max_cpu, max_memory_bytes, max_storage_bytes |
| **RBAC middleware** | `crates/chv-webui-bff/src/router.rs:35-400` — Three-tier routing (viewer / operator / admin) with `require_operator_or_admin` and `require_admin` gates |
| **Node state machine library** | `crates/chv-agent-core/src/cache.rs:268-276` — `StateMachine` with `transition_node_state` and transition validation |
| **nginx WebSocket proxy** | `docs/examples/nginx/chv-ui.conf` — `/ws/vms/` location with upgrade headers |
| **systemd services** | `docs/examples/systemd/*.service` — All 4 daemons with `KillMode=mixed` and `TimeoutStopSec=5` |
| **Pre-migration DB backup** | CHANGELOG.md: "Pre-migration SQLite backup hook (I5): automatic DB backup before migrations with 10-backup rotation" |
| **Version bump automation** | `scripts/bump-version.sh` + `Makefile` target |
| **Dark mode** | CHANGELOG.md: "full implementation with UserMenu toggle" |
| **Command palette** | `ui/src/lib/components/shell/CommandPalette.svelte` — fuzzy-search with 16 commands |
| **Design token alignment** | CHANGELOG.md: "earthy palette applied to app.css, tailwind.config.cjs, and all 8 drifted components" |
| **Error handling alignment** | `crates/chv-webui-bff/src/error.rs` — no `unreachable!()` panic paths |
| **Logging alignment** | `crates/chv-config/src/lib.rs` — `tracing` replaces `eprintln!` |
| **Async runtime safety** | `crates/chv-agent-core/src/console_server.rs`, `crates/chv-controlplane-service/src/container.rs` — `tokio::sync::Mutex` in async paths |

---

## Appendix A: Gap-to-Spec Cross-Reference

| Gap | ADR | Component Spec | Plan Phase |
|-----|-----|---------------|------------|
| 1.1 VM resize not executed | ADR-002 | chv-agent-spec | Phase 2 |
| 1.2 Backup no execution engine | — | — | Phase 3 |
| 1.3 Network mutation incomplete | ADR-005 | chv-nwd-spec | Phase 2 |
| 1.4 stord security hardening | — | chv-stord-spec | — |
| 2.1 Partition policy missing | ADR-006 | chv-agent-spec | — |
| 2.2 Reconcile test mocks | — | chv-agent-spec | Phase 1 |
| 2.3 VM resize agent-side | ADR-002 | chv-agent-spec | Phase 2 |
| 2.4 State machine enforcement | ADR-003 | chv-agent-spec | — |
| 2.5 DHCP/DNS enforcement | — | chv-nwd-spec | Phase 2 |
| 3.1 svelte-check warnings | — | — | Phase 1 |
| 3.2 Components >300 lines | CLAUDE.md | — | Phase 3 |
| 3.3 Playwright not in CI | — | — | Phase 3 |
| 3.4 Toast hardcoded colors | DESIGN.md | — | Phase 3 |
| 3.5 awaiting-operator-input | ADR-004-WebUI | — | — |
| 3.6 InventoryListPage any | CONTRIBUTING.md | — | — |
| 3.7 A11y suppressions | DESIGN.md | — | Phase 1 |
| 4.1 CI missing Playwright | — | — | Phase 3 |
| 4.2 Multi-node WS routing | — | — | Phase 3 |
| 4.3 Docker compose gaps | DEPLOYMENT.md | — | — |

---

## Appendix B: Files Examined

**Specs:** `docs/ARCHITECTURE.md`, `docs/DEPLOYMENT.md`, `docs/OPERATIONS.md`, `PHASED_IMPLEMENTATION_PLAN.md`, `DESIGN.md`, `CLAUDE.md`, `CONTRIBUTING.md`, all ADRs 001–010, all component specs.

**Backend:** `crates/chv-controlplane-service/src/lifecycle.rs`, `bff_mutations.rs`, `orchestrator.rs`, `reconcile.rs`, `server.rs`, `api/stub.rs`, `api/router.rs`; `crates/chv-webui-bff/src/handlers/*.rs`, `router.rs`, `mutations.rs`, `error.rs`.

**Agent:** `crates/chv-agent-core/src/reconcile.rs`, `agent_server.rs`, `vm_runtime.rs`, `console_server.rs`, `cache.rs`, `daemon_clients.rs`, `supervisor.rs`; `crates/chv-nwd-core/src/executor.rs`.

**UI:** `ui/src/routes/**/*.svelte`, `ui/src/lib/components/**/*.svelte`, `ui/src/lib/api/client.ts`, `ui/tests/e2e/navigation.spec.ts`.

**Infra:** `.github/workflows/ci.yml`, `docs/examples/nginx/chv-ui.conf`, `docs/examples/systemd/*.service`, `Dockerfile`, `docker-compose.yml`, `scripts/*.sh`.
