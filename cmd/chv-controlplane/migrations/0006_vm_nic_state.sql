CREATE TABLE IF NOT EXISTS vm_nic_desired_state (
    nic_id text PRIMARY KEY,
    vm_id text NOT NULL REFERENCES vms (vm_id) ON DELETE CASCADE,
    network_id text NOT NULL REFERENCES networks (network_id) ON DELETE RESTRICT,
    mac_address text,
    ip_address text,
    nic_model text NOT NULL DEFAULT 'virtio',
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_vm_nic_desired_vm_id ON vm_nic_desired_state (vm_id);
