-- Remove CHECK constraints on status/type enum columns.
-- These constraints prevent adding new states without table recreation.
-- Enum validation is enforced in application code instead.

PRAGMA foreign_keys = OFF;

-- 1. backup_jobs: remove CHECK on status and backup_type
CREATE TABLE backup_jobs_new (
    job_id TEXT PRIMARY KEY,
    vm_id TEXT NOT NULL,
    volume_id TEXT,
    status TEXT NOT NULL DEFAULT 'Pending',
    backup_type TEXT NOT NULL DEFAULT 'full',
    target_path TEXT,
    storage_backend TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    started_at TEXT,
    completed_at TEXT,
    error_message TEXT,
    size_bytes INTEGER
);

INSERT INTO backup_jobs_new SELECT * FROM backup_jobs;
DROP TABLE backup_jobs;
ALTER TABLE backup_jobs_new RENAME TO backup_jobs;

CREATE INDEX idx_backup_jobs_vm_id ON backup_jobs(vm_id);
CREATE INDEX idx_backup_jobs_status ON backup_jobs(status);

-- 2. backup_restores: remove CHECK on status
CREATE TABLE backup_restores_new (
    restore_id TEXT PRIMARY KEY,
    backup_job_id TEXT NOT NULL REFERENCES backup_jobs(job_id),
    target_vm_id TEXT,
    target_volume_id TEXT,
    status TEXT NOT NULL DEFAULT 'Pending',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    started_at TEXT,
    completed_at TEXT,
    error_message TEXT
);

INSERT INTO backup_restores_new SELECT * FROM backup_restores;
DROP TABLE backup_restores;
ALTER TABLE backup_restores_new RENAME TO backup_restores;

CREATE INDEX idx_backup_restores_backup_job_id ON backup_restores(backup_job_id);
CREATE INDEX idx_backup_restores_status ON backup_restores(status);

-- 3. volume_desired_state: remove CHECK on snapshot_op
CREATE TABLE volume_desired_state_new (
    volume_id text PRIMARY KEY REFERENCES volumes (volume_id) ON DELETE CASCADE,
    desired_generation integer NOT NULL,
    desired_status text,
    requested_by text,
    updated_by text,
    attached_vm_id text REFERENCES vms (vm_id) ON DELETE SET NULL,
    attachment_mode text,
    device_name text,
    read_only integer NOT NULL DEFAULT 0,
    requested_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    resize_to_bytes integer,
    snapshot_op text,
    snapshot_name text,
    clone_source_volume_id text
);

INSERT INTO volume_desired_state_new SELECT * FROM volume_desired_state;
DROP TABLE volume_desired_state;
ALTER TABLE volume_desired_state_new RENAME TO volume_desired_state;

CREATE INDEX volume_desired_state_status_idx ON volume_desired_state (desired_status);
CREATE INDEX idx_volume_desired_state_attached_vm_id ON volume_desired_state(attached_vm_id);

PRAGMA foreign_keys = ON;
