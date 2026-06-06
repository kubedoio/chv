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

-- NOTE: The bootstrap admin user is seeded at install time by install.sh,
-- which generates a random password, bcrypts it, and writes it to
-- /etc/chv/initial_admin_password (mode 0600, root-owned). The user is
-- created with must_change_password=true (see migration 0044) so the
-- operator is forced to rotate the credential on first login.
