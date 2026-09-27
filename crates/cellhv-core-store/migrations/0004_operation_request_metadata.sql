-- Durable per-operation request metadata (M2.1b). Pre-0004 operation rows keep
-- NULL in all four columns; submit paths after this migration always populate
-- them together.
ALTER TABLE operations ADD COLUMN requested_by TEXT;
ALTER TABLE operations ADD COLUMN external_operation_id TEXT;
ALTER TABLE operations ADD COLUMN request_unix_ms INTEGER;
ALTER TABLE operations ADD COLUMN legacy_generation INTEGER;
