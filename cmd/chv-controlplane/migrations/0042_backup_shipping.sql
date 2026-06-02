-- Add checksum and shipping metadata to backup_jobs
ALTER TABLE backup_jobs ADD COLUMN checksum TEXT;
ALTER TABLE backup_jobs ADD COLUMN checksum_algorithm TEXT;

-- Add retention_days for time-based retention (complementing retention_count)
ALTER TABLE backup_schedules ADD COLUMN retention_days INTEGER DEFAULT 0;

-- Add storage_backend and target_path to backup_restores for tracking
ALTER TABLE backup_restores ADD COLUMN source_path TEXT;
ALTER TABLE backup_restores ADD COLUMN storage_backend TEXT;
