-- 0055: default serial transport Pty -> Socket
--
-- (Renumbered from 0054: PR #287's 0054_fabric_ip_unique.sql collided
-- with this file's original number — sqlx applies migrations by version
-- and two files claiming 0054 made every fresh controlplane boot fail
-- with "UNIQUE constraint failed: _sqlx_migrations.version". The
-- renumber keeps fabric-lineage databases (54 = fabric_ip_unique)
-- valid; main-lineage databases that applied this migration as 54 are
-- dev-only and unreleased — see the PR for the one-line repair.)
--
-- cloud-hypervisor v43 gates Pty-mode serial output until input arrives
-- on the pty master (vmm/src/serial_manager.rs starts the serial output
-- gate closed and only an epoll input event with EPOLLHUP clear opens
-- it), so a passive consumer — exactly what the agent's console capture
-- is — never receives a byte and console.log stays empty forever. Socket
-- mode streams output to a connected unix-stream client immediately and
-- is the agent's default transport (chv_common::hypervisor::
-- DEFAULT_SERIAL_MODE).
--
-- Flip every surface that still carries the old seeded default: the
-- singleton global-settings row and the built-in profiles. Pty remains a
-- valid, explicitly selectable mode (interactive kernel debugging); it is
-- just no longer anything's default. Rows set to other modes ('File',
-- 'Off') are deliberate choices and are untouched, as are per-VM
-- overrides (vms.hv_serial_mode), which are explicit by construction.
--
-- An operator who deliberately re-selected 'Pty' after deployment is
-- indistinguishable from the seeded default and is migrated too; the
-- migration comment and the release notes document the one-line remedy
-- (re-select Pty in settings) for anyone who wants the old behavior
-- back.

UPDATE hypervisor_settings
SET serial_mode = 'Socket', updated_at = datetime('now')
WHERE id = 1 AND serial_mode = 'Pty';

UPDATE hypervisor_profiles
SET serial_mode = 'Socket'
WHERE is_builtin = 1 AND serial_mode = 'Pty';
