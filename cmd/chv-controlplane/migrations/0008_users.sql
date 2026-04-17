CREATE TABLE IF NOT EXISTS users (
    user_id text PRIMARY KEY DEFAULT (lower(hex(randomblob(4)))||'-'||lower(hex(randomblob(2)))||'-4'||substr(lower(hex(randomblob(2))),2)||'-'||substr('89ab',abs(random())%4+1,1)||substr(lower(hex(randomblob(2))),2)||'-'||lower(hex(randomblob(6)))),
    username text NOT NULL UNIQUE,
    password_hash text NOT NULL,
    role text NOT NULL DEFAULT 'viewer',
    display_name text,
    email text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    last_login_at text
);

-- Bootstrap admin user (password: admin, bcrypt cost 12)
INSERT OR IGNORE INTO users (user_id, username, password_hash, role, display_name)
VALUES (
    '00000000-0000-0000-0000-000000000001',
    'admin',
    '$2b$12$JbNLkka47ajSOyzKo8fKI.CBvQav06.Vrnh4pbZf4VSaLwS7yI71m',
    'admin',
    'Administrator'
);
