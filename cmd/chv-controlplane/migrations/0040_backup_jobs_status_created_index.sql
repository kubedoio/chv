-- Add composite index on backup_jobs(status, created_at) to support efficient
-- queries that filter by status and order/range by created_at (e.g. list pending
-- jobs, list retryable jobs, prune old jobs).
CREATE INDEX IF NOT EXISTS idx_backup_jobs_status_created ON backup_jobs(status, created_at);
