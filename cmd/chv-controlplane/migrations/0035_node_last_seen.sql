ALTER TABLE node_observed_state ADD COLUMN last_seen_at TEXT;

-- Initialize existing nodes to now
UPDATE node_observed_state SET last_seen_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now');
