CREATE TABLE IF NOT EXISTS vm_exports (
    export_id text PRIMARY KEY,
    vm_id text NOT NULL REFERENCES vms(vm_id) ON DELETE CASCADE,
    filename text NOT NULL,
    export_path text NOT NULL,
    status text NOT NULL DEFAULT 'creating',
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_vm_exports_vm ON vm_exports(vm_id);
