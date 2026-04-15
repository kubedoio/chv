CREATE TABLE IF NOT EXISTS bootstrap_tokens (
    token_hash text PRIMARY KEY,
    description text,
    one_time_use boolean NOT NULL DEFAULT false,
    used_at timestamptz,
    expires_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS bootstrap_tokens_expires_at_idx ON bootstrap_tokens (expires_at);
