CREATE TABLE migrations (
    migration_id TEXT NOT NULL PRIMARY KEY,
    operation_id TEXT NOT NULL REFERENCES operations(operation_id),
    vm_id TEXT NOT NULL REFERENCES vms(vm_id),
    source_node_id TEXT NOT NULL,
    destination_node_id TEXT NOT NULL,
    phase TEXT NOT NULL DEFAULT 'Pending',
    bytes_transferred INTEGER NOT NULL DEFAULT 0,
    total_bytes INTEGER NOT NULL DEFAULT 0,
    convergence_round INTEGER NOT NULL DEFAULT 0,
    dirty_blocks_remaining INTEGER NOT NULL DEFAULT 0,
    started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    completed_at TEXT,
    error_message TEXT
);
CREATE INDEX idx_migrations_vm ON migrations(vm_id);
CREATE INDEX idx_migrations_phase ON migrations(phase);
CREATE INDEX idx_migrations_operation ON migrations(operation_id);
