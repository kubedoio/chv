# Architecture Designer — Implementation Plan

**Date:** 2026-06-13
**Status:** Proposed (depends on acceptance of ADR-001-Designer through ADR-006-Designer)
**Owner:** TBD
**Supersedes / Refines:** [`2026-06-13-architecture-designer-roadmap.md`](2026-06-13-architecture-designer-roadmap.md) — that document is the high-level roadmap; **this** document is the executable plan with code surfaces, subagent assignments, and acceptance gates.

> **Read first.** Before any work starts, the implementer must have read:
> - [`docs/specs/architecture-designer/README.md`](../specs/architecture-designer/README.md)
> - All six [`docs/specs/adr/00*-designer-*.md`](../specs/adr/) ADRs
> - All five [`docs/specs/component/architecture-designer-*.md`](../specs/component/) component specs
> - All four [`docs/specs/architecture-designer/contracts/`](../specs/architecture-designer/contracts/) contracts
> - The [`docs/schemas/chvarchitecture-v1alpha1.schema.json`](../schemas/chvarchitecture-v1alpha1.schema.json) JSON schema and the example YAML files

---

## 1. Scope and non-goals

### In scope (MVP)
A SvelteKit-based design-time topology editor (Architecture Designer) with:
- A new `DESIGN` left-nav group above Fleet Overview (per ADR-001-Designer).
- Editable Svelte Flow canvas with custom node types for the 8 CHV-native resource kinds (per ADR-002-Designer).
- `CHVArchitecture` (`chv.kubedo.io/v1alpha1`) YAML as the source of truth, with bidirectional graph↔YAML conversion (per ADR-003-Designer).
- Three-layer validation: schema/static → fleet consistency → deployment safety (per ADR-004-Designer).
- Plan/Apply pipeline gated by typed confirmation, executed via the existing CHV Operations system (per ADR-004-Designer).
- Read-only drift detection (per component spec `architecture-designer-reconciler.md` and ADR-005-Designer).
- Per-architecture RBAC against existing role infrastructure (per `architecture-designer-security.md`).

### Explicit non-goals
- Generic TOSCA/Cloudify DSL or plugin framework (ADR-006-Designer).
- Automatic drift remediation (deferred — see §11 *Open questions* and `architecture-designer-reconciler.md`).
- Multi-user concurrent canvas editing with conflict resolution (last-write-wins via `version_number` only).
- Reverse-engineering existing infrastructure into a topology (deferred).
- Topology rollback UI (versions are stored; rollback is a future-phase deliverable).

### Quality bar
This feature ships at **industrial grade**: every phase produces a verifiable artifact, every BFF endpoint has an integration test, every UI route has a Playwright test, every validation finding has a stable code with a unit test, and every reconciler operation is idempotent and re-runnable. No phase merges to `main` without **all** of its acceptance gates green.

---

## 2. Code-surface map

| Concern | Existing surface | New artifacts |
|---|---|---|
| Persistence | `crates/chv-controlplane-store` (SQLite, last migration `0045_migration_cancel.sql`) | `0046_architectures.sql` … `0050_architecture_drift.sql` (5 migrations); new repos in `crates/chv-controlplane-store/src/architectures/{mod,topology,version,plan,apply_run,drift}.rs` |
| Domain model | `crates/chv-controlplane-types` | New module `crates/chv-controlplane-types/src/architecture/{mod,model,graph,plan,finding,drift}.rs` |
| Validation engine | (none) | New crate `crates/chv-architecture-validate` (pure-data — no I/O) |
| YAML parser & schema | (none) | Inside `chv-architecture-validate`; uses `serde_yaml`, `jsonschema` |
| Planner & reconciler | (none) | New crate `crates/chv-architecture-reconcile` (depends on `chv-architecture-validate` + `chv-controlplane-store` + the existing `OperationRepository`) |
| BFF HTTP surface | `crates/chv-webui-bff/src/handlers/<resource>.rs` + `router.rs` | New `crates/chv-webui-bff/src/handlers/architectures.rs`; routes registered in `router.rs` under the existing role-gated layers |
| Errors | `crates/chv-errors` | Extend with `ArchitectureError` variants (no panics, ADR-008) |
| Logging | `chv-observability` + `tracing` | Designer routes use existing correlation middleware; no `println!` (ADR-009 enforced by `scripts/check-no-println.sh`) |
| UI routes | `ui/src/routes/` | `ui/src/routes/architectures/{+page.svelte, new/+page.svelte, [id]/{+page.svelte, runs/+page.svelte, drift/+page.svelte, versions/+page.svelte}}` |
| UI components | `ui/src/lib/components/<feature>/` (10 existing folders) | New folder `ui/src/lib/components/architectures/` with subfolders `canvas/`, `nodes/`, `inspector/`, `yaml/`, `plan/`, `drift/`, `dashboard/` |
| UI stores | `ui/src/lib/stores/*.svelte.ts` | `architecture-store.svelte.ts`, `architecture-canvas-store.svelte.ts`, `plan-store.svelte.ts` (must use `mutateWithRefresh` per the existing reactive-state ADR — see [memory: vitest-compliance-glob-import-meta]) |
| UI deps | `ui/package.json` | Add `@xyflow/svelte` (Svelte Flow) and `js-yaml` (YAML stringify/parse for editor) |

> **Proto note.** No `.proto` changes are required. The designer is HTTP/JSON-only on the BFF (per ADR-002-WebUI: browser → BFF only). This avoids breaking the `buf breaking` gate (ADR-014) and keeps the change surface bounded.

---

## 3. Phases, gates, and acceptance criteria

Eight phases. Phases 0-7 follow the roadmap doc; Phase 8 is the ship gate.

Each phase has:
- **Inputs** (what must already exist).
- **Deliverables** (concrete files/code).
- **Acceptance gates** (commands that must pass — no hand-waving).
- **Rollback** (how to back out without leaving debris).

### Phase 0 — Skeleton, persistence, BFF stubs, nav

**Inputs:** Accepted ADRs 001-Designer through 006-Designer.

**Deliverables:**
1. SQLite migrations `0046_architecture_topologies.sql` through `0050_architecture_drift_reports.sql` matching the schemas in `architecture-designer-data-model.md`.
2. New types in `chv-controlplane-types::architecture` with `serde` derives and round-trip tests.
3. New repos in `chv-controlplane-store::architectures` covering create/list/get/update/delete/archive for all five tables. Each repo: ≥1 happy-path test + ≥1 not-found test + ≥1 update-latest-yaml test, all using the existing `test_util.rs` in-memory DB harness.
4. BFF handler `handlers/architectures.rs` exposing **only** the CRUD subset:
   `POST /v1/architectures` (create), `POST /v1/architectures/list`, `POST /v1/architectures/get`, `POST /v1/architectures/update`, `POST /v1/architectures/delete`. Validate, plan, apply, etc. return `501 Not Implemented` with structured `BffError::NotImplemented`.
5. UI route shell: `ui/src/routes/architectures/+page.svelte` (empty list), `[id]/+page.svelte` (placeholder), `new/+page.svelte` (form: name, description, environment).
6. Left-nav `DESIGN` group with `Architecture Designer` and `Saved Topologies` links inserted **above** Fleet Overview in the existing sidebar component.
7. Permission strings registered in `chv-webui-bff::auth` for the architecture-scoped permissions enumerated in `architecture-designer-security.md` §A.

**Acceptance gates:**
- `cargo test -p chv-controlplane-store architectures::` — green.
- `cargo test -p chv-webui-bff handlers::architectures::` — green.
- `cargo clippy --workspace -- -D warnings` — clean.
- `cargo fmt --all -- --check` — clean.
- `bash scripts/check-no-println.sh` — clean.
- `cd ui && npx playwright test architectures-skeleton.spec.ts` — green (visits `/architectures`, asserts empty state, creates a topology, asserts it appears in list).
- `cd ui && npm run check` — 0 errors, 0 warnings.

**Rollback:** Drop migrations 0046-0050 (each is its own file; `DROP TABLE IF EXISTS` in a forward-only `0051_revert_architectures.sql` if reverting after release). Remove handler module + nav links + UI routes.

---

### Phase 1 — YAML model, schema validation, static checks

**Inputs:** Phase 0 merged.

**Deliverables:**
1. New crate `crates/chv-architecture-validate` with:
   - `model.rs` — strongly typed `CHVArchitecture` matching the YAML contract (every section a `Vec<T>`).
   - `parse.rs` — `parse_yaml(&str) -> Result<CHVArchitecture, ParseError>`.
   - `schema.rs` — embeds `docs/schemas/chvarchitecture-v1alpha1.schema.json` via `include_str!` and validates with the `jsonschema` crate. **One** `ValidationFinding` per JSON-Schema error — no aggregation that hides individual fields.
   - `static_checks.rs` — implements every static check from `architecture-designer-validation.md` Layer 1 (duplicate names, missing refs, CIDR validity, CIDR overlap, IP-in-CIDR, gateway-in-CIDR, DHCP-range-valid, role-permissions-valid, no-raw-secrets, platform-vs-instance-user separation).
   - `finding.rs` — `Finding { severity, code, message, path, resource_ref, blocking, suggestion }` with stable `&'static str` codes.
2. **Stable code registry** at `crates/chv-architecture-validate/src/codes.rs` listing every code as a `pub const` constant. New codes never reuse a retired string. Test: `codes_are_unique()`.
3. BFF endpoint `POST /v1/architectures/validate` returns `{status, summary, findings[]}` per the validation/plan contract.
4. BFF endpoint `POST /v1/architectures/generate-yaml` (graph → YAML) and `POST /v1/architectures/import-yaml` (YAML string → graph + normalized model). Round-trip property test in the BFF.
5. UI: YAML side-panel in the `[id]/+page.svelte` view (read-only initially; editable behind a feature flag), import-from-YAML drag-drop in `new/+page.svelte`.
6. Validation findings panel that groups by severity, shows code, message, path, suggestion, and links to the offending node when one is selected on the canvas.

**Acceptance gates:**
- Unit-test fixture set: `crates/chv-architecture-validate/tests/fixtures/` with **at least** one passing example per Phase-1 finding code, plus the canonical good example from `docs/examples/chvarchitecture-example.yaml`.
- `cargo test -p chv-architecture-validate` — green; `cargo tarpaulin -p chv-architecture-validate` ≥ 90% line coverage on `static_checks.rs`.
- BFF integration test: validate the canonical example → 0 errors. Validate a fixture with a known-bad CIDR → exactly one `NETWORK_CIDR_INVALID` finding.
- Round-trip test: `import_yaml(generate_yaml(graph)) == graph` for 20+ fuzz-generated graphs (`proptest`).
- Playwright: `architectures-validate.spec.ts` covers static-validate-happy and static-validate-error flows.

**Rollback:** Validation endpoints return 501; UI hides the validation panel behind the feature flag from Phase 0.

---

### Phase 2 — Svelte Flow canvas, inspector, graph store

**Inputs:** Phases 0-1 merged.

**Deliverables:**
1. `ui/package.json` adds `@xyflow/svelte` (pinned minor version) and `js-yaml`.
2. `ui/src/lib/components/architectures/canvas/Canvas.svelte` mounts `<SvelteFlow>` with `nodeTypes` and `edgeTypes` props.
3. Eight custom node components in `ui/src/lib/components/architectures/nodes/{Host,Network,Datastore,Image,Template,Instance,User,Role}Node.svelte`. Each node ≤300 lines per project rule; extract sub-components when growing.
4. Palette component (drag source) wired to a `palette.ts` definition file. Adding a new resource kind in MVP requires updating exactly two places: the YAML model and the palette registry — enforced by a compile-time test in the UI.
5. Edge-validation rules (graph contract §C) in `lib/components/architectures/canvas/edge-rules.ts`. Invalid drops get rejected with a toast. Test matrix in `edge-rules.test.ts` covering every (sourceKind, targetKind, edgeType) combination from the graph contract.
6. Inspector panel (`inspector/Inspector.svelte`): selecting a node shows its YAML-equivalent fields; edits update the canvas store, which regenerates the YAML buffer.
7. Graph save/load: `PUT /v1/architectures/update` accepts `design_graph_json` and `latest_yaml`; both are persisted atomically (single repo call inside one transaction).
8. Visual badges: validation-status pill on each node (red error, yellow warning, gray clean), bound to the validation findings of Phase 1.

**Acceptance gates:**
- Vitest unit tests for `edge-rules.test.ts` enumerate every allowed and disallowed combination and assert the result. Coverage ≥ 95% on `edge-rules.ts`.
- Playwright: `architectures-canvas.spec.ts` performs drag-add-host, drag-add-instance, draw `placed_on` edge, attempt `placed_on` from instance to network (should be rejected with toast), persist, reload page, assert state restored.
- `cd ui && npm run check` — 0 errors, 0 warnings.
- A11y: every interactive node and edge has `aria-label` (axe scan in Playwright passes with 0 violations).
- No component file >300 lines (CI lint: `find ui/src/lib/components/architectures -name '*.svelte' -exec wc -l {} \; | awk '$1>300'` must be empty).

**Rollback:** Hide `DESIGN` nav behind a feature flag; canvas component renders an empty state when the flag is off.

---

### Phase 3 — Fleet consistency checks

**Inputs:** Phases 0-2 merged.

**Deliverables:**
1. New module `chv-architecture-validate::fleet` with the pure-data check definitions; a separate trait `InventoryProvider` for the I/O side.
2. Implementation `chv-architecture-reconcile::FleetInventory` reading from existing repos: `NodeRepository`, `NetworkRepository`, `DatastoreRepository`, `ImageRepository`, `BackupTargetRepository` (the last one may need to be added if missing — block this phase on its existence).
3. Inventory snapshot table `0049_inventory_snapshots.sql` (already created in Phase 0): `id`, `created_at`, `payload_json`. Snapshots are referenced by plan and drift records to anchor a check-result to a fleet point in time.
4. BFF endpoint `POST /v1/architectures/check-fleet` returns `{status, inventory_snapshot_id, checked_at, findings[]}`.
5. All findings from `architecture-designer-validation.md` Layer 2: `HOST_NOT_FOUND`, `HOST_NOT_SCHEDULABLE`, `INSUFFICIENT_MEMORY`, `INSUFFICIENT_CPU`, `BRIDGE_UNAVAILABLE`, `VLAN_UNAVAILABLE`, `IP_ALREADY_USED`, `DATASTORE_NOT_FOUND`, `DATASTORE_INSUFFICIENT_CAPACITY`, `IMAGE_NOT_FOUND`, `BACKUP_TARGET_UNREACHABLE`, `SECRET_REF_MISSING`, `PERMISSION_DENIED_DEPLOY`. Each has a unit test against a stubbed inventory.
6. UI: Fleet-check tab on `[id]/+page.svelte`, with "Refresh inventory" button and a finding list grouped by severity. Refresh records a new snapshot and invalidates the previous one for plan eligibility.

**Acceptance gates:**
- `cargo test -p chv-architecture-validate fleet` — green; ≥1 fixture per finding code.
- BFF integration test seeds a node with 16GB RAM, requests a topology requiring 32GB → expect exactly one `INSUFFICIENT_MEMORY` finding with the right `resource_ref` and `path`.
- Playwright: `architectures-fleet-check.spec.ts` covers happy path and "deploy blocked due to errors" flow.
- Inventory snapshot rows are pruned by a periodic job (or have a `created_at` index used by a future cleanup task — TBD; **do not** ship without at least an issue tracking the cleanup).

**Rollback:** `check-fleet` endpoint returns 501; fleet-check tab hides behind a feature flag.

---

### Phase 4 — Plan generation

**Inputs:** Phases 0-3 merged.

**Deliverables:**
1. `chv-architecture-reconcile::plan` module:
   - `Diff::compute(desired: &Model, snapshot: &Snapshot) -> Vec<PlanChange>`. Pure function. Input is the validated model + the inventory snapshot from Phase 3.
   - `PlanChange` matches the contract object exactly (`action`, `resource_type`, `resource_name`, `resource_ref`, `description`, `risk`, `requires_confirmation`).
   - Operation-ordering function `order_changes(changes) -> Vec<PlanChange>` enforcing: roles → users → datastores → networks → images → templates → instances → disks → network-attachments → cloud-init → backup-policies → drift-baseline.
2. Plan persistence (table `0048_architecture_plans.sql` from Phase 0): `expires_at = created_at + 15 minutes`. A background task or per-request check rejects expired plans.
3. BFF endpoints:
   - `POST /v1/architectures/plan` — generate apply-mode plan.
   - `POST /v1/architectures/destroy-plan` — generate destroy-mode plan.
   - `POST /v1/architectures/discard-plan` — explicit discard.
4. UI: Plan preview screen (`plan/PlanReview.svelte`). Shows summary (N to create, M to update, K to delete) and a per-resource action list, color-coded by risk. Destructive operations are highlighted; `requires_confirmation=true` rows show the typed-name confirmation field.
5. Plan TTL UX: a countdown badge ("expires in 14:32"); if it runs out, the apply button is disabled and the user is prompted to regenerate the plan.

**Acceptance gates:**
- `cargo test -p chv-architecture-reconcile plan::` — green.
- Property test: applying any plan twice produces an empty second plan (idempotency at the diff level).
- Property test: order_changes is total — every input ordering produces the same output sequence (deterministic).
- BFF integration test: create topology with 1 instance referencing a missing image → plan returns `blocked` status with the original schema/fleet finding inlined.
- BFF integration test: stale plan (created_at > 15 min ago) cannot be applied — returns 409 Conflict with code `PLAN_EXPIRED`.
- Playwright: `architectures-plan.spec.ts` covers generate-plan-happy, plan-expired, and plan-with-blocking-finding.

**Rollback:** `plan` and `destroy-plan` endpoints return 501. Plan preview UI hides behind a feature flag.

---

### Phase 5 — Apply / reconciler

**Inputs:** Phases 0-4 merged. **Critical dependency:** the existing `OperationRepository` in `chv-controlplane-store::operations` is the task substrate. Designer apply does not invent a new task model.

**Deliverables:**
1. `chv-architecture-reconcile::apply` module:
   - `apply_plan(plan: &Plan, ops: &OperationRepository, ctx: &ApplyContext) -> Result<ApplyRun, ApplyError>` enqueues one Operation per ordered PlanChange. Each Operation carries metadata `{ architecture_id, architecture_version_id, resource_ref }`.
   - Idempotency: re-applying the same plan after a partial failure picks up where it left off based on existing Operations' status. Verified by an integration test that crashes mid-apply and re-runs.
   - Confirmation token check: `apply_plan` rejects requests where `confirmation.typed_name != topology.name` for destructive plans, and rejects without `acknowledged_warnings` if any warnings are present.
2. BFF endpoints `POST /v1/architectures/apply` and `POST /v1/architectures/destroy` returning `{run_id, task_id, status}`. The `task_id` resolves to the **first** Operation; clients use the existing `/v1/tasks` endpoints to track progress.
3. ApplyRun persistence (table `0048_architecture_apply_runs.sql` from Phase 0). Status transitions: `queued → running → succeeded | partially_failed | failed | cancelled`. Result JSON includes per-resource outcomes.
4. Drift baseline write: after `succeeded` ApplyRun, write a baseline row to `0050_architecture_drift_reports.sql` for use by Phase 6.
5. Production-protection guard: if `topology.environment == "production"`, the apply endpoint also checks `architecture:apply:production` permission and **requires** typed-name confirmation regardless of risk classification.
6. UI: Apply progress page (`runs/[run_id]/+page.svelte`) showing per-Operation status, with deep links to the existing Tasks page. Run history page (`runs/+page.svelte`) listing all ApplyRuns for a topology.

**Acceptance gates:**
- `cargo test -p chv-architecture-reconcile apply::` — green, including the crash-and-resume idempotency test.
- BFF integration test: missing typed-name on destructive plan → 400 with `MISSING_CONFIRMATION`.
- BFF integration test: production environment without `architecture:apply:production` permission → 403.
- Playwright: `architectures-apply.spec.ts` covers happy-apply, destructive-confirmation, and partial-failure (one Operation deliberately fails) flows.
- Logs: every apply emits a structured `tracing::info!(target: "architecture.apply", ...)` event with `architecture_id`, `version_id`, `plan_id`, `run_id` (verified by a log-capture test).
- Metrics: `chv_architecture_apply_total{status}` and `chv_architecture_apply_duration_seconds` histogram are exported (verified by scraping the metrics endpoint in an integration test).

**Rollback:** `apply` and `destroy` endpoints return 501. The drift baseline writer is skipped without affecting data integrity (no foreign-key cascades).

---

### Phase 6 — Drift detection (read-only)

**Inputs:** Phases 0-5 merged. Drift baselines exist after at least one Phase-5 apply succeeds.

**Deliverables:**
1. `chv-architecture-reconcile::drift` module:
   - `compute_drift(baseline: &Baseline, snapshot: &Snapshot) -> Vec<DriftFinding>` (pure function).
   - Drift finding types: `missing_resource`, `unexpected_resource`, `field_changed`, `capacity_changed`, `network_changed`, `permission_changed`, `attachment_changed`. Each has a stable code and a unit test.
2. BFF endpoint `POST /v1/architectures/drift` returning the latest drift report or computing on demand if `force_refresh=true`.
3. UI: Drift tab on `[id]/+page.svelte` showing findings grouped by type. **Read-only** — no remediation buttons (per ADR-005-Designer / `architecture-designer-reconciler.md` MVP guidance).
4. Sidebar drift badge on the topology card in the dashboard list (`+page.svelte`).
5. (Stretch — keep behind a feature flag) Periodic background drift sweep, configurable interval, default off. Out of MVP if the implementer is short on time.

**Acceptance gates:**
- `cargo test -p chv-architecture-reconcile drift::` — green, ≥1 fixture per finding type.
- BFF integration test: apply a topology, mutate a resource directly via the underlying repo to simulate out-of-band change, request drift → expect a `field_changed` finding with the right `path`.
- Playwright: `architectures-drift.spec.ts` covers no-drift and drift-detected flows.

**Rollback:** Drift endpoint returns 501. Drift tab hides behind a feature flag.

---

### Phase 7 — Hardening

**Inputs:** Phases 0-6 merged.

**Deliverables:**
1. Permission-matrix tests: every endpoint × every defined role × every architecture-scoped permission. Generate the matrix programmatically; flag any combination not explicitly tested.
2. Stale-plan E2E: generate plan, sleep 16 minutes (using a fake-clock test harness — do **not** actually sleep in CI), attempt apply, assert refusal with `PLAN_EXPIRED`.
3. Large-graph performance: load a 500-node, 800-edge topology fixture. Acceptance: canvas first-paint < 1.5 s on the project's existing UI perf-test rig; validate completes in < 2 s; plan completes in < 3 s.
4. Import/export round-trip: every fixture in `crates/chv-architecture-validate/tests/fixtures/` round-trips through `generate-yaml → import-yaml → generate-yaml` byte-equal modulo whitespace.
5. Regression sweep: run the existing full Playwright suite + the new architecture suite; assert no pre-existing tests broke. Document any flake in `task_plan.md` and fix root cause (no quarantining without a tracking issue).
6. Documentation pass:
   - Update `docs/OPERATIONS.md` with day-2 operations for designer (backup of architecture tables, retention of plans/runs/snapshots).
   - Update `CONTRIBUTING.md` with how to add a new resource kind end-to-end.
   - Update `docs/specs/architecture-designer/README.md` "Status" header from Proposed → Implemented (this is a docs-only change, gated on every previous phase landing).
7. Move ADRs 001-Designer through 006-Designer to **Accepted** in their `## Status` sections **and** update `docs/decisions/README.md` accordingly. This is a single PR at the end of Phase 7.

**Acceptance gates:**
- Full Playwright suite green.
- `cargo test --workspace` green.
- `cargo clippy --workspace -- -D warnings` clean.
- Permission-matrix test enumerates ≥ N=number_of_endpoints × number_of_roles cases and asserts on each.
- Manual GO/NO-GO checklist in PR description signed off by review.

**Rollback:** This phase is doc + test work; rollback is `git revert`.

---

### Phase 8 — Ship gate

**Inputs:** Phases 0-7 merged into `main`.

**Deliverables:**
1. Release notes entry referencing every Phase 0-7 PR.
2. CHANGELOG entry under `Added`.
3. Feature flag (if any kept from earlier phases) flipped to **on** by default in a separate PR after one full release cycle of bake-in on a staging cluster.
4. Acceptance review with a recorded GO/NO-GO decision linked from the topology of `docs/specs/architecture-designer/README.md`.

---

## 4. Subagent roster and orchestration

The plan parallelizes safely **inside** a phase but **never across** phases — each phase has hard dependencies (tables exist, types are stable, validation codes registered) on the previous one.

### Subagent types and assignments

| Subagent | Phases used | Tooling | Isolation |
|---|---|---|---|
| **rust-platform** (custom alias for `golang-general-engineer`'s Rust counterpart — use `general-purpose` if no Rust specialist exists) | 0, 1, 3, 4, 5, 6 | `cargo test`, `cargo clippy`, `cargo fmt`, repo-aware Rust review | **worktree** (mutates Cargo.lock and crate dirs) |
| **typescript-frontend-engineer** | 0, 1, 2, 4, 5, 6 | `npm run check`, Playwright, Vitest | **worktree** when run in parallel with rust-platform |
| **database-engineer** | 0, 3, 5, 6 | Migration review, indexing, retention strategy | shared (read-mostly) |
| **testing-automation-engineer** | 1-7 | Playwright, Vitest, `proptest`, fixture authorship | shared |
| **reviewer-language-specialist** | end of every phase | Rust + TypeScript review | shared (read-only) |
| **reviewer-security** | 0 (permissions), 5 (apply auth), 7 (final sweep) | RBAC review, secret-ref handling | shared (read-only) |
| **reviewer-api-contract** | 0, 4, 5 | Validates BFF endpoints against `api-contract.md` | shared (read-only) |
| **reviewer-test-analyzer** | end of every phase | Confirms acceptance-gate tests cover the deliverables | shared (read-only) |

### Orchestration pattern per phase

Each phase is a 4-stage pipeline. Use the `Workflow` tool only when the user has explicitly opted into multi-agent orchestration; otherwise dispatch agents one at a time via `Agent` calls.

```
Stage 1 (parallel): rust-platform + typescript-frontend-engineer + database-engineer
                    each working on disjoint files in a single worktree.
Stage 2 (serial):   testing-automation-engineer adds/extends tests against the
                    deliverables from Stage 1.
Stage 3 (parallel): reviewer-language-specialist + reviewer-test-analyzer +
                    (Phase-specific reviewers per the table above).
Stage 4 (serial):   Coordinator agent merges all reviewer findings, opens
                    follow-up tasks for non-blocking findings, ships PR.
```

### Worktree isolation rules

- Stage 1 agents write to **the same worktree** because Phase artifacts span Rust + UI + migrations and a single PR ships them together. Stage-1 parallelism is achieved by *file-disjoint* assignments, not by separate worktrees.
- Reviewer agents always run in **read-only** mode against the worktree.
- **Never** spawn two `rust-platform` agents in the same worktree — `cargo build` cache contention causes flakes. Either serialize them or use one agent that batches the work.

### Death-loop prevention

- Each phase has a hard 8-hour wall-clock budget. If an agent exceeds it, the coordinator stops the phase and surfaces blockers to the human.
- Reviewer findings that score below 80/100 confidence are deferred to a tracking issue, not blocking merge (consistent with `reviewer-code-quality` confidence threshold).
- A phase that fails its acceptance gates **three times** is rolled back to the prior phase boundary and the failure is re-planned with the human. No "just one more try" loop.

---

## 5. Validation matrix

| Layer | Tool | Where it runs | Gate phase |
|---|---|---|---|
| Rust unit | `cargo test -p <crate>` | Local + CI | Every phase |
| Rust integration | `cargo test -p chv-webui-bff --test architectures` | Local + CI | 0, 1, 3, 4, 5, 6 |
| Rust property | `proptest` inside `chv-architecture-validate` and `chv-architecture-reconcile` | CI | 1, 2, 4, 7 |
| Rust coverage | `cargo tarpaulin -p chv-architecture-validate` | CI nightly | 1, 7 |
| Vitest unit | `cd ui && npx vitest run` | Local + CI | 2, 4, 5, 6 |
| Vitest compliance | existing `mutation-compliance.test.ts` extended with new stores | CI | 2, 4, 5, 6 |
| Playwright | `cd ui && npx playwright test architectures-*.spec.ts` | CI | Every phase |
| Schema lint | `bash scripts/check-no-println.sh` | CI (already wired) | Every phase |
| Buf gates | `buf lint` + `buf breaking` | CI | None — no proto changes expected |
| A11y | axe scans inside Playwright | CI | 2, 7 |
| Perf | UI perf rig + a Rust micro-bench in `chv-architecture-reconcile` | CI Phase 7 | 7 |
| Permission matrix | Generated matrix test in `chv-webui-bff` | CI | 7 |

A phase **does not merge** until every applicable row in this matrix is green for that phase.

---

## 6. Risks and mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| YAML model drifts from JSON Schema | Medium | High (silent acceptance of bad YAML) | `chv-architecture-validate::schema_drift_test` deserializes the embedded schema and walks the Rust type with reflection-style assertions. Block CI on drift. |
| `OperationRepository` semantics don't fit designer apply (e.g. ordering is implicit, not enforced) | Medium | High (apply ships with a hidden race) | Phase 5 includes a one-day spike in its planning slot to confirm OperationRepository's ordering and partial-failure model. If it doesn't fit, escalate to the human before writing any apply code. |
| Svelte Flow major-version churn | Low | Medium | Pin `@xyflow/svelte` to a minor version; subscribe to upstream releases via `npm outdated` in CI weekly. |
| Production-environment guardrails get bypassed in tests | Medium | High (confidence rot — passing tests on prod-protected paths that don't actually exercise the guard) | Negative tests are required, not optional, on every endpoint that has an env-aware code path. Reviewer-security sign-off blocks Phase 5 merge. |
| Drift findings give noise (false positives on resource-fields the designer doesn't manage) | High | Medium | Restrict drift comparison to fields the designer **owns** in the YAML contract (whitelist, not blacklist). Document the whitelist in `architecture-designer-reconciler.md`. |
| Inventory snapshot table grows unbounded | Medium | Low (until very high) | Phase 3 ships a tracking issue for periodic snapshot pruning. Phase 7 wires the pruner. |
| Permission-string proliferation (architecture:* explodes existing role schemas) | Low | Medium | Permission registration is centralized in Phase 0; reviewer-security signs off on the full set before any handler uses it. |
| Validation finding-code drift (codes silently changing across phases breaks API consumers) | Medium | High | Single registry file `codes.rs` with `pub const`; PR reviewer must reject any rename. CI test `codes_are_stable.rs` snapshots the set. |
| Coding-agent prompt files (01-07) drift from this plan | Medium | Medium | The seven prompt files in `docs/specs/architecture-designer/prompts/` remain as **inputs to subagents**. This plan is the orchestration; the prompts are the per-step payload. Phase-7 docs pass aligns the two if drift accrues. |

---

## 7. Mapping: roadmap phase → prompt file → this plan's phase

| Roadmap phase | Prompt file | This plan's phase | Notes |
|---|---|---|---|
| 0 — Contracts and skeleton | `01-foundation.md` | Phase 0 | 1:1 |
| 1 — YAML model and validation | `03-yaml-schema-validator.md` | Phase 1 | YAML + static checks consolidated |
| 2 — Svelte Flow designer | `02-svelte-flow-canvas.md` | Phase 2 | 1:1 |
| 3 — Fleet consistency | `04-fleet-consistency-checks.md` | Phase 3 | 1:1 |
| 4 — Plan generation | `05-plan-apply-reconciler.md` (first half) | Phase 4 | Split: this plan separates plan from apply for review-gate clarity |
| 5 — Apply / reconcile | `05-plan-apply-reconciler.md` (second half) | Phase 5 | Split as above |
| 6 — Drift detection | `06-dashboard-saved-topologies.md` (drift portion) | Phase 6 | Dashboard + drift split: dashboard work folds into Phase 0 + 5 |
| 7 — Hardening | `07-validate-complete-feature.md` | Phase 7 | 1:1 |
| (none) | (none) | Phase 8 | Ship gate is new in this plan |

The seven prompts remain authoritative for **how a subagent does the work**. This plan is authoritative for **what each phase delivers, how it is gated, and how subagents are coordinated**.

---

## 8. Acceptance: how the human knows we're done

A single end-to-end demo against a staging cluster:

1. Operator logs in, navigates `DESIGN → Architecture Designer`, clicks `New`.
2. Drags 1 host, 2 networks, 1 datastore, 2 templates, 4 instances onto the canvas. Wires edges.
3. Inspector shows correct fields per node. Saves. Reloads page. State restored.
4. Validate → 0 errors.
5. Check fleet → exposes a real `INSUFFICIENT_MEMORY` finding because the staging host has 32 GB and the topology asks for 64.
6. Operator reduces instance memory; re-checks; clean.
7. Generate plan → shows 9 creates, 0 updates, 0 deletes. Clicks Apply with typed name.
8. Tasks page shows 9 Operations queued in the right order; first one runs to success.
9. ApplyRun status reaches `succeeded`.
10. Operator manually deletes one VM via SSH (or simulates out-of-band change). Refreshes drift → exactly one `missing_resource` finding.
11. Destroy plan → shows 9 deletes. Apply destroys. Drift clears.

Demo passes ⇒ all phases green ⇒ ADRs flip Proposed → Accepted.

---

## 9. References

- ADRs: [`docs/specs/adr/001-designer-first-class-surface.md`](../specs/adr/001-designer-first-class-surface.md) … [`006-designer-no-tosca-engine.md`](../specs/adr/006-designer-no-tosca-engine.md)
- Component specs: [`docs/specs/component/architecture-designer-*.md`](../specs/component/)
- Contracts: [`docs/specs/architecture-designer/contracts/`](../specs/architecture-designer/contracts/)
- Coding-agent prompts: [`docs/specs/architecture-designer/prompts/`](../specs/architecture-designer/prompts/)
- Schema: [`docs/schemas/chvarchitecture-v1alpha1.schema.json`](../schemas/chvarchitecture-v1alpha1.schema.json)
- Examples: [`docs/examples/chvarchitecture-example.yaml`](../examples/chvarchitecture-example.yaml), [`docs/examples/architecture-plan-result-example.yaml`](../examples/architecture-plan-result-example.yaml)
- High-level roadmap: [`2026-06-13-architecture-designer-roadmap.md`](2026-06-13-architecture-designer-roadmap.md)
- ADR cross-cuts: ADR-002-WebUI (browser→BFF only), ADR-008 (errors), ADR-009 (logging), ADR-014 (API evolution)

---

## 10. Estimate

| Phase | Engineer-days (best / expected / worst) |
|---|---|
| 0 — Skeleton | 3 / 5 / 8 |
| 1 — YAML + static validation | 4 / 7 / 12 |
| 2 — Canvas | 5 / 9 / 15 |
| 3 — Fleet checks | 3 / 5 / 8 |
| 4 — Plan | 4 / 6 / 10 |
| 5 — Apply | 5 / 9 / 15 |
| 6 — Drift | 3 / 5 / 8 |
| 7 — Hardening | 4 / 7 / 12 |
| 8 — Ship gate | 1 / 2 / 4 |
| **Total** | **32 / 55 / 92** |

At one full-time engineer + reviewer overhead, expected delivery is ~11 calendar weeks. With two engineers parallelizing where this plan allows (Stage-1 disjoint files within a phase), expected drops to ~7 calendar weeks. Worst case assumes one phase rolls back once.

---

## 11. Resolved questions (locked 2026-06-13)

The five §11 questions plus two additional dependency-choice questions were resolved at plan-acceptance time. Implementation begins with these as **locked decisions**; changing any of them requires a plan amendment PR.

| # | Question | Resolution | Rationale |
|---|---|---|---|
| Q1 | New crates vs. extend existing? | **Two new crates:** `chv-architecture-validate` (pure data, no I/O) and `chv-architecture-reconcile` (planner + apply + drift) | `validate` reusable from BFF, CLI, and reconcile without dragging SQLite. Keeps dependency graph clean. |
| Q2 | `architecture:*` permission strings — exist or add? | **Add in Phase 0** via forward-only migration. Grant `architecture:read` to read-only roles; `architecture:*` (less `:apply:production`) to operator/admin roles | Existing role schema has no `architecture:*` namespace. Doing this once in Phase 0 unblocks every later handler. |
| Q3 | Multi-user concurrency model | **Optimistic with `version_number`.** `PUT /architectures/{id}` rejects on stale version; UI banner offers reload | Operator-scale audience; conflicts will be rare. Pessimistic locking adds 4× endpoints + lock-expiry timer for a problem we don't have. Upgrade path stays open. |
| Q4 | Inventory snapshot retention | **Keep last 50 per topology; prune nightly via periodic task in `chv-controlplane-service`.** Tracking issue filed Phase 3, implementation Phase 7 | 50 covers ~weeks of typical use. Snapshots can be large (full inventory JSON). |
| Q5 | Fake-clock harness | **Add `Clock` trait in `chv-common`, introduced Phase 4.** Production binds to `std::time::SystemTime`; tests bind to `ManualClock` with `tick(Duration)` | ~50 lines of code. Avoids `tokio::time::pause()` foot-guns. Used by stale-plan tests in Phase 4 and drift tests in Phase 6. |
| Q6 | YAML library | **`serde_yml`** (Rust, parsing/emitting) and **`js-yaml`** (UI side) | `serde_yaml` is unmaintained as of 2024; `serde_yml` is the maintained fork with the same API. |
| Q7 | JSON Schema library | **`jsonschema` v0.18+** (Rust) | De facto crate; supports JSON Schema draft 2020-12 used by `chvarchitecture-v1alpha1.schema.json`; structured errors with JSON pointer paths. |

These resolutions are reflected in §2 *Code-surface map* and §3 *Phases*.

---

**End of plan.**
