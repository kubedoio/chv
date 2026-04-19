CREATE TABLE IF NOT EXISTS storage_pools (
    pool_id text PRIMARY KEY,
    node_id text REFERENCES nodes(node_id) ON DELETE CASCADE,
    name text NOT NULL,
    backend_class text NOT NULL DEFAULT 'localdisk',
    path text DEFAULT '',
    total_bytes integer NOT NULL DEFAULT 0,
    used_bytes integer NOT NULL DEFAULT 0,
    status text NOT NULL DEFAULT 'available',
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);
