CREATE TABLE vtep_registry (
    node_id TEXT NOT NULL PRIMARY KEY REFERENCES nodes(node_id),
    vtep_ip TEXT NOT NULL,
    vtep_port INTEGER NOT NULL DEFAULT 4789,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX idx_vtep_registry_ip ON vtep_registry(vtep_ip);
