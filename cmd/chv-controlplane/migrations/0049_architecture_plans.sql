-- Architecture Designer: plan snapshots.
-- A plan is generated from a specific architecture_version + inventory snapshot.
-- Plans expire (default 15 minutes from created_at, set by application code per
-- ADR-004-Designer) so stale confirmation tokens cannot be applied.
-- inventory_snapshots is created by migration 0048 so its FK is resolvable at
-- table-create time and at row-insert time alike.

CREATE TABLE IF NOT EXISTS architecture_plans (
    id text PRIMARY KEY,
    architecture_id text NOT NULL REFERENCES architecture_topologies (id) ON DELETE CASCADE,
    architecture_version_id text NOT NULL REFERENCES architecture_versions (id) ON DELETE CASCADE,
    inventory_snapshot_id text REFERENCES inventory_snapshots (id) ON DELETE SET NULL,
    mode text NOT NULL
        CHECK (mode IN ('dry_run','confirm')),
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
    discarded_at text
);

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
