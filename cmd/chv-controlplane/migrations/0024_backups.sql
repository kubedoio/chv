-- Backup system schema v2: jobs (executions), schedules, restores

DROP TABLE IF EXISTS backup_history;
DROP TABLE IF EXISTS backup_jobs;

-- Backup job executions (individual backup runs)
CREATE TABLE IF NOT EXISTS backup_jobs (
    job_id TEXT PRIMARY KEY,
    vm_id TEXT NOT NULL,
    volume_id TEXT,
    status TEXT NOT NULL CHECK (status IN ('Pending', 'Running', 'Succeeded', 'Failed', 'Cancelled')) DEFAULT 'Pending',
    backup_type TEXT NOT NULL CHECK (backup_type IN ('full', 'incremental')) DEFAULT 'full',
    target_path TEXT,
    storage_backend TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    started_at TEXT,
    completed_at TEXT,
    error_message TEXT,
    size_bytes INTEGER
);

-- Backup schedules (recurring configuration)
CREATE TABLE IF NOT EXISTS backup_schedules (
    schedule_id TEXT PRIMARY KEY,
    vm_id TEXT NOT NULL,
    volume_id TEXT,
    name TEXT NOT NULL,
    cron_expression TEXT NOT NULL,
    retention_count INTEGER NOT NULL DEFAULT 7,
    destination TEXT,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

-- Backup restores
CREATE TABLE IF NOT EXISTS backup_restores (
    restore_id TEXT PRIMARY KEY,
    backup_job_id TEXT NOT NULL REFERENCES backup_jobs(job_id),
    target_vm_id TEXT,
    target_volume_id TEXT,
    status TEXT NOT NULL CHECK (status IN ('Pending', 'Running', 'Succeeded', 'Failed', 'Cancelled')) DEFAULT 'Pending',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    started_at TEXT,
    completed_at TEXT,
    error_message TEXT
);

CREATE INDEX IF NOT EXISTS idx_backup_jobs_vm_id ON backup_jobs(vm_id);
CREATE INDEX IF NOT EXISTS idx_backup_jobs_status ON backup_jobs(status);
CREATE INDEX IF NOT EXISTS idx_backup_schedules_vm_id ON backup_schedules(vm_id);
CREATE INDEX IF NOT EXISTS idx_backup_restores_backup_job_id ON backup_restores(backup_job_id);
CREATE INDEX IF NOT EXISTS idx_backup_restores_status ON backup_restores(status);
