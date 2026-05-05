-- Activate the bootstrap admin account if it still has the locked marker.
-- Sets password to 'admin' (bcrypt hash). The controlplane logs a security
-- warning on startup if this default password is still in use.
UPDATE users
SET password_hash = '$2b$12$JbNLkka47ajSOyzKo8fKI.CBvQav06.Vrnh4pbZf4VSaLwS7yI71m',
    updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
WHERE username = 'admin' AND password_hash = '!locked';
