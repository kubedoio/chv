-- Backup jobs and backup history

CREATE TABLE IF NOT EXISTS backup_jobs (
    job_id TEXT PRIMARY KEY,
    vm_id TEXT NOT NULL,
    name TEXT NOT NULL,
    schedule TEXT NOT NULL,
    destination TEXT NOT NULL,
    retention_days INTEGER NOT NULL DEFAULT 7,
    enabled BOOLEAN NOT NULL DEFAULT true,
    last_run_at TEXT,
    next_run_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE TABLE IF NOT EXISTS backup_history (
    history_id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES backup_jobs(job_id),
    vm_id TEXT NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    status TEXT NOT NULL CHECK (status IN ('Pending', 'Running', 'Succeeded', 'Failed')),
    size_bytes INTEGER,
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS idx_backup_history_job_id ON backup_history(job_id);
CREATE INDEX IF NOT EXISTS idx_backup_history_status ON backup_history(status);
CREATE INDEX IF NOT EXISTS idx_backup_history_started_at ON backup_history(started_at DESC);
