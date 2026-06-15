# Task Plan: Architecture Designer — Phase 3 (Fleet consistency checks)

## Goal
Land Layer 2 fleet-consistency validation per
`docs/plans/2026-06-13-architecture-designer-implementation-plan.md` §3 (Phase 3,
lines 152–170): pure-data check definitions + `InventoryProvider` trait,
implementation reading existing repos, snapshot persistence, BFF
`/v1/architectures/check-fleet`, UI Fleet-check tab, ≥1 unit test fixture
per finding code, BFF integration test, Playwright spec, axe a11y.

## Branch
`feat/architecture-designer-phase3-fleet` (forked from `main` at 02bcd98b — Phase 2)

## Inventory reality (audit results — 2026-06-14)

The plan §2 listed 5 repositories. Audit shows:

| Repo / Table | Status | Plan to use |
|---|---|---|
| `NodeRepository` (`nodes`, `node_inventory`) | ✅ exists | Read for Layer-2 host checks (memory/cpu/schedulability) |
| `networks` table | ✅ exists | New thin `NetworkRepository` (read-only list) |
| `images` table (mig 0007) | ✅ exists | New thin `ImageRepository` (read-only list) |
| `BackupRepository` | ⚠️ exists (jobs + schedules, not targets) | Skip backup-target snapshot; emit `BACKUP_TARGET_UNREACHABLE` only when YAML refers to a backup target — return `unknown` (warning) until a real BackupTargetRepository lands |
| `DatastoreRepository` | ❌ missing | Derive from per-node inventory (`node_inventory.payload_json` includes storage paths/pools); document as derived |
| `BackupTargetRepository` | ❌ missing | Defer per the plan's allowance ("the last one may need to be added — block this phase on its existence"). We DO NOT block — instead emit `BACKUP_TARGET_UNREACHABLE` as a warning when the YAML references a target that we can't verify, with a "fleet snapshot incomplete" note. Tracked as Phase 3.1 follow-up issue. |

This decision keeps the phase scoped without leaving silent gaps. Every
finding code from the plan's enumerated list is still emittable; the
backup-target case carries a documented "incomplete-inventory" caveat.

## Phases
- [ ] Phase 1: Lock plan + scaffold branch (this file)
- [ ] Phase 2: Pure-data fleet checks + InventoryProvider trait (agent A — Rust)
- [ ] Phase 3: chv-architecture-reconcile crate + thin repos + BFF endpoint (agent B — Rust)
- [ ] Phase 4: UI Fleet-check tab + Playwright spec + a11y (agent C — TS)
- [ ] Phase 5: Independent code review (reviewer-code-quality)
- [ ] Phase 6: Address review findings, run full quality matrix, push, open PR

## Scope ledger (locked from §3 of the plan)

### Deliverables

1. **New module `crates/chv-architecture-validate/src/fleet/`**:
   - `mod.rs` — public surface: `check_fleet(model, inventory) -> Vec<Finding>`.
   - `inventory.rs` — `pub trait InventoryProvider` with: `list_nodes()`, `list_networks()`, `list_datastores()`, `list_images()`, `list_backup_targets()` (returns empty placeholder + warning until a repo exists). Plus `pub struct InventorySnapshot` matching the persisted JSON shape.
   - `checks/host.rs` — `HOST_NOT_FOUND`, `HOST_NOT_SCHEDULABLE`, `INSUFFICIENT_MEMORY`, `INSUFFICIENT_CPU`.
   - `checks/network.rs` — `BRIDGE_UNAVAILABLE`, `VLAN_UNAVAILABLE`, `IP_ALREADY_USED`.
   - `checks/datastore.rs` — `DATASTORE_NOT_FOUND`, `DATASTORE_INSUFFICIENT_CAPACITY`.
   - `checks/image.rs` — `IMAGE_NOT_FOUND`.
   - `checks/backup.rs` — `BACKUP_TARGET_UNREACHABLE`, `SECRET_REF_MISSING` (latter is also schema-validated but here the inventory layer flags missing secret refs in the topology).
   - `checks/permissions.rs` — `PERMISSION_DENIED_DEPLOY` (uses caller identity from BFF; trait method `caller_can_deploy() -> bool`).
   - **Stable code registry** added to `chv-architecture-validate/src/codes.rs` for all 13 new codes. Existing `codes_are_unique` test catches dupes.

2. **New crate `crates/chv-architecture-reconcile`**:
   - Cargo.toml: depends on `chv-architecture-validate`, `chv-controlplane-store`, `chv-controlplane-types`, `tokio`, `tracing`, `chv-errors`.
   - `src/lib.rs` — `FleetInventoryProvider` struct implementing the `InventoryProvider` trait by reading `NodeRepository`, the new `NetworkRepository`/`ImageRepository`, and synthesizing datastores from `node_inventory.payload_json`.
   - `src/snapshot.rs` — `capture(provider) -> InventorySnapshot`: serializes the provider's view into a deterministic JSON blob suitable for `InventorySnapshotRepository::create`.
   - **NEW thin repos** in `crates/chv-controlplane-store/src/`:
     - `networks.rs` — `NetworkRepository::list() -> Vec<NetworkRow>`
     - `images.rs` — `ImageRepository::list() -> Vec<ImageRow>`
     Each ≥1 happy-path test using the in-memory test_util DB harness.

3. **BFF endpoint** `POST /v1/architectures/check-fleet` in `crates/chv-webui-bff/src/handlers/architectures.rs`:
   - Body: `{ id }`, optional `{ refresh: bool }` (default true — captures fresh snapshot).
   - Runs the fleet provider, persists the snapshot via `InventorySnapshotRepository`, runs `chv_architecture_validate::fleet::check_fleet`, persists `last_fleet_check_status`, returns `{ status, inventory_snapshot_id, checked_at, findings[] }`.
   - Role-gated: same role layer as `validate` (Operator+).
   - Returns 501 + `BffError::NotImplemented` for `caller_can_deploy()` until role-mapping lands — Phase 4 will refine.

4. **UI Fleet-check tab** on `[id]/+page.svelte`:
   - 4th tab "Fleet check" (alongside Overview, YAML, Validation).
   - Reuses `ValidationFindingsPanel` (Phase 1) — findings have the same `Finding` shape.
   - "Refresh inventory" button calls `architectureStore.checkFleet(id)`. Success populates the panel + a "checked Xs ago" timestamp pill.
   - When ANY finding has `severity === 'error'`, render a red banner: "Deploy blocked by N fleet errors. Resolve before applying." — non-functional in Phase 3 (Apply lands in Phase 4) but visible.
   - Behind the SAME feature flag as Phase 2 — `PUBLIC_ARCHITECTURE_DESIGNER_CANVAS=1`.

5. **Tests**:
   - `cargo test -p chv-architecture-validate fleet::` — green; ≥1 fixture per finding code, deterministic.
   - BFF integration test: seed node with 16GB RAM, instance topology requiring 32GB → exactly one `INSUFFICIENT_MEMORY` finding with `resource_ref: "instance/<name>"`.
   - Playwright `architectures-fleet-check.spec.ts` covers happy path + "deploy blocked" banner.
   - Axe scan on the new tab: 0 serious/critical violations.
   - vitest unit test for `architectureStore.checkFleet` (mock the BFF response).
   - Component-size guard (existing Phase 2 vitest test) covers any new components.

### Non-goals (deferred)
- BackupTargetRepository (defer to Phase 3.1 — see Inventory reality table).
- Snapshot pruning (open issue at PR time per §3 acceptance gate).
- Inventory-snapshot diffing (Phase 4 plan generation).

## Key questions

1. **Where does `caller_can_deploy()` get its identity?**  
   ANSWER: From the BFF's role layer. `architecture:apply` permission grants deploy — until that role exists, the trait returns `true` (so we never spuriously emit `PERMISSION_DENIED_DEPLOY`) and an explicit TODO references the Phase 4 plan endpoint.

2. **How does fleet-check interact with existing `last_validation_status`?**  
   ANSWER: Layer 1 (schema/static, Phase 1) and Layer 2 (fleet, Phase 3) are persisted in distinct columns: `last_validation_status` and `last_fleet_check_status`. Phase 0 already created both. The check-fleet handler updates the latter.

3. **DATASTORE handling without a real DatastoreRepository?**  
   ANSWER: Derive from `node_inventory.payload_json`. The agent inventory crate emits a `storage_paths: [{ name, path, capacity_gb, free_gb }]` block per node. We aggregate these across nodes into a synthetic datastore list keyed by `name`. Document as a Phase 3 stop-gap.

4. **Does the snapshot row REQUIRE a non-null `summary_json`?**  
   ANSWER: Schema allows null. We always emit `{ totals: { hosts, networks, datastores, images, backup_targets }, captured_by }` for observability. Lightweight.

## Decisions Made
- **Two thin repos, not five**: only `NetworkRepository` and `ImageRepository` are new — `Datastore` is derived; `BackupTarget` is deferred. This keeps the migration surface zero and the change set bounded.
- **InventoryProvider is a trait + struct, not a generic** — easier to mock in unit tests, easier to evolve.
- **Findings reuse Phase 1's `Finding` shape verbatim** — no new finding type, callers can render via the existing `FindingItem.svelte`.
- **Behind the Phase 2 flag** — same feature flag gates the new tab. Production stays default-OFF.

## Subagent dispatch plan

| Agent | Subagent type | Owns |
|---|---|---|
| A | python-general-engineer (no — Rust; use general-purpose with strong instructions) | `chv-architecture-validate::fleet` (pure-data + InventoryProvider trait + 13 finding codes + ≥13 fixture tests) |
| B | general-purpose | `chv-architecture-reconcile` crate, thin `NetworkRepository` + `ImageRepository`, BFF check-fleet handler + integration tests |
| C | typescript-frontend-engineer | UI Fleet-check tab, store method, Playwright spec, axe scan |
| R | reviewer-code-quality | Independent diff review against CLAUDE.md, code stability, every finding code emittable |

A and B run in parallel — A produces the trait, B references it (declaring a dev-dep on `chv-architecture-validate` which is already in the workspace; B's reconcile crate can compile against A's trait once A finishes). C runs in parallel with A and B (UI doesn't block on compile-side).

Sequencing: A → B (B compiles against A) → R; C in parallel with A+B; final integration after all complete.

## Acceptance gates (must all be green before PR opens)

```bash
# Rust
rtk cargo build --workspace
rtk cargo test --workspace
rtk cargo test -p chv-architecture-validate fleet
rtk cargo clippy --workspace --all-targets -- -D warnings
rtk cargo fmt --all -- --check
bash scripts/check-no-println.sh   # if it exists

# UI
cd ui && rtk npm run check
cd ui && rtk npx vitest run
cd ui && PUBLIC_ARCHITECTURE_DESIGNER_CANVAS=1 BFF_BASE_URL=http://localhost:8888 rtk npx playwright test architectures-fleet-check.spec.ts
```

## Errors Encountered
(populate during execution)

## Status
**Phase 1 complete — branch created, plan locked. Dispatching agents A, B, C.**
