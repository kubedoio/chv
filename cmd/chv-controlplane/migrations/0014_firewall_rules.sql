CREATE TABLE IF NOT EXISTS firewall_rules (
    rule_id text PRIMARY KEY,
    network_id text NOT NULL REFERENCES networks(network_id) ON DELETE CASCADE,
    direction text NOT NULL DEFAULT 'inbound',
    action text NOT NULL DEFAULT 'allow',
    protocol text NOT NULL DEFAULT 'tcp',
    port_range text DEFAULT '',
    source_cidr text DEFAULT '0.0.0.0/0',
    description text DEFAULT '',
    priority integer NOT NULL DEFAULT 100,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS idx_firewall_rules_network ON firewall_rules(network_id);
