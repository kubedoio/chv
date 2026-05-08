# Task Plan: P0 Critical Fixes — Data Loss and Correctness

## Goal
Fix all 6 CRITICAL production bugs identified in the product readiness gap analysis, bringing the system from "will lose data" to "safe for production use."

## Phases
- [x] Phase 1: Understand/research (gap analysis complete)
- [x] Phase 2: Plan approach (this document)
- [ ] Phase 3: Implement (6 P0 fixes via parallel subagents)
- [ ] Phase 4: Verify (cargo check, clippy, tests)

## P0 Items

| # | Bug | File | Fix Strategy |
|---|-----|------|-------------|
| P0-1 | Live migration skips dirty-block sync | stord-core/migration/sender.rs | Add dirty round loop between bulk_copy and FinalSync |
| P0-2 | stord session persistence disabled | cmd/chv-stord/main.rs | Pass db_path to StorageServer, wire SessionStore |
| P0-3 | stop_vm reports success without confirming | agent-runtime-ch/process.rs | Check process exit after timeout; force-kill if still running |
| P0-4 | Volume resource leak on partial prep failure | agent-core/reconcile.rs | Add cleanup of opened volumes if NIC attach fails |
| P0-5 | VNI allocation race condition | controlplane-store/vtep.rs | Wrap read+insert in BEGIN IMMEDIATE transaction |
| P0-6 | Silent JSON serialization data loss | controlplane-service/reconcile.rs | Propagate error instead of unwrap_or_default |

## Subagent Strategy

- **Agent A**: P0-1 (dirty sync rounds) — largest change, ~150 lines
- **Agent B**: P0-2 + P0-3 (stord persistence + stop_vm confirmation) — related daemon fixes
- **Agent C**: P0-4 + P0-5 + P0-6 (volume leak + VNI race + JSON loss) — smaller fixes

## Key Decisions
- Work on a feature branch: `fix/p0-critical-production-bugs`
- All fixes must compile with `cargo check --workspace`
- All existing tests must pass
- No new features — only fix the bugs identified

## Errors Encountered
- (none yet)

## Status
**Currently in Phase 3** — Dispatching subagents
