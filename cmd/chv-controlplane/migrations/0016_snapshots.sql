CREATE TABLE IF NOT EXISTS vm_snapshots (
    snapshot_id text PRIMARY KEY,
    vm_id text NOT NULL REFERENCES vms(vm_id) ON DELETE CASCADE,
    name text NOT NULL,
    description text DEFAULT '',
    size_bytes integer DEFAULT 0,
    includes_memory integer NOT NULL DEFAULT 0,
    snapshot_path text NOT NULL,
    status text NOT NULL DEFAULT 'creating',
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS idx_vm_snapshots_vm ON vm_snapshots(vm_id);
