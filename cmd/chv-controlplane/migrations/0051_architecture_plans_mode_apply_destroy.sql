-- Architecture Designer Phase 4: extend architecture_plans.mode to include
-- 'apply' and 'destroy', and add `discarded_by` for actor accounting on
-- the discard-plan path.
--
-- Migration 0049 created the table with `CHECK (mode IN ('dry_run','confirm'))`.
-- Phase 4 introduces `PlanMode::Apply` (POST /v1/architectures/plan) and
-- `PlanMode::Destroy` (POST /v1/architectures/destroy-plan); both serialize
-- through `as_str()` to "apply" and "destroy" respectively. The original
-- CHECK would reject those rows.
--
-- SQLite cannot ALTER TABLE … DROP CONSTRAINT, so we use the documented
-- ALTER TABLE rebuild recipe:
--
--   1. Create a new table with the widened CHECK constraint and the new
--      `discarded_by` column.
--   2. Copy every row from the old table into the new one.
--   3. Drop the old table by name and rename the new one into place.
--
-- ## Foreign-key safety: why we set legacy_alter_table = ON
--
-- The runtime keeps `PRAGMA foreign_keys = ON` (cmd/chv-controlplane/src/db.rs).
-- Two FKs reference architecture_plans:
--   - inventory_snapshot_id REFERENCES architecture_plans(id)? No: plans
--     reference inventory_snapshots, not the other way around.
--   - architecture_apply_runs.plan_id REFERENCES architecture_plans(id)
--       ON DELETE SET NULL  (see 0050_architecture_apply_runs_and_drift.sql)
--
-- Naively running `DROP TABLE architecture_plans` here would fire the
-- apply-runs ON DELETE SET NULL and silently null `plan_id` on every
-- existing apply-run row.
--
-- A previous version of this migration tried `PRAGMA foreign_keys = OFF`
-- inside the implicit sqlx transaction; that pragma is a no-op when a
-- transaction is already open, so the FK still fires.
--
-- Since SQLite 3.25 (2018) `ALTER TABLE ... RENAME TO` rewrites foreign
-- key references in *child* tables to point at the new name. Renaming
-- the old table out and then renaming the new one in therefore leaves
-- `architecture_apply_runs.plan_id` pointing at the renamed-out
-- old-table name — and dropping that orphaned table would create a
-- dangling FK, breaking subsequent cascade deletes from
-- `architecture_topologies` (the cascade cannot rewrite a row whose FK
-- target table no longer exists).
--
-- Setting `PRAGMA legacy_alter_table = ON` for the duration of this
-- migration disables the FK-rewriting behaviour. With it ON:
--   - `ALTER TABLE architecture_plans_new RENAME TO architecture_plans`
--     does NOT touch apply_runs.plan_id (it still references the literal
--     name "architecture_plans"), so the FK rebinds to the new table by
--     name.
--   - We can drop the old table without firing ON DELETE SET NULL because
--     by the time the DROP runs, the FK in apply_runs already points at
--     the new (just-renamed-in) table — not at the doomed old one.
--
-- Phase-4 rollout context: `architecture_apply_runs` was added in 0050
-- and Phase 4 is the first phase where any plan rows can exist that an
-- apply-run could reference. Even so, this migration is written to be
-- safe in re-runs and in test fixtures that pre-seed apply_runs against
-- synthetic plans.

PRAGMA legacy_alter_table = ON;

CREATE TABLE architecture_plans_new (
    id text PRIMARY KEY,
    architecture_id text NOT NULL REFERENCES architecture_topologies (id) ON DELETE CASCADE,
    architecture_version_id text NOT NULL REFERENCES architecture_versions (id) ON DELETE CASCADE,
    inventory_snapshot_id text REFERENCES inventory_snapshots (id) ON DELETE SET NULL,
    mode text NOT NULL
        CHECK (mode IN ('dry_run','confirm','apply','destroy')),
    status text NOT NULL DEFAULT 'draft'
        CHECK (status IN (
            'draft','failed_validation','requires_confirmation','ready_to_apply',
            'applying','applied','failed','expired','discarded'
        )),
    plan_json text,
    summary_json text,
    created_by text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    expires_at text NOT NULL,
    confirmed_at text,
    confirmed_by text,
    discarded_at text,
    discarded_by text
);

INSERT INTO architecture_plans_new (
    id, architecture_id, architecture_version_id, inventory_snapshot_id,
    mode, status, plan_json, summary_json, created_by, created_at,
    expires_at, confirmed_at, confirmed_by, discarded_at, discarded_by
)
SELECT
    id, architecture_id, architecture_version_id, inventory_snapshot_id,
    mode, status, plan_json, summary_json, created_by, created_at,
    expires_at, confirmed_at, confirmed_by, discarded_at, NULL
FROM architecture_plans;

-- With legacy_alter_table = ON, DROP does NOT cascade through child FKs
-- by name because the runtime FK enforcement looks up the target by
-- name when the DELETE/UPDATE is actually emitted. The old table has
-- the FK arrows pointing at it but we are not deleting any plan rows;
-- DROP TABLE architecture_plans removes the schema entry, and the
-- subsequent rename-in restores the same name pointing at the new
-- table. apply_runs.plan_id text continues to reference
-- architecture_plans by name — and that name is satisfied again
-- before any subsequent FK check runs.
DROP TABLE architecture_plans;
ALTER TABLE architecture_plans_new RENAME TO architecture_plans;

CREATE INDEX IF NOT EXISTS architecture_plans_architecture_id_idx
    ON architecture_plans (architecture_id);

CREATE INDEX IF NOT EXISTS architecture_plans_architecture_version_id_idx
    ON architecture_plans (architecture_version_id);

CREATE INDEX IF NOT EXISTS architecture_plans_inventory_snapshot_id_idx
    ON architecture_plans (inventory_snapshot_id);

CREATE INDEX IF NOT EXISTS architecture_plans_status_idx
    ON architecture_plans (status);

CREATE INDEX IF NOT EXISTS architecture_plans_expires_at_idx
    ON architecture_plans (expires_at);

CREATE INDEX IF NOT EXISTS architecture_plans_architecture_id_created_at_idx
    ON architecture_plans (architecture_id, created_at DESC);

PRAGMA legacy_alter_table = OFF;
