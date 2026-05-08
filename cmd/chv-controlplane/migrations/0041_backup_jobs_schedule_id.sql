-- Add schedule_id to backup_jobs so retention pruning can scope to a specific
-- backup schedule/policy rather than all jobs for a VM.
ALTER TABLE backup_jobs ADD COLUMN schedule_id TEXT;

CREATE INDEX IF NOT EXISTS idx_backup_jobs_schedule_id ON backup_jobs(schedule_id);
