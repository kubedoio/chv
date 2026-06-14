# Task Plan: Architecture Designer — Phase 2 (Svelte Flow canvas + inspector)

## Goal
Land an editable Svelte Flow canvas with eight custom node types, a draggable
palette, an inspector panel, edge-validation rules, and a graph⇄YAML
synchronization pipeline behind a feature flag — meeting every Phase 2
acceptance gate from `docs/plans/2026-06-13-architecture-designer-implementation-plan.md` §3.

## Branch
`feat/architecture-designer-phase2-canvas` (forked from `main` at d13b2258)

## Phases
- [x] Phase 1: Lock plan + create branch
- [ ] Phase 2: UI deps + canvas store + edge rules (parallel agent A)
- [ ] Phase 3: Eight node components + palette + inspector (parallel agent B)
- [ ] Phase 4: Page wiring + feature flag + Playwright + a11y (parallel agent C)
- [ ] Phase 5: Independent code review (parallel reviewer agent)
- [ ] Phase 6: Address review findings, run full quality matrix, push, open PR

## Scope ledger (locked from §3 of the plan)

### In scope
1. UI deps: pin `@xyflow/svelte` and `js-yaml` in `ui/package.json`.
2. `ui/src/lib/components/architectures/canvas/Canvas.svelte` — mounts
   `<SvelteFlow>` with `nodeTypes` + `edgeTypes` props.
3. Eight node components in `ui/src/lib/components/architectures/nodes/`:
   `HostNode`, `NetworkNode`, `DatastoreNode`, `ImageNode`, `TemplateNode`,
   `InstanceNode`, `UserNode`, `RoleNode`. Each ≤300 lines.
4. Palette (drag source) + `palette.ts` registry. Adding a new resource
   kind requires changes in exactly two places: YAML model + palette registry.
   Compile-time test enforces.
5. `lib/components/architectures/canvas/edge-rules.ts` — implements every
   row of the edge-rules matrix from `docs/specs/architecture-designer/contracts/graph-contract.md`.
   Companion vitest `edge-rules.test.ts` covers every (source, target, edgeType)
   pair (allowed and disallowed) with ≥95 % line coverage.
6. Inspector (`inspector/Inspector.svelte`) — when a node is selected, edits
   YAML-equivalent fields and updates the canvas store, which regenerates
   the YAML buffer.
7. Graph save/load: `architecture-canvas-store.svelte.ts` calls
   `architectureStore.update(id, expectedVersion, { design_graph_json,
   latest_yaml })`. Both blobs persist atomically through the existing
   `update` handler (BFF already accepts both fields — see Phase 1 wire-up
   at `crates/chv-webui-bff/src/handlers/architectures.rs:312-334`).
8. Per-node validation badges: red (error), yellow (warning), gray (clean),
   bound to the validation findings from Phase 1's
   `ValidationFindingsPanel`. Badge attaches when `finding.resource_ref`
   matches the node's `(kind, name)`.
9. Replace the Overview canvas-placeholder in
   `ui/src/routes/architectures/[id]/+page.svelte` with the live canvas
   when the feature flag is enabled. Flag default OFF in production builds,
   ON in dev — controlled by `PUBLIC_ARCHITECTURE_DESIGNER_CANVAS=1` env var
   (read in `ui/src/lib/feature-flags.ts`, new file).
10. Playwright spec `architectures-canvas.spec.ts` covering: drag-add-host,
    drag-add-instance, draw `placed_on` edge, attempt `placed_on` from
    instance to network (rejected w/ toast), persist, reload page, assert
    state restored.
11. Axe a11y scan in Playwright passes with 0 violations on the canvas page.

### Explicit non-goals
- Plan/Apply pipeline (Phase 4 of the master plan).
- Fleet inventory checks (Phase 3 of the master plan).
- Drift reconciler.
- Multi-user concurrent canvas editing.
- Touch/mobile gestures.

## Acceptance gates (must all be green before PR opens)

```bash
# Rust
rtk cargo build --workspace
rtk cargo test --workspace
rtk cargo clippy --workspace -- -D warnings
rtk cargo fmt --all -- --check

# UI
cd ui && rtk npm run check                           # 0 errors, 0 warnings
cd ui && rtk npx vitest run                          # all green
cd ui && rtk npx playwright test architectures-canvas.spec.ts

# Component-size lint (project rule: ≤300 lines per Svelte component)
find ui/src/lib/components/architectures -name '*.svelte' -exec wc -l {} \; \
  | awk '$1>300 {print; e=1} END {exit e}'
```

## Subagent dispatch plan

| Agent | Subagent type | Owns |
|---|---|---|
| A | typescript-frontend-engineer | UI deps, `architecture-canvas-store.svelte.ts`, `edge-rules.ts` + tests, `palette.ts`, `feature-flags.ts` |
| B | ui-design-engineer | Eight node components + `Canvas.svelte` + `Inspector.svelte` + per-node validation badge bindings |
| C | testing-automation-engineer | Playwright spec `architectures-canvas.spec.ts` + axe a11y scan + page wiring (`[id]/+page.svelte` overview tab swap behind flag) |
| R | reviewer-code-quality | Independent diff review against CLAUDE.md, edge-rules correctness vs `graph-contract.md` matrix, finding↔node binding logic, atomic save (single `update` call carrying both blobs) |

A runs first (foundation). B and C run in parallel after A lands. R runs
**after** A/B/C complete and **before** the PR is opened.

## Key Questions (resolved before dispatch)

1. **Wire format for `design_graph_json`?** ANSWER: locked to
   `docs/specs/architecture-designer/contracts/graph-contract.md` v1.0
   shape: `{ version: "1.0", nodes: [...], edges: [...] }`. The BFF already
   round-trips this column as an opaque string.
2. **How does graph→YAML regeneration work in Phase 2?** ANSWER: Phase 2
   does NOT generate canonical YAML server-side from the graph yet (the BFF
   handler at handler line 532 explicitly defers this). The canvas store
   serializes a *best-effort* YAML on the client using `js-yaml` and posts
   it as `latest_yaml` alongside `design_graph_json`. If empty / invalid,
   the user can open the YAML tab and import explicit YAML — Phase 1 import
   flow already handles this.
3. **Per-node validation binding key?** ANSWER: tuple `(node.data.kind,
   node.data.name)` matched against `finding.resource_ref` shape
   `"<kind>/<name>"` (already produced by `chv-architecture-validate`).
4. **Feature flag scope?** ANSWER: gate the **canvas mount only**. Tabs
   themselves stay visible on `[id]/+page.svelte` so existing Playwright
   selectors don't break. Overview tab shows canvas when flag on, the
   Phase-1 placeholder when off.

## Decisions Made
- **Architecture-store extends, not replaces**: existing `update(id,
  expectedVersion, fields)` already accepts `design_graph_json` and
  `latest_yaml` (Phase 1 added them). The new `architecture-canvas-store`
  delegates to it for persistence — no parallel save path.
- **Edge rules live in TS, not in the Rust validator** for Phase 2: they
  are UX-time guards (reject the drop, toast). The Rust validator already
  catches the same class of mistake at validate-time via `INVALID_EDGE`.
- **Node component size budget**: each ≤300 lines. Inspector field groups
  extract into per-kind `inspector/{Kind}Fields.svelte` partials when growing.
- **Feature flag default**: OFF in `production` builds. For Phase 2 the
  nav is already shipped — flag governs only the canvas mount.

## Errors Encountered
(populate during execution)

## Status
**Phase 2 complete. PR #114 open, CI running, all local quality gates green.**

### Commits on `feat/architecture-designer-phase2-canvas`
| SHA | Description |
|-----|-------------|
| `edf254e` | feat(designer): Phase 2 — canvas + 8 nodes + inspector + palette |
| `4a28e12` | fix(designer): apply Phase 2 review NITs (severity guard, size-test rigor) |

### Final verification (local)
- `cargo build --workspace` — clean
- `cargo test --workspace` — 676 passed, 2 ignored
- `cargo clippy --workspace --all-targets -- -D warnings` — clean
- `cargo fmt --all -- --check` — clean
- `cd ui && npm run check` — 4604 files, **0 errors, 0 warnings**
- `cd ui && npx vitest run` — 41 files, **280 tests pass**
- Component-size guard: all canvas/nodes/inspector components ≤300 lines

### PR
https://github.com/kubedoio/chv/pull/114 — OPEN, MERGEABLE, mergeStateStatus=UNSTABLE (CI in progress).

### Reviewer verdict
ship-it (4 NITs total — 2 applied in `4a28e12`, 2 deferred to Phase 2.1).

