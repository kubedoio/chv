ALTER TABLE network_desired_state ADD COLUMN cidr text;
ALTER TABLE network_desired_state ADD COLUMN gateway text;
ALTER TABLE network_desired_state ADD COLUMN nat_enabled integer NOT NULL DEFAULT 0;
