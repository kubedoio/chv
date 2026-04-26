-- Network firewall/NAT/DHCP/DNS service schema

ALTER TABLE network_desired_state ADD COLUMN firewall_rules_json TEXT;
ALTER TABLE network_desired_state ADD COLUMN nat_rules_json TEXT;
ALTER TABLE network_desired_state ADD COLUMN dhcp_scope_json TEXT;
ALTER TABLE network_desired_state ADD COLUMN dns_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE network_desired_state ADD COLUMN dns_scope_json TEXT;
