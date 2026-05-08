# Task Plan: Production Readiness Fixes

## Goal
Fix all CRITICAL and HIGH production bugs from the gap analysis, progressing through P0 (data loss) → P1 (security/reliability) → P2 (spec compliance).

## Phases
- [x] Phase 1: P0 Critical Fixes — Data Loss (6 bugs, DONE)
- [x] Phase 2: P1 Security/Reliability (8 items, DONE)
- [x] Phase 3: P2 Spec Compliance (6 items, DONE)
- [x] Phase 4: Final verification

## P0 Items (COMPLETED — commit a3700393)

| # | Bug | Status |
|---|-----|--------|
| P0-1 | Live migration skips dirty-block sync | FIXED |
| P0-2 | stord session persistence disabled | FIXED |
| P0-3 | stop_vm reports success without confirming | FIXED |
| P0-4 | Volume resource leak on partial prep failure | FIXED |
| P0-5 | VNI allocation race condition | FIXED |
| P0-6 | Silent JSON serialization data loss | FIXED |

## P1 Items (NEXT)

| # | Issue | Location | Effort |
|---|-------|----------|--------|
| 7 | No gRPC timeouts to stord/nwd | agent-core/daemon_clients.rs | 2h |
| 8 | TLS domain hardcoded "localhost" | agent-core/control_plane.rs, enrollment.rs | 1h |
| 9 | Blocking I/O on async executor | stord-backends/local.rs | 2h |
| 10 | TOCTOU in migration port allocation | agent-core/migration.rs | 1h |
| 11 | Non-atomic quota check | controlplane-service/lifecycle.rs | 1h |
| 12 | Hot-plug ops not persisted | controlplane-service/lifecycle.rs | 3h |
| 13 | Ready-node metric always 0 | controlplane-service/orchestrator.rs | 30min |
| 14 | Migration mTLS missing | stord-core/migration/sender.rs | 2h |

## Key Decisions
- Work on branch: `fix/p0-critical-production-bugs` (continuing)
- All fixes must compile with `cargo check --workspace`
- All existing tests must pass
- No new features — only fix the bugs identified

## Errors Encountered
- Pre-existing test failures (missing timeout_multiplier field) — fixed alongside P0

## Status
**All phases complete.** P0, P1, and P2 fixes implemented and verified.

## P2 Items (COMPLETED)

| # | Issue | Location | Status |
|---|-------|----------|--------|
| 15 | SQLite startup integrity check | controlplane-store/db.rs | FIXED |
| 16 | MTU auto-detection + bridge MTU | nwd-core/executor.rs | FIXED |
| 17 | OperationStatus::Cancelled variant | controlplane-types/domain.rs | FIXED |
| 18 | Agent-side StartVm denial when not TenantReady | agent-core/agent_server.rs | FIXED |
| 19 | Telemetry health not hardcoded | agent-core/telemetry.rs | FIXED |
| 20 | Cursor-based pagination in BFF | webui-bff/handlers/events.rs | FIXED |
