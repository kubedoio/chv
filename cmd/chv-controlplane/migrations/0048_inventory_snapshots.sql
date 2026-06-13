-- Architecture Designer: inventory snapshots used as the basis for plans
-- and drift detection. A snapshot captures the observed state of the fleet
-- (nodes, datastores, networks, etc.) at a point in time so plans are
-- deterministic against a known fleet view. Pruning is a future task; we
-- only add the index needed for time-ordered listing here.

CREATE TABLE IF NOT EXISTS inventory_snapshots (
    id text PRIMARY KEY,
    source text NOT NULL,
    snapshot_json text NOT NULL,
    summary_json text,
    captured_by text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS inventory_snapshots_created_at_idx
    ON inventory_snapshots (created_at DESC);

CREATE INDEX IF NOT EXISTS inventory_snapshots_source_idx
    ON inventory_snapshots (source);
