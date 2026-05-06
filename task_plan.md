# Task Plan: Orchestrator Retry + Critical Gap Fixes

## Goal
Make the end-to-end VM lifecycle reliable by adding operation retry, image validation, node liveness detection, and eliminating WAL contention in install.

## Phases
- [x] Phase 1: C1+C2+M6 — Add retry with backoff to orchestrator
- [x] Phase 2: C3 — Validate image path at VM creation time (fail fast)
- [x] Phase 3: H1 — Node liveness detection (mark unreachable if no heartbeat)
- [x] Phase 4: H5 — Replace sqlite3 CLI token seeding with HTTP API endpoint
- [x] Phase 5: M1 — Operation reaper for stuck Running operations
- [x] Phase 6: Verify — cargo build + cargo test + cargo clippy (233 tests pass)

## Key Decisions
- Model retry after backup_worker: exponential backoff (60/120/240s), max 3 retries
- New migration adds retry_count + next_retry_at to operations table
- Node liveness: mark Unreachable if no state report in 60s
- Token seeding: add /internal/bootstrap-token endpoint (localhost only)
- Operation reaper: Running ops older than 60s transition back to Accepted

## Errors Encountered
- (none yet)

## Status
**Currently in Phase 1** - Dispatching parallel subagents for implementation
