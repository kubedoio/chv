-- Pre-seeded topology starters require a sentinel so the seeder runs
-- exactly once per deployment. Operators who want to opt out can flip
-- the sentinel to '1' before first boot. Operators who want to re-seed
-- after deletion can flip back to '0' and restart the service.

CREATE TABLE IF NOT EXISTS system_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT OR IGNORE INTO system_settings (key, value)
VALUES ('seed_starters_completed', '0');
