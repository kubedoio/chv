# CHV Phased Implementation Plan

Based on the comprehensive repository review and existing roadmap, the following is a detailed, phased implementation plan to bring the CHV platform to production readiness.

## Current Status Summary

As of Sprint 15 (PRs 49-52), all P0 (CRITICAL) and P1 (HIGH) items are complete. Most P2 (MEDIUM) items are done. The platform has transitioned from early-MVP to stability phase.

**Delivered in recent sprints:**
- Rolling upgrade orchestration (`UpgradeOrchestrator` + `SystemdNodeUpgrader`)
- Compatibility matrix boot gate (version validation before upgrade)
- mTLS enforcement on storage migration (plaintext rejected)
- Backpressure handling in migration sender
- Drain evacuation (automatic VM migration on node drain)
- Partition reconnect flush (pending messages flushed on reconnect)
- eBPF auto-load on NIC attach (policy + ingress programs)
- FDB cleanup on VM detach
- VXLAN teardown on topology delete
- MigrationReaper (auto-fails stuck migrations after 2h)
- Circuit breaker on node communication
- Deep health checks (database, agent socket, agent connectivity)

---

## Phase 1: Stability, Bug Fixes & Agent Parity [COMPLETE]

**Goal**: Ensure the foundation is solid, fix failing tests, and replace mock agent implementations with real daemon calls.

### 1.1 UI & Frontend Fixes [COMPLETE]
*   ~~Fix `Ctrl+K` Command Palette Test~~
*   ~~Clean Up Unused CSS~~
*   ~~Fix A11y Warnings~~

### 1.2 Agent Daemon Integration (Testing Mocks & Coverage) [COMPLETE]
*   ~~`chv-stord` and `chv-nwd` Mock Completion~~
*   ~~Implement missing mock logic for storage RPCs~~
*   ~~Implement missing mock logic for network RPCs~~

---

## Phase 2: Feature Completion (Orchestration, Quotas, & Networking) [COMPLETE]

**Goal**: Fulfill the remaining core features outlined in the gap analysis.

### 2.1 Backend / BFF Completion [COMPLETE]
*   ~~Hypervisor Orchestrator Merge (`B5`)~~
*   ~~Quota Enforcement (`B3`)~~
*   ~~Agent API Token Auth (`B10`)~~
*   ~~Metadata Propagation (`A6`)~~

### 2.2 Storage & Network Daemons [COMPLETE]
*   ~~`chv-nwd` Linux Executor (`A8`)~~
*   ~~Storage Pool Provisioning (`B7`)~~
*   ~~Image Import Validation (`B8`)~~

---

## Phase 3: Production Readiness & Observability [IN PROGRESS]

**Goal**: Prepare the system for long-term maintainability, backup strategies, and scale.

### 3.1 UI Production Refactor (`U1`, `U6`, `U7`) [NOT STARTED]
*   **Tailwind-first Migration**: Strip `app.css` and fully migrate primitives/shell components to Tailwind.
*   **Component Reorganization**: Reorganize `ui/src/lib/components` into strictly feature-based folders.
*   **DataTable Refactoring**: Split the large `DataTable` (688 lines) into smaller modules handling selection, sorting, and visibility.
*   **Overview Logic Extraction**: Extract logic from the overview page (526 lines) into dedicated helpers.

### 3.2 Backups & Role-Based Access [PARTIAL]
*   **Backup/DR (`B2`)**: Remove the backup job and history stubs in `api/stub.rs`. Design the backup schema and implement the backend orchestration for taking VM snapshots and shipping them.
*   ~~RBAC Middleware (`B6`)~~: Role-based access on BFF routes implemented.

### 3.3 CI/CD & Deployments (`I3`, `I4`, `I5`, `I6`) [COMPLETE]
*   ~~GitHub Actions~~: CI set up.
*   ~~DB Migration Backups~~: Pre-migration SQLite backup implemented.
*   ~~Nginx Routing~~: WebSocket routing configured.
*   ~~Versioning~~: Release versioning automated via `VERSION` file + Makefile.

### 3.4 Production Gaps (Sprints 11-15) [COMPLETE]
*   ~~Rolling upgrade orchestration~~ (UpgradeOrchestrator, SystemdNodeUpgrader)
*   ~~Compatibility matrix enforcement~~ (boot gate rejects incompatible versions)
*   ~~mTLS on storage migration~~ (plaintext connections rejected)
*   ~~Backpressure handling~~ (sender throttles based on receiver feedback)
*   ~~eBPF auto-load on NIC attach~~ (policy + ingress programs)
*   ~~FDB cleanup on VM detach~~ (forwarding database entries cleaned)
*   ~~Drain evacuation~~ (Draining state triggers VM migration requests)
*   ~~Partition reconnect flush~~ (agent flushes pending messages on reconnect)
*   ~~VXLAN teardown~~ (delete_topology cleans up VXLAN interfaces)
*   ~~MigrationReaper~~ (background task times out stalled migrations)
*   ~~Circuit breaker~~ (protects against cascading failures in node communication)
*   ~~Deep health checks~~ (component-level: database, agent socket, agent connectivity)

---

## Success Criteria
1.  ~~All Playwright E2E tests pass reliably.~~
2.  `svelte-check` reports 0 errors and 0 warnings.
3.  ~~All `unimplemented!()` stubs in the agent `reconcile.rs` are replaced with real daemon clients.~~
4.  ~~No `BffError::NotImplemented` returned from any frontend API call.~~
5.  ~~CI pipeline is green and automated backups are working.~~
6.  All production-gap items (P0, P1) resolved — **DONE**

## Remaining Work

| Area | Item | Priority |
|------|------|----------|
| UI | Tailwind-first migration | P2 |
| UI | Command palette | P2 |
| Backend | Backup/DR job implementation | P2 |
| UI | DataTable / Overview refactor | P3 |
