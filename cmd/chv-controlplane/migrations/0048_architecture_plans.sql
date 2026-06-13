-- Architecture Designer: plan snapshots.
-- A plan is generated from a specific architecture_version + inventory snapshot.
-- Plans expire (default 15 minutes from created_at, set by application code per
-- ADR-004-Designer) so stale confirmation tokens cannot be applied. The
-- inventory_snapshots table is created by migration 0049; SQLite resolves the
-- FK lazily at row-insert time, so forward reference here is intentional and
-- safe.

CREATE TABLE IF NOT EXISTS architecture_plans (
    id text PRIMARY KEY,
    architecture_id text NOT NULL REFERENCES architecture_topologies (id) ON DELETE CASCADE,
    architecture_version_id text NOT NULL REFERENCES architecture_versions (id) ON DELETE CASCADE,
    inventory_snapshot_id text REFERENCES inventory_snapshots (id) ON DELETE SET NULL,
    mode text NOT NULL,
    status text NOT NULL DEFAULT 'draft',
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
