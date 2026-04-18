ALTER TABLE network_desired_state ADD COLUMN dhcp_enabled INTEGER NOT NULL DEFAULT 1;
ALTER TABLE network_desired_state ADD COLUMN ipam_mode TEXT NOT NULL DEFAULT 'internal';
ALTER TABLE network_desired_state ADD COLUMN is_default INTEGER NOT NULL DEFAULT 0;
