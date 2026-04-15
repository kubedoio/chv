-- Remove NOT NULL constraint on desired_status columns so they can be omitted
-- when only domain-specific fields carry intent.
ALTER TABLE vm_desired_state ALTER COLUMN desired_status DROP NOT NULL;
ALTER TABLE volume_desired_state ALTER COLUMN desired_status DROP NOT NULL;
ALTER TABLE network_desired_state ALTER COLUMN desired_status DROP NOT NULL;

-- Volume resize intent
ALTER TABLE volume_desired_state ADD COLUMN IF NOT EXISTS resize_to_bytes bigint;

-- Node scheduling policy and drain options
ALTER TABLE node_desired_state ADD COLUMN IF NOT EXISTS scheduling_paused boolean NOT NULL DEFAULT false;
ALTER TABLE node_desired_state ADD COLUMN IF NOT EXISTS allow_workload_stop boolean;
