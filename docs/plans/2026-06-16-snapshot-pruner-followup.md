# Architecture Designer — Periodic Retention Pruner (Follow-up)

Date: 2026-06-16
Status: Tracking issue (not yet implemented)
Origin: Phase 7 hardening carve-out — see [`2026-06-13-architecture-designer-implementation-plan.md`](2026-06-13-architecture-designer-implementation-plan.md) §367.

## Goal

Implement the periodic retention pruner for Architecture Designer artefacts so that the `architecture_plans`, `architecture_apply_runs`, `architecture_drift_reports`, and `inventory_snapshots` tables do not grow unbounded.

## Why deferred from Phase 7

Phase 7 is the hardening phase. Its scope is tests, performance benchmarks, documentation, and ADR status flips. A new scheduled job that mutates production state is a new feature, not hardening, so it carved out cleanly.

Phase 7 does ship the **policy contract** (in [`docs/OPERATIONS.md`](../OPERATIONS.md), section "Architecture Designer day-2 operations") so this follow-up only has to honor numbers that already appeared in operator-facing documentation.

## Proposed surface

A new module `chv-controlplane-store::retention` exposes:

```rust
pub struct PrunePolicy {
    pub plans_terminal_ttl: Duration,            // 30 days
    pub apply_runs_succeeded_ttl: Duration,      // 90 days
    pub apply_runs_failed_ttl: Option<Duration>, // None = retain indefinitely
    pub drift_reports_ttl: Duration,             // 14 days
    pub drift_reports_keep_latest_per_arch: bool,// true
    pub inventory_snapshots_ttl: Duration,       // 7 days
    pub dry_run: bool,
}

pub struct PruneSummary {
    pub plans_deleted: u64,
    pub apply_runs_deleted: u64,
    pub drift_reports_deleted: u64,
    pub inventory_snapshots_deleted: u64,
    pub elapsed: Duration,
}

pub async fn run_pruner(
    pool: &SqlitePool,
    clock: &dyn Clock,
    policy: &PrunePolicy,
) -> Result<PruneSummary, PruneError>;
```

Invocation from the controlplane scheduler runs once per hour with a configurable policy loaded from `controlplane.toml`. The scheduler tolerates pruner errors (logs at `WARN`, continues running) — a stuck pruner must never block the controlplane event loop.

## Retention policy (mirrors OPERATIONS.md)

| Table | Retention | Notes |
|-------|-----------|-------|
| `architecture_versions` | Indefinite | Append-only history of intent; do not prune. |
| `architecture_plans` | 30 days after a terminal status (`Applied`, `Discarded`, `Expired`) | Active plans never deleted. |
| `architecture_apply_runs` (succeeded) | 90 days | |
| `architecture_apply_runs` (failed) | Indefinite | Audit trail. |
| `architecture_drift_reports` | 14 days, but always keep the **latest report per architecture** | Latest-per-arch is the user-facing "current drift" view. |
| `inventory_snapshots` | 7 days | High churn; informational. |

The policy values must stay in sync with `docs/OPERATIONS.md` "Architecture Designer day-2 operations". Any future change requires a paired edit in both places.

## Acceptance criteria

1. **Unit tests** — one `#[test]` per table verifying that:
   - Rows older than the TTL are deleted.
   - Rows newer than the TTL are kept.
   - Special exemptions (failed apply_runs, latest-per-arch drift report, active plans) are honored.
2. **Integration test** — simulate a 30-day window using `ManualClock`, populate each table with synthetic rows on a daily cadence, run the pruner three times across the window, and assert the post-state matches the expected retention.
3. **Dry-run flag** — `policy.dry_run = true` returns a `PruneSummary` with the counts that *would* be deleted, mutates nothing.
4. **Observability** — emits Prometheus counters `chv_architecture_retention_pruned_total{table}` and a gauge `chv_architecture_retention_last_run_seconds`.
5. **Idempotency** — running the pruner twice in a row produces 0 deletes on the second run.

## Out of scope

- UI surface for retention policy editing (config-file only for v1).
- Per-organization retention overrides.
- Object-storage backup before deletion (operators rely on the SQLite backup procedure documented in `docs/OPERATIONS.md`).

## Owners

To be assigned at Phase 8 kickoff.
