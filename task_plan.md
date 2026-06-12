# Task Plan: Fix Comprehensive PR Review Findings (2026-06-12)

## Goal
Resolve all CRITICAL and HIGH findings from the comprehensive review of HEAD~5..HEAD on `fix/pr-review-findings-2026-06-12`, with verified industrial-grade quality (build green, type-check clean, tests pass, no regressions).

## Branch
`fix/pr-review-findings-2026-06-12` (forked from `main` at 73a9d174)

## Phases
- [x] Phase 1: Understand/research (review already complete in prior turn)
- [x] Phase 2: Plan + branch
- [x] Phase 3: Implement fixes via parallel subagents
- [x] Phase 4: Verify (npm run check, npm test, cargo build) and deliver

## Findings to Fix

### Group A — Build/Test Infrastructure (BLOCKER — must run first)
- **CQ-1** `glob` not installed → `cd ui && npm install`
- **CQ-2** `import.meta.dirname` undefined in jsdom → `fileURLToPath(new URL('../../../', import.meta.url))` AND add `// @vitest-environment node`
- **CQ-3** 13 implicit `any` lambdas → cast `globSync(...)` as `string[]`

### Group B — Silent failures + reactive store bugs (`task-stream.svelte.ts`, `live-state.svelte.ts`)
- **SF-1** Separate `onTaskCompleted` from JSON parse try/catch
- **SF-2** Empty polling catch → log + 401 handling consistent with SSE
- **BUG-1** Implement `detailId` consumption (invalidate detail key `${pattern}${detailId}`)
- **BUG-2** Reset `reconnectDelay` in `disconnect()`
- **BUG-3** `.catch()` on unawaited `invalidateAndRefresh` in `handleTaskCompleted`
- **BUG-4** `.catch()` on unawaited calls inside `setTimeout` deferred path
- **SEC-1** Replace local `getStoredToken()` with import from `$lib/api/client`
- **SEC-2** Bound `seen` Set: TTL Map with pruning + clear on disconnect
- **SEC-3** Polling 401 → mirror SSE behavior (set status='error', stop polling)
- **CQ-4** Remove dead `DETAIL_TTL` constant from `live-state.svelte.ts`
- **CQ-6** Add `// eslint-disable-next-line no-console` annotations consistent with old code
- **DOC-1** Fix `getCacheEntry` deprecation message → recommend `liveState.cachedFetch()`

### Group C — New tests (TypeScript/Vitest)
- **T-1** Tests for `mutateWithRefresh` (success path, error rethrow, skipRefresh, options forwarding)
- **T-2** Tests for `liveState.invalidateAndRefresh` (patterns, sidebar, detailId, delayMs, SSR no-op)

### Group D — Rust packaging
- **CQ-5** Remove redundant `libsqlite3-sys` from 3 library crates (keep workspace decl + binary crate `cmd/chv-controlplane`)

### Group E — Docs
- **DOC-2** ADR-004: stop referencing `taskStream.seen` as if public — describe behavior abstractly

## Decisions Made
- **Branch**: `fix/pr-review-findings-2026-06-12`
- **Sequencing**: Group A first (foundation); then B, C, D, E in parallel via subagents
- **detailId**: Implement targeted invalidation by also clearing cache key `${pattern}${detailId}` (the cache uses `:` separator like `vms:abc-123`)
- **No commit to main** — work isolated to feature branch

## Key Questions
1. After Group A fix, does `npm run check` exit 0? → Verify before launching B/C
2. Does the existing `live-state.test.ts` still pass after Group B mutations?
3. Does `cargo build --workspace` succeed after Group D?

## Errors Encountered
(to be logged)

## Status
**Phase 4 complete — all findings closed.**

### Commits on `fix/pr-review-findings-2026-06-12` (5 bisectable)
| SHA | Group | Closes |
|-----|-------|--------|
| `8c60d340` | A | CQ-1, CQ-2, CQ-3 |
| `24917ddc` | E | DOC-2 |
| `e1c7716d` | D | CQ-5 |
| `16f46b81` | B | SF-1, SF-2, BUG-1..4, SEC-1, SEC-2, SEC-3, CQ-4, CQ-6, DOC-1 |
| `72ea9134` | C | T-1, T-2 |

### Final verification
- `cd ui && npm run check`: **4424 files, 0 errors, 0 warnings**
- `cd ui && npx vitest run`: **162 tests passing across 32 files** (was 153 before — 9 new in mutation.test.ts, 7 new in live-state.test.ts; less the 7 mutation-compliance tests that are no longer vacuous)
- `cargo build --workspace`: clean
- `cargo test -p chv-controlplane-store -p chv-webui-bff -p chv-controlplane-service --lib`: 47+ tests pass, 0 fail
- compliance suite now actively scans **26** `+page.svelte` files (was 0 — vacuously passing)

### Findings closed: **18 of 18** from this review pass
Critical: CQ-1, CQ-2, CQ-3, SF-1, SF-2, T-1, T-2 (7)
High: BUG-1, BUG-2, BUG-3, BUG-4, SEC-1, SEC-2, DOC-1 (7)
Medium: SEC-3, CQ-4, CQ-5, CQ-6, DOC-2 (5)

Branch ready for PR. No commit to `main`.
