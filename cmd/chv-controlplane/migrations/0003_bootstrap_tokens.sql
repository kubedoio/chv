CREATE TABLE IF NOT EXISTS bootstrap_tokens (
    token_hash text PRIMARY KEY,
    description text,
    one_time_use integer NOT NULL DEFAULT 0,
    used_at text,
    expires_at text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);
CREATE INDEX IF NOT EXISTS bootstrap_tokens_expires_at_idx ON bootstrap_tokens (expires_at);
