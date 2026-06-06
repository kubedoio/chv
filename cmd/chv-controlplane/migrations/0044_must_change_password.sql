-- Force first-login password rotation for admin accounts seeded by install.sh.
-- The application must check this flag at login and short-circuit to a
-- "change password" flow if true.
ALTER TABLE users ADD COLUMN must_change_password INTEGER NOT NULL DEFAULT 0;
