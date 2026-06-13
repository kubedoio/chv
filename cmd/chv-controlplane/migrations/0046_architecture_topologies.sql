-- Architecture Designer: top-level topology entity.
-- One row per saved topology. Mutable design surface. Uses optimistic
-- concurrency via version_number — clients send the version they read and
-- updates only succeed when the stored version still matches.

CREATE TABLE IF NOT EXISTS architecture_topologies (
    id text PRIMARY KEY,
    name text NOT NULL UNIQUE,
    display_name text,
    description text,
    environment text,
    status text NOT NULL DEFAULT 'draft',
    owner_user_id text,
    design_graph_json text,
    latest_yaml text,
    latest_version_id text,
    last_validation_status text,
    last_fleet_check_status text,
    last_plan_id text,
    last_apply_run_id text,
    last_apply_task_id text,
    last_drift_status text,
    version_number integer NOT NULL DEFAULT 1,
    archived_at text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS architecture_topologies_owner_user_id_idx
    ON architecture_topologies (owner_user_id);

CREATE INDEX IF NOT EXISTS architecture_topologies_status_idx
    ON architecture_topologies (status);

CREATE INDEX IF NOT EXISTS architecture_topologies_archived_at_idx
    ON architecture_topologies (archived_at);

CREATE INDEX IF NOT EXISTS architecture_topologies_created_at_idx
    ON architecture_topologies (created_at DESC);
