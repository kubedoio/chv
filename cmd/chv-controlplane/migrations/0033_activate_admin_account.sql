-- This migration was originally used to unlock the bootstrap admin account
-- that earlier versions of migration 0008 created with a '!locked' password.
-- As of CHV 0.2.x, the admin user is no longer created at migration time —
-- install.sh creates it post-migration with a random password.
--
-- This migration is retained as a no-op to preserve migration ordering and
-- to ensure historical deployments that already ran 0033 see no schema diff.
SELECT 1 WHERE 0;
