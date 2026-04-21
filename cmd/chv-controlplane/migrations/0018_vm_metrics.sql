CREATE TABLE IF NOT EXISTS vm_metrics (
    vm_id text NOT NULL REFERENCES vms (vm_id) ON DELETE CASCADE,
    collected_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    cpu_percent real,
    memory_bytes_used integer,
    memory_bytes_total integer,
    disk_bytes_read integer,
    disk_bytes_written integer,
    net_bytes_rx integer,
    net_bytes_tx integer,
    PRIMARY KEY (vm_id, collected_at)
);

CREATE INDEX IF NOT EXISTS idx_vm_metrics_vm_id
    ON vm_metrics (vm_id);
CREATE INDEX IF NOT EXISTS idx_vm_metrics_collected_at
    ON vm_metrics (collected_at);
