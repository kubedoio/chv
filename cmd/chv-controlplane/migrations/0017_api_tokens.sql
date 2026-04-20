CREATE TABLE IF NOT EXISTS api_tokens (
    token_id text PRIMARY KEY,
    user_id text NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    name text NOT NULL,
    token_hash text NOT NULL,
    scope text NOT NULL DEFAULT 'full',
    expires_at text,
    last_used_at text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS idx_api_tokens_user ON api_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_api_tokens_hash ON api_tokens(token_hash);
