-- Architecture Designer: apply runs and drift reports.
-- An apply run records the lifecycle of executing a plan against the fleet,
-- including a link to the operations/task table for streaming logs and
-- progress. Drift reports compare a baseline version against an inventory
-- snapshot to surface manual changes outside of the designer.

CREATE TABLE IF NOT EXISTS architecture_apply_runs (
    id text PRIMARY KEY,
    architecture_id text NOT NULL REFERENCES architecture_topologies (id) ON DELETE CASCADE,
    architecture_version_id text NOT NULL REFERENCES architecture_versions (id) ON DELETE CASCADE,
    plan_id text REFERENCES architecture_plans (id) ON DELETE SET NULL,
    task_id text,
    status text NOT NULL DEFAULT 'queued'
        CHECK (status IN (
            'queued','running','succeeded','partially_failed','failed','cancelled'
        )),
    started_at text,
    finished_at text,
    requested_by text,
    result_json text,
    logs_ref text,
    error_message text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS architecture_apply_runs_architecture_id_idx
    ON architecture_apply_runs (architecture_id);

CREATE INDEX IF NOT EXISTS architecture_apply_runs_architecture_version_id_idx
    ON architecture_apply_runs (architecture_version_id);

CREATE INDEX IF NOT EXISTS architecture_apply_runs_plan_id_idx
    ON architecture_apply_runs (plan_id);

CREATE INDEX IF NOT EXISTS architecture_apply_runs_task_id_idx
    ON architecture_apply_runs (task_id);

CREATE INDEX IF NOT EXISTS architecture_apply_runs_status_idx
    ON architecture_apply_runs (status);

CREATE INDEX IF NOT EXISTS architecture_apply_runs_architecture_id_created_at_idx
    ON architecture_apply_runs (architecture_id, created_at DESC);

CREATE TABLE IF NOT EXISTS architecture_drift_reports (
    id text PRIMARY KEY,
    architecture_id text NOT NULL REFERENCES architecture_topologies (id) ON DELETE CASCADE,
    baseline_version_id text NOT NULL REFERENCES architecture_versions (id) ON DELETE CASCADE,
    inventory_snapshot_id text REFERENCES inventory_snapshots (id) ON DELETE SET NULL,
    status text NOT NULL DEFAULT 'unknown'
        CHECK (status IN ('unknown','no_drift','drifted','check_failed')),
    summary_json text,
    findings_json text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS architecture_drift_reports_architecture_id_idx
    ON architecture_drift_reports (architecture_id);

CREATE INDEX IF NOT EXISTS architecture_drift_reports_baseline_version_id_idx
    ON architecture_drift_reports (baseline_version_id);

CREATE INDEX IF NOT EXISTS architecture_drift_reports_inventory_snapshot_id_idx
    ON architecture_drift_reports (inventory_snapshot_id);

CREATE INDEX IF NOT EXISTS architecture_drift_reports_status_idx
    ON architecture_drift_reports (status);

CREATE INDEX IF NOT EXISTS architecture_drift_reports_architecture_id_created_at_idx
    ON architecture_drift_reports (architecture_id, created_at DESC);
