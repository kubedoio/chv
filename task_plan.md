# Task Plan: Production Readiness Fixes

## Goal
Fix all CRITICAL and HIGH production bugs from the gap analysis, progressing through P0 (data loss) → P1 (security/reliability) → P2 (spec compliance), then implement ship-blocking feature gaps.

## Phases
- [x] Phase 1: P0 Critical Fixes — Data Loss (6 bugs, DONE)
- [x] Phase 2: P1 Security/Reliability (8 items, DONE)
- [x] Phase 3: P2 Spec Compliance (6 items, DONE)
- [x] Phase 4: Final verification
- [x] Phase 5: Ship-blocking feature gaps (3 items — all already implemented)

## P0 Items (COMPLETED — commit a3700393)

| # | Bug | Status |
|---|-----|--------|
| P0-1 | Live migration skips dirty-block sync | FIXED |
| P0-2 | stord session persistence disabled | FIXED |
| P0-3 | stop_vm reports success without confirming | FIXED |
| P0-4 | Volume resource leak on partial prep failure | FIXED |
| P0-5 | VNI allocation race condition | FIXED |
| P0-6 | Silent JSON serialization data loss | FIXED |

## P1 Items (COMPLETED — commit 176eec71)

| # | Issue | Location | Status |
|---|-------|----------|--------|
| 7 | No gRPC timeouts to stord/nwd | agent-core/daemon_clients.rs | FIXED |
| 8 | TLS domain hardcoded "localhost" | agent-core/control_plane.rs | FIXED |
| 9 | Blocking I/O on async executor | stord-backends/local.rs | FIXED |
| 10 | TOCTOU in migration port allocation | agent-core/migration.rs | FIXED |
| 11 | Non-atomic quota check | controlplane-service/lifecycle.rs | N/A (stub only) |
| 12 | Hot-plug ops not persisted | controlplane-service/lifecycle.rs | N/A (already persisted) |
| 13 | Ready-node metric always 0 | controlplane-service/orchestrator.rs | FIXED |
| 14 | Migration mTLS missing | stord-core/migration/sender.rs | FIXED |

## P2 Items (COMPLETED — commit 2e9e7114)

| # | Issue | Location | Status |
|---|-------|----------|--------|
| 15 | SQLite startup integrity check | controlplane-store/db.rs | FIXED |
| 16 | MTU auto-detection + bridge MTU | nwd-core/executor.rs | FIXED |
| 17 | OperationStatus::Cancelled variant | controlplane-types/domain.rs | FIXED |
| 18 | Agent-side StartVm denial when not TenantReady | agent-core/agent_server.rs | FIXED |
| 19 | Telemetry health not hardcoded | agent-core/telemetry.rs | FIXED |
| 20 | Cursor-based pagination in BFF | webui-bff/handlers/events.rs | FIXED |

## Phase 5: Ship-blocking Features (COMPLETED — already implemented in prior work)

From gap analysis P0 items (must-have for first deployment):

| # | Feature | Scope | Status |
|---|---------|-------|--------|
| F1 | User Management (CRUD) | Backend handlers + UI page | DONE (handlers/users.rs, /settings/users UI) |
| F2 | Image Delete | Backend handler + UI button | DONE (handlers/images.rs::delete_image, /v1/images/delete) |
| F3 | Cloud-init Support | DB + Backend + Wire existing UI | DONE (migration 0012, templates CRUD, VM create wiring, UI components) |

## Key Decisions
- Work on branch: `fix/p0-critical-production-bugs` (continuing)
- All fixes must compile with `cargo check --workspace`
- All existing tests must pass
- No new features — only fix the bugs identified (Phases 1-4)
- Phase 5: Ship-blocking features from gap analysis

## Errors Encountered
- Pre-existing test failures (missing timeout_multiplier field) — fixed alongside P0

## Status
**All phases complete.** P0-P2 bug fixes committed. Phase 5 ship-blocking features verified as already implemented from prior work. The production readiness task plan is done.
