ALTER TABLE operations ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE operations ADD COLUMN next_retry_at TEXT;
