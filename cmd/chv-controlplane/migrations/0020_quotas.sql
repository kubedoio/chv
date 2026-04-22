CREATE TABLE IF NOT EXISTS quotas (
    user_id text PRIMARY KEY,
    max_vms integer,
    max_cpu integer,
    max_memory_bytes integer,
    max_storage_bytes integer,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);
