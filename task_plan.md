# Task Plan: Fix Robustness & Production Readiness Issues (Phases C + D)

## Goal
Address all HIGH-severity robustness findings (Phase C) and production readiness gaps (Phase D) identified in the comprehensive review.

## Phases
- [ ] Phase 1: Create branch, verify baseline compiles
- [ ] Phase 2: Phase C fixes — Robustness (7 items)
- [ ] Phase 3: Phase D fixes — Production Readiness (6 items)
- [ ] Phase 4: Verify — cargo check, clippy, test, fmt

## Phase C Items (Robustness)
- [ ] C1: Ownership checks in snapshot handlers — verify VM belongs to caller
- [ ] C2: BearerToken extractor in all handlers — defense-in-depth auth
- [ ] C3: Bridge name validation (IFNAMSIZ=15) in nwd
- [ ] C4: Quiescence before snapshot — pause VM or fsfreeze advisory
- [ ] C5: Cache TTL/size limits in BFF
- [ ] C6: Auth guard to +layout.ts load function (server-side redirect)
- [ ] C7: Remove/implement stub handlers (handleImportVM)

## Phase D Items (Production Readiness)
- [ ] D1: Wire observability metrics (ADR-009) to hot paths
- [ ] D2: Health endpoints for nwd/stord
- [ ] D3: Rate limiting on auth endpoints
- [ ] D4: CSRF protection
- [ ] D5: Content-Security-Policy headers
- [ ] D6: Audit logging for mutations

## Key Questions
1. Which handlers currently lack BearerToken extractors?
2. What's the BFF cache implementation (in-memory HashMap? TTL crate?)
3. What metrics are defined but unwired per ADR-009?

## Decisions Made
- Branch from main as `fix/robustness-production-readiness`
- Fix in dependency order: C items first (some D items build on C)

## Errors Encountered
- (none yet)

## Status
**Currently in Phase 1** - Creating branch and researching current state
