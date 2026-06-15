# Task Plan: Architecture Designer — Phase 6 (Drift detection, read-only)

## Goal
Ship Phase 6 of the Architecture Designer per `docs/plans/2026-06-13-architecture-designer-implementation-plan.md` §3 Phase 6 (lines 230–248). Industrial grade: pure-data Diff between baseline and live snapshot, ≥1 fixture per finding type, BFF endpoint with cached + force-refresh paths, Drift tab + dashboard badge, Playwright spec.

## Branch
`feat/designer-phase-6-drift-detection` from `main` @ `28dc05db`.

## Inventory reality
- `architecture_drift_reports` table: ✅ shipped 0050 (Phase 0)
- `ArchitectureDriftReport` + `DriftStatus { Unknown, NoDrift, Drifted, CheckFailed }`: ✅ exist (Phase 0)
- `DriftReportRepository` (create/get/list_for_architecture): ✅ exists (Phase 0)
- `get_architecture_drift` BFF handler: ❌ **STUB** returning `BffError::NotImplemented("phase 0")`
- `list_architecture_runs` BFF handler: ❌ **STILL STUB** (Phase 5's B1 was not actually wired despite reviewer fix-up; folding into this PR)
- `chv_architecture_validate::fleet::InventorySnapshot` + `FleetInventoryProvider` (live snapshot capture): ✅ exist
- Plan baseline: Phase 5's `apply_plan` writes `architecture_apply_runs` rows but does **not** snapshot a baseline on success. **Phase 6 carve-out**: define "baseline" as the `architecture_versions.snapshot_json` row written when an apply succeeds — but since orchestrator-side Run completion is out-of-scope (Phase 5 stops at `Queued → Running`), Phase 6's baseline source is the **architecture model itself** (the YAML / topology canvas state) compared against the live `InventorySnapshot`. This matches ADR-005-Designer's "reconciler is read-only MVP" guidance.
- No `DriftFinding` type yet — Phase 6 introduces it.

## Scope (locked)

### Rust (`chv-architecture-reconcile::drift`)
1. New module `drift.rs`:
   - `DriftFinding` enum with the 7 types from §237: `MissingResource`, `UnexpectedResource`, `FieldChanged`, `CapacityChanged`, `NetworkChanged`, `PermissionChanged`, `AttachmentChanged`. Each carries a `code: &'static str` (e.g. `DRIFT_MISSING_RESOURCE`), a `path: String`, a `resource_ref: Option<String>`, a `message: String`, plus type-specific fields (e.g. `FieldChanged { field, expected, actual }`).
   - `compute_drift(baseline: &CHVArchitecture, snapshot: &InventorySnapshot) -> DriftReport` — pure function, returns `DriftReport { findings: Vec<DriftFinding>, status: DriftStatus, summary: DriftSummary }`. `status = NoDrift if findings.is_empty() else Drifted`.
   - `DriftSummary { total: usize, by_type: BTreeMap<&'static str, usize> }` — for fast list-view rendering.
   - 7 unit-test fixtures, one per finding type, asserting the exact stable code + message shape.
   - 1 idempotency test: same inputs twice → identical findings.
   - 1 no-drift fixture: baseline matches snapshot exactly → empty findings, status NoDrift.

2. Detection rules per finding type (industrial — match ADR-005-Designer reconciler MVP):
   - `MissingResource` (`DRIFT_MISSING_RESOURCE`): a resource declared in the baseline that is absent from the live snapshot.
   - `UnexpectedResource` (`DRIFT_UNEXPECTED_RESOURCE`): a live resource whose name does NOT appear in the baseline (informational; the architecture is the source of truth, but the live fleet has extras).
   - `FieldChanged` (`DRIFT_FIELD_CHANGED`): the resource exists in both but a non-capacity/non-network field differs (e.g. server.cpu_cores baseline=8, live=4 → wait, that's capacity. Use a non-numeric: e.g. datastore.kind baseline="nfs", live="iscsi"). Capacity is its own bucket.
   - `CapacityChanged` (`DRIFT_CAPACITY_CHANGED`): numeric capacity attributes (server.cpu_cores, server.memory_gb, datastore.capacity_gb, datastore.free_gb) differ.
   - `NetworkChanged` (`DRIFT_NETWORK_CHANGED`): network bridge / vlan_id / cidr differ between baseline and live.
   - `PermissionChanged` (`DRIFT_PERMISSION_CHANGED`): the snapshot's `deploy_allowed` no longer matches what the baseline assumed (heuristic: emit if baseline declared roles/permissions but live caller_can_deploy is now false).
   - `AttachmentChanged` (`DRIFT_ATTACHMENT_CHANGED`): instance.networks attachments differ (baseline expects net-A, live shows net-B), or instance placement (server) differs.

### Store (`chv-controlplane-store`)
3. No new migration. `DriftReportCreateInput` already exists. Add a helper `DriftReportRepository::list_latest_for_architecture(arch_id, limit)` if not present (it isn't — current is `list_for_architecture`).

### BFF (`crates/chv-webui-bff/src/handlers/architectures.rs`)
4. Replace the stub `get_architecture_drift` with the real handler:
   - `POST /v1/architectures/drift` (already mounted under viewer middleware in router.rs:225-227 — keep there since drift is read-only)
   - Request: `{ id: String, force_refresh: bool? }`
   - Response: `{ status: DriftStatus, findings: Vec<DriftFinding>, summary: DriftSummary, baseline_version_id: String, snapshot_at: String, computed_at: String, drift_report_id: String? }`
   - Algorithm:
     1. Look up architecture; 404 if not found
     2. If `!force_refresh`, fetch latest drift report from `DriftReportRepository::list_for_architecture(id, limit=1)`. If exists and is recent (< 5 min old per `state.clock`), return cached.
     3. Otherwise: capture live snapshot via `FleetInventoryProvider::capture(...)`, call `compute_drift(model, &snapshot)`, persist via `DriftReportRepository::create(...)`, return.
   - On compute failure (e.g. snapshot capture errors), persist a `DriftReport { status: CheckFailed, error_message }` and return 200 with the failed status.
   - Tracing: `architecture.drift.invoked` / `.computed` / `.cache_hit` / `.failed` with `architecture_id`, `report_id`, `findings_count`.
   - Metrics: `chv_architecture_drift_total{status}`.
5. **Also wire `list_architecture_runs`** (Phase 5 carryover B1):
   - Same shape as the inline doc said. `ApplyRunRepository::list_for_architecture` already exists; map results to `ApplyRunDto` and return.
   - Move route to operator middleware (security finding from Phase 5).
6. New `BffError::DriftCheckFailed { architecture_id, message }` → 502 `DRIFT_CHECK_FAILED` (used only when even persisting the failed report fails).

### UI
7. BFF client (`ui/src/lib/bff/architectures.ts`):
   - Types: `DriftStatus`, `DriftFinding` (discriminated union by `code`), `DriftReport`, `DriftSummary`
   - `getArchitectureDrift(id, force_refresh?)` returning `DriftReport`
   - Wire `listApplyRuns` to the now-real endpoint (replace the Phase-5 fallback comment)

8. Drift store (`ui/src/lib/stores/architecture-drift-store.svelte.ts`): runes store with `mutateWithRefresh`, methods `refresh(force=false)`, `state.report`, `state.loading`, `state.error`.

9. Drift tab on `/architectures/[id]/+page.svelte`:
   - Add 6th tab `'drift'` with `data-testid="tab-drift"`
   - Component `DriftReportPanel.svelte` (≤ 300 lines): summary chips by finding type, per-finding row (testid `drift-finding-row`), refresh button, last-computed-at TTL hint
   - Read-only — **no remediation buttons** (per spec line 239)
   - 7 finding-type icons / colors

10. Sidebar drift badge on dashboard list (`/architectures/+page.svelte`):
    - Each topology card shows a small badge if its `last_drift_status` is `Drifted` (testid `architecture-drift-badge`)
    - **Carve-out**: `last_drift_status` is not yet on `architecture_topologies` — adding it would require a migration. Phase 6 instead reads the latest drift_report on dashboard mount via a single batched call. Acceptable for MVP scale; document the per-topology drift fetch is N round-trips, not a single batched query, and a Phase 7 task is to add a denormalized `last_drift_status` column.

11. Playwright `architectures-drift.spec.ts` (3 tests):
    - `drift-no-drift-shows-clean-banner` (mock empty findings)
    - `drift-with-findings-shows-grouped-list` (mock 2 findings of different types)
    - `drift-refresh-button-fetches-fresh-report` (force_refresh=true)

### Migrations
None. (Carve-out: a denormalized `architecture_topologies.last_drift_status` column would be ideal but is deferred to Phase 7.)

## Phases
- [x] Phase 1: Audit existing surfaces (done — see Inventory)
- [ ] Phase 2: Stage 1 parallel implementation (3 subagents, single worktree)
  - Subagent A (general-purpose) — `chv-architecture-reconcile::drift` + 9 unit tests
  - Subagent B (general-purpose) — BFF `get_architecture_drift` real handler + `list_architecture_runs` (Phase 5 carryover) + integration tests + tracing/metrics
  - Subagent C (typescript-frontend-engineer) — drift store, DriftReportPanel, drift tab, sidebar badge, Playwright spec
- [ ] Phase 3: Reviewer pass (language-specialist, test-analyzer, api-contract; security only if surface changed)
- [ ] Phase 4: Apply MAJOR findings, run quality matrix, push, open PR, merge

## Decisions Made
- **Baseline source**: the architecture model (YAML / topology canvas state) — NOT the apply_run snapshot. Phase 5 didn't snapshot a baseline on apply (orchestrator concern), so the architecture model is the closest stable source-of-truth.
- **Drift staleness**: 5-minute cache via `state.clock`. Force-refresh bypasses cache.
- **Sidebar badge**: per-topology fetch on dashboard mount; denormalized column deferred.
- **Permission/attachment-changed heuristics**: minimal — flagged when explicit signals exist, not exhaustive. Phase 7 hardens.
- **`list_architecture_runs` Phase 5 carryover**: folded into this PR (same surface, same reviewer).

## Acceptance gates
```bash
rtk cargo build --workspace
rtk cargo test --workspace
rtk cargo test -p chv-architecture-reconcile drift
rtk cargo clippy --workspace --all-targets -- -D warnings
rtk cargo fmt --all -- --check
cd ui && rtk npm run check
cd ui && rtk npx vitest run
cd ui && rtk npx playwright test architectures-drift.spec.ts
```

## Errors Encountered
(populate)

## Status
**Phase 2 — dispatching parallel subagents A/B/C for Stage 1 implementation.**
