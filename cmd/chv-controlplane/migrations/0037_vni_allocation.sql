ALTER TABLE networks ADD COLUMN vni INTEGER DEFAULT 0;
ALTER TABLE networks ADD COLUMN overlay_type TEXT NOT NULL DEFAULT 'none';

CREATE TABLE vni_allocations (
    vni INTEGER NOT NULL PRIMARY KEY,
    network_id TEXT NOT NULL REFERENCES networks(network_id),
    allocated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    released_at TEXT
);
CREATE INDEX idx_vni_allocations_network ON vni_allocations(network_id);
CREATE INDEX idx_vni_allocations_released ON vni_allocations(released_at);
