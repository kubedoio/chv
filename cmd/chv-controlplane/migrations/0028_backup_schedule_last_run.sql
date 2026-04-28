-- Add last_run_at to backup schedules for cron evaluation

ALTER TABLE backup_schedules ADD COLUMN last_run_at TEXT;
