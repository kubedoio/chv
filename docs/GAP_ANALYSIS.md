# CHV Specification vs Implementation Gap Analysis

**Date:** 2026-06-02  
**Version:** 0.1.0  
**Scope:** Backend (Rust), Agent, UI (SvelteKit), Infrastructure  
**Method:** Cross-reference of ADRs 001–013, component specs, ARCHITECTURE.md, DESIGN.md, and PHASED_IMPLEMENTATION_PLAN.md against the actual codebase.

---

## Executive Summary

| Category | Total Gaps | P0 | P1 | P2 | P3 |
|----------|-----------|----|----|----|----|
| Backend / Control Plane | 1 | 0 | 0 | 1 | 0 |
| Agent / Node Runtime | 0 | 0 | 0 | 0 | 0 |
| UI / Web Frontend | 1 | 0 | 0 | 1 | 0 |
| Infrastructure / Deployment | 0 | 0 | 0 | 0 | 0 |
| **Total** | **2** | **0** | **0** | **2** | **0** |

**Previously reported gaps that are now resolved:**
- Partition policy (ADR-006) is fully implemented via `ConnectivityTracker`, `flush_pending_messages`, and agent-side RPC rejection.
- VM resize is wired end-to-end (BFF → desired state → agent reconcile → Cloud Hypervisor).
- Network mutations (`start`/`stop`/`restart`) are wired through BFF → control plane → agent → nwd.
- svelte-check reports **0 errors and 0 warnings**.
- Playwright E2E tests run in CI (`.github/workflows/ci.yml` `e2e` job).
- Toast component uses design-system CSS variables (no hardcoded hex).
- TopologyCanvas, SidebarNav, and settings/users page are all under 300 lines after refactor.
- There are **zero** `unimplemented!()` stubs in production Rust code.
- **`awaiting-operator-input` task state** implemented in `OperationStatus`, UI task list, and migration reaper exclusion.
- **Disk migration dirty sync + final flush** implemented end-to-end: stord `TriggerDiskMigration`/`GetDiskMigrationStatus`/`ResumeDiskMigration` RPCs, `MigrationTaskTable` with VM-pause coordination, agent progress polling and reporting to control plane.
- **chv-stord security hardening** implemented: dedicated `chv-stord` service account, path/device allowlists, capability dropping in systemd, socket `chown` to `chv-stord` group.
- **Multi-node WebSocket routing** implemented: nginx `map`-based dynamic upstream by `node_id`, BFF proxied-mode URL generation, documented in `DEPLOYMENT.md`.

---

## Legend

- **P0** — Safety or data-loss risk; blocks production usage
- **P1** — Required for MVP completeness; user-facing broken promise
- **P2** — Quality / maintainability; degrades operator experience
- **P3** — Nice to have; future enhancement
- **Evidence** — File path and line number where the gap is observable

---

## 1. Backend / Control Plane Gaps

### 1.1 Disk Migration Dirty Sync and Final Flush — ✅ RESOLVED 2026-06-02
- **Spec:** ADR-012, `chv-stord-spec.md`, live-migration-spec.md
- **Resolution:** Implemented end-to-end with stord `TriggerDiskMigration`/`GetDiskMigrationStatus`/`ResumeDiskMigration` RPCs, `MigrationTaskTable` with VM-pause coordination via `tokio::sync::watch`, agent progress polling every 5s, and control-plane progress reporting via `PendingControlPlaneMessage` queue.
- **Evidence:** `proto/node/chv-stord-api.proto`, `crates/chv-stord-core/src/migration/task.rs`, `crates/chv-stord-core/src/handlers.rs:844-1067`, `crates/chv-agent-core/src/migration.rs:229-410`, `crates/chv-agent-core/src/daemon_clients.rs:422-520`
- **Priority:** P1

### 1.2 Backup Jobs: Execution Engine Complete — Restore/DR Runbooks Pending
- **Spec:** ARCHITECTURE.md, PHASED_IMPLEMENTATION_PLAN.md Phase 3
- **Status:** Core execution engine resolved (2026-06-03). Backup shipper trait (Null, NFS, S3 with streaming upload) wired into `BackupWorker`. VM-level snapshots are staged, shipped to remote destinations, and DB records are updated with checksum, size, remote path, and storage backend. `retention_days` enforcement works alongside `retention_count` pruning, with correct remote artifact deletion using `job.destination` (original URL) to build the shipper and `job.target_path` (shipped key/path) for deletion. S3 credentials are configurable per-schedule. 9 unit tests cover shipper implementations.

  **Post-implementation review fixes (2026-06-03):**
  - Race condition in scheduled job creation eliminated via optimistic locking (`try_claim_schedule_run`)
  - Count-based retention pruning now cleans up remote artifacts before deleting DB rows (`list_old_jobs_for_count_retention` + `delete_jobs_by_ids`)
  - S3 credentials encrypted at rest with AES-256-GCM (`CredentialEncryption`), key from `CHV_ENCRYPTION_KEY` or `CHV_JWT_SECRET`
  - BFF duplicate JSON keys removed; destination tracking preserved across job creation and re-run paths

  Remaining gaps: volume-level `snapshot_volume` shipping (requires agent protocol changes), restore execution/validation, and documented DR runbooks.
- **Evidence:** `crates/chv-controlplane-service/src/backup_shipper.rs`, `crates/chv-controlplane-service/src/backup_worker.rs`, `crates/chv-controlplane-store/src/backups.rs`, `crates/chv-controlplane-store/src/credential_crypto.rs`, `cmd/chv-controlplane/migrations/0043_backup_destination_and_credentials.sql`
- **Priority:** P2

### 1.3 iSCSI and Ceph RBD Storage Backend Adapters — ✅ RESOLVED 2026-06-03
- **Spec:** ADR-004, `chv-stord-spec.md`
- **Resolution:** Both backends implement the full `StorageBackend` trait (open, close, attach, detach, health, resize, snapshot, clone, dirty tracking, read/write block, migration). They are selectable at runtime in `cmd/chv-stord/src/main.rs` via `backend_type = "iscsi"` or `backend_type = "ceph"` with corresponding config sections. Config parsing (`chv-config`) supports `StordIscsiConfig` and `StordCephConfig`. The generic `StorageServer<B>` and `StorageMigrationServiceImpl<B>` work with `Box<dyn StorageBackend>` via the blanket impl. All backends compile, pass clippy, and have unit tests.
- **Evidence:** `crates/chv-stord-backends/src/iscsi.rs`, `crates/chv-stord-backends/src/ceph.rs`, `cmd/chv-stord/src/main.rs:43-77`, `crates/chv-config/src/lib.rs:237-242`
- **Priority:** P2

---

## 2. Agent / Node Runtime Gaps

### 2.1 chv-stord-spec Security Requirements — ✅ RESOLVED 2026-06-02
- **Spec:** `chv-stord-spec.md` (dedicated service account, restricted socket permissions, explicit device/path allowlists, capability drop)
- **Resolution:** Dedicated `chv-stord` system user/group created in `install.sh` and `postinstall.sh`. `path_allowlist` and `device_allowlist` enforced in `StorageServiceImpl::open_volume`. Systemd service updated with `CapabilityBoundingSet=CAP_SYS_ADMIN CAP_MKNOD CAP_DAC_OVERRIDE`, `RestrictAddressFamilies=AF_UNIX`, `RestrictSUIDSGID=true`, and socket `chown` to `chv-stord` group.
- **Evidence:** `scripts/install.sh:203-235`, `packaging/scripts/postinstall.sh:15-40`, `docs/examples/systemd/chv-stord.service`, `crates/chv-stord-core/src/handlers.rs:108-213`
- **Priority:** P1

---

## 3. UI / Web Frontend Gaps

### 3.1 Components Over 300 Lines
- **Spec:** CLAUDE.md / CONTRIBUTING.md: "Keep Svelte components under ~300 lines"
- **Gap:** 2 components/pages still exceed 300 lines:
  - `CreateVMModal.svelte` — ~580 lines
  - `DataTable.svelte` — still the primary table component (extracted sub-modules exist but main file may still be large)
- **Status:** Partially addressed (2026-06-03). `vms/[id]/+page.svelte` refactored from 467 lines to ~300 lines by extracting `VmDetailSummaryTab` (overview), `VmConsoleTab`, `VmMetricsTab`, `VmBootLogTab` (formerly `VmSettingsTab`), `VmTasksTab`, and `VmDetailHeader`. `VmOverviewTab` pass-through wrapper removed. TopologyCanvas, SidebarNav, settings/users, and Dashboard all refactored below 300 lines.
- **Evidence:** `wc -l` across `ui/src/lib/components/` and `ui/src/routes/`
- **Priority:** P2

### 3.2 InventoryListPage Uses `any` Types — ✅ RESOLVED 2026-06-03
- **Spec:** CONTRIBUTING.md: "Use TypeScript strictly; avoid `any`"
- **Resolution:** Added Svelte 5 generic parameter `<T extends Record<string, unknown>>` to `InventoryListPage.svelte`. Replaced `any[]` props with `ColumnDef<T>[]`, `T[]`, and typed `rowHref`/`cell` snippets. All list views (VMs, volumes, networks, nodes, tasks) now get type-safe column and row typing.
- **Evidence:** `ui/src/lib/components/shell/InventoryListPage.svelte:1-45`
- **Priority:** P2

### 3.3 "awaiting-operator-input" Task State — ✅ RESOLVED 2026-06-02
- **Spec:** ADR-004-WebUI: Required task states include `awaiting-operator-input` (reserved for later)
- **Resolution:** Added `AwaitingOperatorInput` to `OperationStatus` enum with `is_active()` helper. UI displays the state with a warning-tone badge, includes it in active ops count, and the migration reaper excludes it from timeout failures.
- **Evidence:** `crates/chv-controlplane-types/src/domain.rs:280`, `ui/src/lib/webui/tasks.ts:137-142`, `ui/src/routes/tasks/+page.svelte:99-103`
- **Priority:** P3

---

## 4. Infrastructure / Deployment Gaps

### 4.1 Multi-Node WebSocket Routing — ✅ RESOLVED 2026-06-02
- **Spec:** PHASED_IMPLEMENTATION_PLAN.md Phase 3: "Nginx Routing: configure multi-node WebSocket routing (`/ws/vms/`) using a dynamic upstream based on `node_id`"
- **Resolution:** nginx `map $request_uri $ws_backend` selects the correct agent backend by `node_id` prefix. Regex location rewrites `/ws/vms/{node_id}/{vm_id}/console` to `/vms/{vm_id}/console` before proxying. BFF returns node-routed URLs when `proxied=true` or `agent_ws_address` is empty. Direct `ws://` URLs still work for single-node deployments.
- **Evidence:** `docs/examples/nginx/chv-ui.conf:45-85`, `crates/chv-webui-bff/src/handlers/vms.rs:1047-1062`, `docs/DEPLOYMENT.md`
- **Priority:** P1

### 4.2 Docker Compose Incomplete for Production Use — ✅ RESOLVED 2026-06-03
- **Spec:** DEPLOYMENT.md, CONTRIBUTING.md (Docker optional)
- **Resolution:** Added `docker-compose.prod.yml` with production-oriented configuration: health checks for all services, `read_only: true` root filesystems with `tmpfs` overlays, dedicated service users (`chv`, `chv-stord`), KVM device passthrough for `chv-agent`, host networking for `chv-nwd`, proper volume sharing between services via named Docker volumes, and nginx reverse proxy with WebSocket upgrade support. `Dockerfile` updated with runtime dependencies, service user creation, and additional exposed ports.
- **Evidence:** `docker-compose.prod.yml`, `Dockerfile`
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

| Gap | ADR | Component Spec | Plan Phase | Status |
|-----|-----|---------------|------------|--------|
| 1.1 Disk migration dirty sync/final flush | ADR-012 | chv-stord-spec | Phase 3 | ✅ Resolved |
| 1.2 Backup execution engine | — | — | Phase 3 | 🟡 Partial (shipper wired, retention fixed, tests added; restore/DR runbooks pending) |
| 1.3 iSCSI / Ceph adapters | ADR-004 | chv-stord-spec | Phase 2 | ✅ Resolved |
| 2.1 stord security hardening | — | chv-stord-spec | — | ✅ Resolved |
| 3.1 Components >300 lines | CLAUDE.md | — | Phase 3 | 🟡 Partial (CreateVMModal, DataTable remain) |
| 3.2 InventoryListPage `any` types | CONTRIBUTING.md | — | — | ✅ Resolved |
| 3.3 awaiting-operator-input | ADR-004-WebUI | — | — | ✅ Resolved |
| 4.1 Multi-node WS routing | — | — | Phase 3 | ✅ Resolved |
| 4.2 Docker compose production | DEPLOYMENT.md | — | — | ✅ Resolved |

---

## Appendix B: Files Examined

**Specs:** `docs/ARCHITECTURE.md`, `docs/DEPLOYMENT.md`, `docs/OPERATIONS.md`, `PHASED_IMPLEMENTATION_PLAN.md`, `DESIGN.md`, `CLAUDE.md`, `CONTRIBUTING.md`, all ADRs 001–013, all component specs.

**Backend:** `crates/chv-controlplane-service/src/lifecycle.rs`, `bff_mutations.rs`, `orchestrator.rs`, `reconcile.rs`, `server.rs`, `circuit_breaker.rs`, `migration_reaper.rs`; `crates/chv-webui-bff/src/handlers/*.rs`, `router.rs`, `mutations.rs`, `error.rs`.

**Agent:** `crates/chv-agent-core/src/reconcile.rs`, `agent_server.rs`, `vm_runtime.rs`, `console_server.rs`, `cache.rs`, `daemon_clients.rs`, `supervisor.rs`, `connectivity.rs`, `control_plane.rs`; `crates/chv-nwd-core/src/executor.rs`, `reconcile.rs`, `handlers.rs`; `crates/chv-stord-core/src/handlers.rs`, `migration/sender.rs`.

**UI:** `ui/src/routes/**/*.svelte`, `ui/src/lib/components/**/*.svelte`, `ui/src/lib/api/client.ts`, `ui/tests/e2e/*.ts`.

**Infra:** `.github/workflows/ci.yml`, `docs/examples/nginx/chv-ui.conf`, `docs/examples/systemd/*.service`, `Dockerfile`, `docker-compose.yml`, `scripts/*.sh`.
