ALTER TABLE volume_desired_state
    ADD COLUMN snapshot_op TEXT CHECK (snapshot_op IN ('create', 'restore', 'delete'));

ALTER TABLE volume_desired_state
    ADD COLUMN snapshot_name TEXT;

ALTER TABLE volume_desired_state
    ADD COLUMN clone_source_volume_id TEXT;

ALTER TABLE volumes
    ADD COLUMN parent_volume_id TEXT;

ALTER TABLE volumes
    ADD COLUMN snapshot_chain TEXT;
