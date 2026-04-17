-- SQLite: columns are nullable by default; NOT NULL was never enforced in SQLite schema.
-- This migration is a no-op for SQLite.
-- Original intent: allow desired_status to be NULL in vm/volume/network_desired_state.

-- Volume resize intent
ALTER TABLE volume_desired_state ADD COLUMN resize_to_bytes integer;

-- Node scheduling policy and drain options
ALTER TABLE node_desired_state ADD COLUMN scheduling_paused integer NOT NULL DEFAULT 0;
ALTER TABLE node_desired_state ADD COLUMN allow_workload_stop integer;
