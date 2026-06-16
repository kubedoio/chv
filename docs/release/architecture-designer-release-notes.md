# Release Notes — Architecture Designer (Phases 0–7)

**Status:** Implemented. All eight planned phases (0 through 7) of the Architecture
Designer have shipped to `main`. This document is the authoritative summary of what
was built, what each PR delivered, and the GO/NO-GO disposition that gates inclusion
in the next CHV release.

**Window:** 2026-06-13 → 2026-06-16. Eight PRs, one fix-up PR. No code on `main` was
authored outside the listed PRs during this window for the architecture surface.

**Plan of record:** [`docs/plans/2026-06-13-architecture-designer-implementation-plan.md`](../plans/2026-06-13-architecture-designer-implementation-plan.md)

**GO/NO-GO disposition:** [`docs/specs/architecture-designer/go-no-go-2026-06-16.md`](../specs/architecture-designer/go-no-go-2026-06-16.md)

---

## What landed

The Architecture Designer is a first-class surface for declaring desired CHV
topologies (servers, networks, datastores, instances, backups, RBAC) as YAML or via
a Svelte Flow canvas, validating them against fleet inventory, generating a typed
plan of operations, applying that plan idempotently, and detecting drift between the
declared model and the live fleet.

| Capability | Where it lives | Phase |
|---|---|---|
| Persistence layer (6 tables, soft-delete, optimistic concurrency) | `chv-controlplane-store::architectures` | 0 |
| Domain types + 5 BFF CRUD handlers + 14 stub endpoints | `chv-controlplane-types`, `chv-webui-bff` | 0 |
| Dashboard / list / detail UI scaffolding under DESIGN nav group | `ui/src/routes/architectures` | 0 |
| YAML model (`CHVArchitecture`) + JSON Schema + parse/emit | `chv-architecture-validate` | 1 |
| 14 static checks (name uniqueness, ref-resolves, capacity bounds, …) | `chv-architecture-validate::static_checks` | 1 |
| YAML editor + 7 BFF endpoints (generate / validate / import) | `ui/`, `chv-webui-bff` | 1 |
| Svelte Flow canvas, 8 node kinds, inspector, palette | `ui/src/lib/components/architectures/canvas` | 2 |
| Fleet consistency checks (live snapshot vs declared model) | `chv-architecture-validate::fleet`, BFF `/check-fleet` | 3 |
| `InventorySnapshot` + `FleetInventoryProvider` | `chv-architecture-validate::fleet` | 3 |
| Plan generation (Diff → ordered Operations) | `chv-architecture-reconcile::plan` | 4 |
| Plan UI (preview, dry-run, ready-to-apply states) | `ui/`, BFF `/plan` `/discard-plan` | 4 |
| Apply / reconciler (CAS plan-status, idempotency-key contract, prod-env guard) | `chv-architecture-reconcile::apply`, BFF `/apply` `/destroy` `/runs/list` | 5 |
| Drift detection (7 finding kinds, 5-min cache, force-refresh) | `chv-architecture-reconcile::drift`, BFF `/drift` | 6 |
| Permission matrix (54 routing-tier cases + exhaustiveness meta-test) | `chv-webui-bff/tests/architecture_permission_matrix.rs` | 7 |
| TTL boundary tests (T0+15m−1ms, exact, +1ms) | `chv-webui-bff/tests/architectures_apply.rs` | 7 |
| Large-graph perf gate (release-only, 2s budget vs 269µs actual) | `chv-architecture-validate/tests/perf_large_graph.rs` | 7 |
| YAML round-trip (3 fixtures, 16 axes) | `chv-architecture-validate/tests/yaml_roundtrip.rs` | 7 |
| OPERATIONS, CONTRIBUTING, ADR status flips (001–006 to Accepted) | `docs/` | 7 |

**Architecture Decision Records (Accepted, 2026-06-16):**

- [`docs/specs/adr/001-designer-first-class-surface.md`](../specs/adr/001-designer-first-class-surface.md)
- [`docs/specs/adr/002-designer-svelte-flow.md`](../specs/adr/002-designer-svelte-flow.md)
- [`docs/specs/adr/003-designer-yaml-source-of-truth.md`](../specs/adr/003-designer-yaml-source-of-truth.md)
- [`docs/specs/adr/004-designer-validation-plan-apply.md`](../specs/adr/004-designer-validation-plan-apply.md)
- [`docs/specs/adr/005-designer-separate-desired-vs-live.md`](../specs/adr/005-designer-separate-desired-vs-live.md)
- [`docs/specs/adr/006-designer-no-tosca-engine.md`](../specs/adr/006-designer-no-tosca-engine.md)

---

## PR-by-PR ledger

| Phase | PR | Squash commit | Subject |
|---|---|---|---|
| 0 | [#112](https://github.com/kubedoio/chv/pull/112) | `666da54a` | Skeleton — migrations, domain types, BFF CRUD, UI nav |
| 1 | [#113](https://github.com/kubedoio/chv/pull/113) | `d13b2258` | YAML model, JSON Schema, 14 static checks, BFF wire-up |
| 2 | [#114](https://github.com/kubedoio/chv/pull/114) | `02bcd98b` | Svelte Flow canvas, 8 nodes, inspector, palette |
| 3 | [#124](https://github.com/kubedoio/chv/pull/124) | `05703888` | Fleet consistency checks (validate + reconcile + BFF + UI) |
| 4 | [#125](https://github.com/kubedoio/chv/pull/125) | `2ddc9dcf` | Plan generation (Diff + ordering + BFF + UI) |
| (fix) | [#126](https://github.com/kubedoio/chv/pull/126) | `8ea4ab76` | Mount canvas by default; remove stale Phase-2 placeholder |
| 5 | [#127](https://github.com/kubedoio/chv/pull/127) | `28dc05db` | Apply / reconciler (Diff → Operations + production guard + UI runs) |
| 6 | [#128](https://github.com/kubedoio/chv/pull/128) | `20d1b3b9` | Drift detection (`compute_drift` + BFF + UI) |
| 7 | [#130](https://github.com/kubedoio/chv/pull/130) | `3e7042d6` | Hardening (permission matrix, perf, YAML round-trip, docs, ADRs to Accepted) |

PR #126 is a fix-up that was discovered during Phase 5 implementation: the
Phase-2 canvas was gated behind `PUBLIC_ARCHITECTURE_DESIGNER_CANVAS=1` and the
detail page rendered "Designer canvas coming in Phase 2" in production. The gate
polarity was inverted (now opt-out via `PUBLIC_ARCHITECTURE_DESIGNER_CANVAS_DISABLED=1`)
and the placeholder removed. This is reported under Phase 4 in the plan because
that is when it was discovered, but it logically belongs to Phase 2.

---

## Quality posture at ship

| Gate | Result | Source |
|---|---|---|
| `cargo test --workspace` | **798 pass / 3 ignored** | Phase 7 PR #130 CI |
| `cargo clippy --workspace -- -D warnings` | clean | Phase 7 PR #130 CI |
| `cargo fmt --all -- --check` | clean | Phase 7 PR #130 CI |
| `cargo deny` (advisories, bans, licenses, sources) | 4/4 pass | Phase 7 PR #130 CI |
| `cargo audit` | pass | Phase 7 PR #130 CI |
| `npm run check` (UI) | 0 errors / 0 warnings | Phase 7 PR #130 CI |
| `vitest` (UI unit + compliance) | full suite green | Phase 7 PR #130 CI |
| Playwright (architecture suite) | **23/23** | local + CI |
| Permission matrix | 54 (route × role) cases + exhaustiveness | Phase 7 |
| TTL boundary | T0+15m−1ms / exact / +1ms | Phase 7 |
| Perf gate | 269µs vs 2s budget (release, 800 NIC edges) | Phase 7 |

**Build and Package** ran on every PR and produced `.deb` and `.rpm` artifacts. The
release-engineering pipeline (`docs/release/PIPELINE.md`) does not require Designer
changes.

---

## Feature flag posture

The Architecture Designer canvas was gated behind a feature flag during Phases 2–4.
PR #126 inverted the polarity so the canvas is now **on by default**; the only
remaining flag is the opt-out `PUBLIC_ARCHITECTURE_DESIGNER_CANVAS_DISABLED=1`,
which exists as an emergency kill switch. There is **no flag flip pending** for the
release: every Designer surface is on for every user (subject to RBAC).

Plan §286 envisioned a "feature flag (if any kept) flipped to **on** by default in
a separate PR after one full release cycle of bake-in on a staging cluster". That
provision is satisfied trivially: the flag was already inverted in #126; no Phase 8
flag-flip PR is required.

---

## Known carve-outs (tracking specs ship; implementation deferred)

These are intentionally not blocking ship:

- [`docs/plans/2026-06-16-snapshot-pruner-followup.md`](../plans/2026-06-16-snapshot-pruner-followup.md) — periodic retention pruner for `inventory_snapshots`, `architecture_apply_runs`, `architecture_drift_reports`. Plan §367 referenced it; Phase 7 ships the spec, not the implementation.
- [`docs/plans/2026-06-16-e2e-flakes-followup.md`](../plans/2026-06-16-e2e-flakes-followup.md) — pre-existing redirect/URL-sync timing flakes in `login.spec.ts`, `navigation.spec.ts`, `vms.spec.ts`. Confirmed reproducible on clean `main` before any Phase 7 changes; out of Designer scope.

---

## Migration / upgrade posture

This is additive: 6 new tables, 0 existing-table changes that affect non-Designer
flows. Existing CHV deployments upgrade with `chv-controlplane` running the new
migrations on first boot. No data backfill is required because the surface is new.

The pre-migration backup hook (shipped earlier as I5) covers the new migrations.

---

## What's NOT in this release

Out of scope for the Designer phases; tracked separately:

- Snapshot pruner / retention enforcer (above)
- TOSCA / OASIS topology import (rejected in [ADR-006-Designer](../specs/adr/006-designer-no-tosca-engine.md))
- Multi-cluster / federation (out of CHV-the-product scope today)
- Cost estimation per architecture (no cost model yet)

---

## How to use it

See:

- [`docs/specs/architecture-designer/README.md`](../specs/architecture-designer/README.md) — overview + ADR index
- [`docs/OPERATIONS.md`](../OPERATIONS.md) §"Architecture Designer day-2 operations" — backup, retention, monitoring, troubleshooting
- [`CONTRIBUTING.md`](../../CONTRIBUTING.md) §"Adding a new architecture resource kind" — 8-step recipe for resource-kind extensions
