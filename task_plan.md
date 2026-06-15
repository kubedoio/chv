# Task Plan: Architecture Designer — Phase 4 (Plan generation)

## Goal
Ship Phase 4 of the Architecture Designer per `docs/plans/2026-06-13-architecture-designer-implementation-plan.md` §3 Phase 4 (lines 174–199). Industrial grade: every endpoint integration-tested, property tests for diff/order, fake-clock TTL, Playwright UI spec, no panics, structured tracing.

## Branch
`feat/designer-phase-4-plan-generation` (forked from `main` at `05703888` — Phase 3 merged)

## Scope (locked by plan)
1. `chv-architecture-reconcile::plan` module
   - `Diff::compute(desired: &CHVArchitecture, snapshot: &validate::fleet::InventorySnapshot) -> Vec<PlanChange>` (pure)
   - `order_changes(changes) -> Vec<PlanChange>` deterministic ordering: roles → users → datastores → networks → images → templates → instances → disks → network-attachments → cloud-init → backup-policies → drift-baseline.
   - PlanMode mapping: extend type-level `PlanMode` enum to add `Apply` and `Destroy` (keep existing `DryRun`/`Confirm` for store/test compatibility — coexist as a union).
2. `chv-common::Clock` trait + `SystemClock` + `ManualClock(Arc<Mutex<DateTime<Utc>>>)` for deterministic TTL.
3. Plan persistence with `expires_at = clock.now() + 15min`. Stale-plan rejection in BFF apply path (Phase-5 will consume; Phase-4 introduces the check via `is_expired(plan, clock)` helper).
4. BFF endpoints (mounted under existing role gates):
   - `POST /v1/architectures/plan` — apply-mode plan
   - `POST /v1/architectures/destroy-plan` — destroy-mode plan
   - `POST /v1/architectures/discard-plan` — explicit discard
5. UI:
   - `plan-store.svelte.ts` reactive store using `mutateWithRefresh`
   - `PlanReview.svelte` component: summary, per-change list (color by risk), TTL countdown badge, typed-name confirmation field
   - Plan tab on `/architectures/[id]/+page.svelte`
6. Acceptance gates (`docs/plans/...` Phase 4):
   - `cargo test -p chv-architecture-reconcile plan::` — green
   - Property test: applying a plan twice produces empty second plan (Diff idempotency)
   - Property test: `order_changes` is total + deterministic
   - BFF integration test: missing-image → plan returns `failed_validation` with finding inlined
   - BFF integration test: stale plan (manual-clock advanced > 15min) cannot be applied — 409 + `PLAN_EXPIRED`
   - Playwright `architectures-plan.spec.ts`: generate-plan-happy, plan-expired, plan-with-blocking-finding

## Inventory reality (Phase 4)
- `architecture_plans` table: ✅ exists (migration 0049)
- `PlanRepository`: ✅ exists with create/get/list/update_status — Phase-4 builds on these
- `ArchitecturePlan`/`PlanChange`/`PlanStatus`/`PlanMode`/`PlanAction`/`ResourceType`/`Risk`: ✅ exist
- `chv-architecture-reconcile`: ✅ exists with `FleetInventoryProvider` (Phase 3) — add `plan` module
- `chv-common`: ✅ exists (lib + hypervisor) — add `Clock` module
- BFF handler `architectures.rs`: ✅ exists with check-fleet, validate, etc. — add 3 new handlers
- UI `architecture-store.svelte.ts`: ✅ exists with `mutateWithRefresh` pattern — add `plan` / `destroyPlan` / `discardPlan`

## Phases
- [x] Phase 1: Audit existing surfaces (done)
- [ ] Phase 2: Stage 1 parallel implementation
  - Subagent A (general-purpose): `chv-common::Clock`, `chv-architecture-reconcile::plan` module (Diff::compute, order_changes), property tests
  - Subagent B (general-purpose): BFF endpoints + integration tests, AppState wiring, fake-clock harness
  - Subagent C (typescript-frontend): plan-store, PlanReview component, plan tab, Playwright spec
- [ ] Phase 3: Reviewer pass (security, language-specialist, test-analyzer, api-contract)
- [ ] Phase 4: Apply reviewer findings, run quality matrix, push, open PR

## Decisions Made
- **PlanMode extension**: add `Apply`/`Destroy` variants (keeping existing `DryRun`/`Confirm` for store-layer compatibility — they coexist; the BFF accepts only `apply`/`destroy` on the new endpoints).
- **Snapshot input to Diff**: use `chv_architecture_validate::fleet::InventorySnapshot` directly (the validate-layer type with completeness flags). Store-side `chv_controlplane_types::architecture::InventorySnapshot` is the persisted-row type; the BFF deserializes `snapshot_json` back into the validate type.
- **Plan storage**: persist `plan_json` as the JSON serialization of `Plan { changes: Vec<PlanChange>, summary, warnings, mode }`; `summary_json` separately for cheap list views.
- **TTL**: `Clock` injected into BFF AppState; `SystemClock` in production, `ManualClock` in tests. Stale check = `clock.now() > plan.expires_at`.
- **Discard semantics**: `update_status(plan_id, Discarded, mark_discarded=true)`; idempotent (already-discarded = 200 noop).
- **Plan ordering**: encoded as a `ResourceOrder` enum with `priority() -> u8` and stable `Ord` impl on `(priority, action_priority, resource_name)`.

## Acceptance gates (must all be green before PR opens)

```bash
# Rust
rtk cargo build --workspace
rtk cargo test --workspace
rtk cargo test -p chv-architecture-reconcile plan
rtk cargo clippy --workspace --all-targets -- -D warnings
rtk cargo fmt --all -- --check

# UI
cd ui && rtk npm run check
cd ui && rtk npx vitest run
cd ui && rtk npx playwright test architectures-plan.spec.ts
```

## Errors Encountered
(populate during execution)

## Status
**Phase 2 — dispatching parallel subagents A/B/C for Stage 1 implementation.**
