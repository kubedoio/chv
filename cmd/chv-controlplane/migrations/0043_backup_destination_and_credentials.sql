-- Add original destination URL to backup_jobs so retention cleanup can
-- reconstruct the correct shipper (s3://, nfs://, null) after target_path
-- has been rewritten to the shipped artifact path/key.
ALTER TABLE backup_jobs ADD COLUMN destination TEXT;

-- Add S3 credential fields to backup_schedules so operators can store
-- per-schedule S3 access keys.  Falls back to environment/IAM when NULL.
ALTER TABLE backup_schedules ADD COLUMN s3_access_key TEXT;
ALTER TABLE backup_schedules ADD COLUMN s3_secret_key TEXT;
