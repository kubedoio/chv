CREATE TABLE IF NOT EXISTS vm_templates (
    template_id text PRIMARY KEY,
    name text NOT NULL,
    description text DEFAULT '',
    cpu_count integer NOT NULL DEFAULT 2,
    memory_bytes integer NOT NULL DEFAULT 2147483648,
    disk_size_bytes integer NOT NULL DEFAULT 10737418240,
    image_id text,
    cloud_init_userdata text,
    network_id text,
    created_by text DEFAULT 'system',
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE TABLE IF NOT EXISTS cloud_init_templates (
    template_id text PRIMARY KEY,
    name text NOT NULL,
    description text DEFAULT '',
    content text NOT NULL DEFAULT '',
    created_by text DEFAULT 'system',
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);
