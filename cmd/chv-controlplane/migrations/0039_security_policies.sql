CREATE TABLE security_policies (
    policy_id TEXT NOT NULL PRIMARY KEY,
    vm_id TEXT NOT NULL,
    network_id TEXT NOT NULL,
    default_action TEXT NOT NULL DEFAULT 'deny',
    rules_json TEXT NOT NULL DEFAULT '[]',
    version INTEGER NOT NULL DEFAULT 1,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX idx_security_policies_vm ON security_policies(vm_id);
CREATE INDEX idx_security_policies_network ON security_policies(network_id);

CREATE TABLE rate_limit_policies (
    vm_id TEXT NOT NULL PRIMARY KEY,
    rate_bps INTEGER NOT NULL,
    burst_bytes INTEGER NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
