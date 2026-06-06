-- Add cooperative cancel flag for in-flight migrations.
-- Set by operator API; observed by migration loop at safe points.
ALTER TABLE migrations ADD COLUMN cancel_requested_at TEXT NULL;
