CREATE TABLE IF NOT EXISTS images (
    image_id text PRIMARY KEY,
    display_name text NOT NULL,
    image_type text NOT NULL DEFAULT 'disk',
    format text NOT NULL DEFAULT 'qcow2',
    size_bytes integer,
    checksum text,
    source_url text,
    status text NOT NULL DEFAULT 'available',
    node_id text REFERENCES nodes (node_id),
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);
