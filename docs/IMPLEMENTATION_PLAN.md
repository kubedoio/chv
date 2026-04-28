# CHV Gap Closure Implementation Plan

**Date:** 2026-04-26  
**Based on:** `docs/GAP_ANALYSIS.md`  
**Target:** Close all P0 and P1 gaps; reduce P2 gaps by 75%.

---

## Sprint Structure

| Sprint | Theme | Focus Areas | Duration |
|--------|-------|-------------|----------|
| Sprint 16 | Agent Safety & Stability | Partition policy, state machine enforcement, test mocks | 1 week |
| Sprint 17 | Feature Completion | VM resize E2E, network mutations, DHCP/DNS enforcement | 1 week |
| Sprint 18 | UI Production Readiness | svelte-check warnings, component splitting, a11y, CI e2e | 1 week |
| Sprint 19 | Infrastructure & Polish | Multi-node WS routing, Docker compose, backup execution, design system alignment | 1 week |

---

## Sprint 16: Agent Safety & Stability

**Goal:** Fix the P0 partition policy gap and close the largest agent test coverage holes.

### Task 16.1: Implement Partition Policy (ADR-006)
**Priority:** P0  
**Assignee:** Backend (Agent)  
**Estimate:** 3 days

**Acceptance Criteria:**
- [ ] Agent detects control-plane unreachable state (gRPC timeout / connection failure)
- [ ] Agent enters `Partitioned` sub-mode when control plane is unreachable
- [ ] In partitioned mode, agent rejects `CreateVm`, `MigrateVm`, and topology-mutating RPCs with `Unavailable` status
- [ ] In partitioned mode, agent allows `StopVm`, `RebootVm` for existing VMs
- [ ] Upon reconnection, agent flushes deferred reports and converges to desired state
- [ ] Unit tests cover partition detection, rejection, and recovery

**Implementation Notes:**
- Add a `partition_detector` module in `chv-agent-core` that tracks control-plane RPC failures
- Use a failure threshold (e.g., 3 consecutive failed polls) before entering partitioned mode
- Store partition mode in `AgentCache` so it survives agent restart
- The reconcile loop should check partition state before accepting new desired-state generations

### Task 16.2: Enforce Node State Machine in Reconcile Loop
**Priority:** P1  
**Assignee:** Backend (Agent)  
**Estimate:** 2 days

**Acceptance Criteria:**
- [ ] Reconcile loop reads current node state from cache before scheduling VM operations
- [ ] If node state is `Degraded`, `Draining`, `Maintenance`, or `Failed`, no new VM creation is attempted
- [ ] If node state is `Draining`, existing VMs are stopped (not deleted) per drain policy
- [ ] Unit tests verify state-gated behavior for each non-TenantReady state

### Task 16.3: Complete Reconcile Test Mocks
**Priority:** P1  
**Assignee:** Backend (Agent)  
**Estimate:** 2 days

**Acceptance Criteria:**
- [ ] `MockStordOk` implements all 7 stubbed RPCs with realistic responses:
  - `get_volume_health` — returns mock health metrics
  - `resize_volume` — updates internal state, returns ok
  - `prepare_snapshot` — creates mock snapshot metadata
  - `prepare_clone` — creates mock clone metadata
  - `restore_snapshot` — restores mock state
  - `delete_snapshot` — removes mock snapshot
  - `set_device_policy` — records policy, returns ok
- [ ] `MockNwdOk` implements all 7 stubbed RPCs with realistic responses:
  - `get_network_health` — returns mock health metrics
  - `set_firewall_policy` — records rules, returns ok
  - `set_nat_policy` — records rules, returns ok
  - `ensure_dhcp_scope` — records scope, returns ok
  - `ensure_dns_scope` — records scope, returns ok
  - `expose_service` — records exposure, returns ok
  - `withdraw_service_exposure` — removes exposure, returns ok
- [ ] All 14 `Status::unimplemented("")` lines are removed from `reconcile.rs`
- [ ] Tests exercise each mock RPC at least once

---

## Sprint 17: Feature Completion

**Goal:** Close the VM resize and network mutation gaps; finish DHCP/DNS enforcement.

### Task 17.1: VM Resize End-to-End
**Priority:** P1  
**Assignee:** Backend (Control Plane + Agent)  
**Estimate:** 2 days

**Acceptance Criteria:**
- [ ] `resize_vm` in control plane extracts `desired_vcpus` and `desired_memory_bytes` from request
- [ ] Control plane writes a `vm_desired_state` fragment with updated `vcpu` and `memory_mb`
- [ ] Agent reconcile loop detects vcpu/memory changes on existing VMs and calls `resize_vm` RPC
- [ ] `VmRuntime::resize_vm` calls Cloud Hypervisor API to resize (or stop-start if hot-resize unsupported)
- [ ] BFF `/v1/vms/:id/resize` endpoint accepts and validates resize params
- [ ] Integration test: create VM → resize → verify new specs

### Task 17.2: Network Mutation Agent-Side Completion
**Priority:** P1  
**Assignee:** Backend (Agent)  
**Estimate:** 2 days

**Acceptance Criteria:**
- [ ] `reconcile_networks` in agent reads `firewall_rules_json`, `nat_rules_json`, `dhcp_scope_json`, `dns_enabled`, `dns_scope_json` from desired state
- [ ] Calls `set_firewall_policy`, `set_nat_policy`, `ensure_dhcp_scope`, `ensure_dns_scope` on `chv-nwd` when rules change
- [ ] Handles network deletion by calling `delete_network_topology`
- [ ] Integration test: create network with firewall rules → verify rules applied in `chv-nwd`

### Task 17.3: DHCP/DNS Enforcement in Reconcile Loop
**Priority:** P2  
**Assignee:** Backend (Agent)  
**Estimate:** 1 day

**Acceptance Criteria:**
- [ ] Reconcile loop detects `dhcp_scope_json` changes and calls `ensure_dhcp_scope`
- [ ] Reconcile loop detects `dns_enabled` / `dns_scope_json` changes and calls `ensure_dns_scope`
- [ ] Verify `dnsmasq` process starts/stops correctly via executor
- [ ] Log message "accepted but not enforced" is removed

### Task 17.4: chv-stord Security Hardening
**Priority:** P2  
**Assignee:** Backend (Stord) + Infra  
**Estimate:** 1 day

**Acceptance Criteria:**
- [ ] Create `chv-stord` system user (separate from `chv`)
- [ ] Update `chv-stord.service` to run as `chv-stord` user
- [ ] Add device/path allowlist config to `stord.toml`
- [ ] `chv-stord` rejects open requests for paths outside allowlist
- [ ] Restrict Unix socket permissions to `chv:chv-stord` group

---

## Sprint 18: UI Production Readiness

**Goal:** Eliminate svelte-check warnings, split oversized components, fix a11y, add e2e to CI.

### Task 18.1: Fix svelte-check Warnings
**Priority:** P2  
**Assignee:** Frontend  
**Estimate:** 2 days

**Acceptance Criteria:**
- [ ] `npm run check` reports **0 errors and 0 warnings**
- [ ] Remove unused CSS selectors (`.action-btn.start:hover`, `.action-btn.danger:hover`, etc.)
- [ ] Fix a11y label associations in form components
- [ ] Fix any remaining `a11y_click_events_have_key_events` suppressions

### Task 18.2: Split Oversized Components
**Priority:** P2  
**Assignee:** Frontend  
**Estimate:** 3 days

**Acceptance Criteria:**
- [ ] `TopologyCanvas.svelte` (986 → <300) — extract `MiniMap`, `ContextMenu`, `ResourceRenderer`
- [ ] `SidebarNav.svelte` (688 → <300) — extract `SidebarTree`, `InstanceContextMenuWrapper`, `UserFooter`
- [ ] `settings/users/+page.svelte` (815 → <300) — extract `UserTable`, `UserFormModal`
- [ ] `vms/[id]/+page.svelte` (791 → <300) — extract `VmConsoleTab`, `VmMetricsTab`, `VmSettingsTab`
- [ ] `CreateVMModal.svelte` (580 → <300) — extract `CloudInitBuilder`, `HypervisorOverrides`
- [ ] All remaining components >400 lines split or documented with exemption reason

### Task 18.3: Fix Modal A11y
**Priority:** P2  
**Assignee:** Frontend  
**Estimate:** 1 day

**Acceptance Criteria:**
- [ ] Replace `<!-- svelte-ignore a11y_* -->` on modal backdrops with proper `<dialog>` elements or `onkeydown` handlers
- [ ] All modals trap focus and close on `Escape`
- [ ] All modals restore focus on close

### Task 18.4: Type InventoryListPage
**Priority:** P2  
**Assignee:** Frontend  
**Estimate:** 1 day

**Acceptance Criteria:**
- [ ] Define `TableColumn<T>` and `TableRow` generic interfaces
- [ ] Replace `any[]` and `any` in `InventoryListPage.svelte` with generics
- [ ] Update all call sites to pass proper type parameters

### Task 18.5: Add Playwright E2E to CI
**Priority:** P1  
**Assignee:** Infra  
**Estimate:** 1 day

**Acceptance Criteria:**
- [ ] New GitHub Actions job `e2e` in `.github/workflows/ci.yml`
- [ ] Job installs Playwright dependencies (`npx playwright install --with-deps`)
- [ ] Job builds Rust workspace in release mode
- [ ] Job starts all 4 daemons + nginx in CI (or uses mock server approach)
- [ ] Job runs `cd ui && npx playwright test`
- [ ] Job uploads Playwright report on failure

---

## Sprint 19: Infrastructure & Polish

**Goal:** Multi-node readiness, Docker compose, backup execution, design system alignment.

### Task 19.1: Multi-Node WebSocket Routing
**Priority:** P1  
**Assignee:** Infra + Backend (BFF)  
**Estimate:** 2 days

**Acceptance Criteria:**
- [ ] BFF generates console WebSocket URLs with `node_id` parameter
- [ ] nginx config supports dynamic upstream selection (lua module or multiple location blocks)
- [ ] For MVP-1 fallback: BFF proxies console WebSocket to correct agent via gRPC streaming
- [ ] Document multi-node nginx setup in `docs/DEPLOYMENT.md`

### Task 19.2: Backup Execution Engine
**Priority:** P2  
**Assignee:** Backend (Control Plane)  
**Estimate:** 3 days

**Acceptance Criteria:**
- [ ] Background worker (tokio task or cron-like loop) polls `backup_jobs` table for `queued` jobs
- [ ] Worker calls agent `prepare_snapshot` → agent calls `chv-stord` snapshot → worker copies snapshot to target storage
- [ ] Worker updates job status: `queued` → `running` → `succeeded`/`failed`
- [ ] Support target backends: local filesystem, S3-compatible (MinIO)
- [ ] REST endpoint to trigger on-demand backup
- [ ] Integration test: create VM → trigger backup → verify snapshot exists

### Task 19.3: Toast Design System Alignment
**Priority:** P2  
**Assignee:** Frontend  
**Estimate:** 0.5 day

**Acceptance Criteria:**
- [ ] Replace hardcoded colors in `Toast.svelte` with CSS custom properties:
  - Success: `var(--color-success)`
  - Error: `var(--color-danger)`
  - Info: `var(--color-info)`
  - Warning: `var(--color-warning)`
- [ ] Verify dark mode compatibility

### Task 19.4: Docker Compose Completeness
**Priority:** P2  
**Assignee:** Infra  
**Estimate:** 1 day

**Acceptance Criteria:**
- [ ] `docker-compose.yml` defines services: controlplane, agent, stord, nwd, nginx
- [ ] Shared volumes for `/etc/chv/certs`, `/var/lib/chv`
- [ ] Network bridge setup automated (privileged mode or pre-created bridge)
- [ ] Bootstrap token generation on first start
- [ ] Health checks for all services
- [ ] Document `docker compose up` workflow in `docs/DEPLOYMENT.md`

---

## Rollup: Success Criteria

After completing this plan:

1. **Agent partition policy** is implemented and tested (closes P0)
2. **All P1 gaps closed:** VM resize E2E, network mutations, Playwright CI, multi-node WS routing, state machine enforcement
3. **svelte-check reports 0 warnings**
4. **No component exceeds 400 lines** without documented exemption
5. **All 14 reconcile test mocks implemented**
6. **Backup execution engine** handles snapshot → ship → status update
7. **Toast colors** use design system tokens
8. **Docker compose** runs full stack with one command

---

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Partition policy is complex (distributed systems edge cases) | Medium | High | Start with simple timeout-based detection; iterate |
| Playwright e2e in CI is flaky (daemon startup timing) | High | Medium | Use health-poll loops; allow retry-on-failure |
| VM resize requires CHV hot-plug support | Medium | Medium | Fallback to stop-resize-start if hot-plug fails |
| Component splitting introduces regressions | Medium | Medium | Split one component at a time; run full check after each |
| Backup engine scope creep (S3, encryption, compression) | Medium | Low | Scope to local + S3 for MVP-1; defer encryption to P3 |
