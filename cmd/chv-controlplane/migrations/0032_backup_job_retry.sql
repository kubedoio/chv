-- Add retry support for backup jobs
ALTER TABLE backup_jobs ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE backup_jobs ADD COLUMN next_retry_at TEXT;
