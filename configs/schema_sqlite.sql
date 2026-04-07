-- Project CHV - MVP-1 Database Schema (SQLite Dialect)

-- Roles for API access control
CREATE TABLE IF NOT EXISTS roles (
    id TEXT PRIMARY KEY,
    name VARCHAR(64) UNIQUE NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- API Tokens (opaque bearer tokens, SHA-256 hashed)
CREATE TABLE IF NOT EXISTS api_tokens (
    id TEXT PRIMARY KEY,
    name VARCHAR(128) NOT NULL,
    token_hash VARCHAR(64) UNIQUE NOT NULL,
    role_id TEXT REFERENCES roles(id) ON DELETE SET NULL,
    expires_at TEXT,
    revoked_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Hypervisor Nodes
CREATE TABLE IF NOT EXISTS nodes (
    id TEXT PRIMARY KEY,
    hostname VARCHAR(128) UNIQUE NOT NULL,
    management_ip TEXT NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'offline',
    maintenance_mode BOOLEAN NOT NULL DEFAULT false,
    total_cpu_cores INTEGER NOT NULL,
    total_ram_mb BIGINT NOT NULL,
    allocatable_cpu_cores INTEGER NOT NULL,
    allocatable_ram_mb BIGINT NOT NULL,
    labels TEXT NOT NULL DEFAULT '{}',
    capabilities TEXT NOT NULL DEFAULT '{}',
    agent_version VARCHAR(64),
    hypervisor_version VARCHAR(64),
    last_heartbeat_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Networks (Linux bridge based)
CREATE TABLE IF NOT EXISTS networks (
    id TEXT PRIMARY KEY,
    name VARCHAR(128) UNIQUE NOT NULL,
    bridge_name VARCHAR(64) UNIQUE NOT NULL,
    cidr TEXT NOT NULL,
    gateway_ip TEXT NOT NULL,
    dns_servers TEXT NOT NULL DEFAULT '[]',
    mtu INTEGER NOT NULL DEFAULT 1500,
    mode VARCHAR(32) NOT NULL DEFAULT 'bridge',
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Storage Pools (local or NFS)
CREATE TABLE IF NOT EXISTS storage_pools (
    id TEXT PRIMARY KEY,
    node_id TEXT REFERENCES nodes(id) ON DELETE CASCADE,
    name VARCHAR(128) NOT NULL,
    pool_type VARCHAR(32) NOT NULL CHECK (pool_type IN ('local', 'nfs')),
    path_or_export TEXT NOT NULL,
    capacity_bytes BIGINT,
    allocatable_bytes BIGINT,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    supports_online_resize BOOLEAN NOT NULL DEFAULT false,
    supports_clone BOOLEAN NOT NULL DEFAULT false,
    supports_snapshot BOOLEAN NOT NULL DEFAULT false,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(node_id, name)
);

-- Cloud Images (templates)
CREATE TABLE IF NOT EXISTS images (
    id TEXT PRIMARY KEY,
    name VARCHAR(128) UNIQUE NOT NULL,
    os_family VARCHAR(64) NOT NULL,
    source_format VARCHAR(32) NOT NULL CHECK (source_format IN ('qcow2', 'raw')),
    normalized_format VARCHAR(32) NOT NULL CHECK (normalized_format IN ('raw')),
    architecture VARCHAR(32) NOT NULL,
    cloud_init_supported BOOLEAN NOT NULL DEFAULT true,
    default_username VARCHAR(64),
    checksum VARCHAR(128),
    status VARCHAR(32) NOT NULL DEFAULT 'importing',
    size_bytes BIGINT,
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    imported_at TEXT
);

-- Virtual Machines
CREATE TABLE IF NOT EXISTS virtual_machines (
    id TEXT PRIMARY KEY,
    name VARCHAR(128) UNIQUE NOT NULL,
    node_id TEXT REFERENCES nodes(id) ON DELETE SET NULL,
    created_by TEXT NOT NULL DEFAULT 'anonymous',
    desired_state VARCHAR(32) NOT NULL DEFAULT 'present',
    actual_state VARCHAR(32) NOT NULL DEFAULT 'provisioning',
    placement_status VARCHAR(32) NOT NULL DEFAULT 'pending',
    spec TEXT NOT NULL,
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Volumes (runtime disks)
CREATE TABLE IF NOT EXISTS volumes (
    id TEXT PRIMARY KEY,
    vm_id TEXT REFERENCES virtual_machines(id) ON DELETE CASCADE,
    pool_id TEXT REFERENCES storage_pools(id) ON DELETE RESTRICT,
    backing_image_id TEXT REFERENCES images(id) ON DELETE SET NULL,
    format VARCHAR(32) NOT NULL CHECK (format IN ('raw')),
    size_bytes BIGINT NOT NULL,
    path TEXT,
    attachment_state VARCHAR(32) NOT NULL DEFAULT 'detached',
    resize_state VARCHAR(32) NOT NULL DEFAULT 'idle',
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- VM Network Attachments
CREATE TABLE IF NOT EXISTS vm_network_attachments (
    id TEXT PRIMARY KEY,
    vm_id TEXT REFERENCES virtual_machines(id) ON DELETE CASCADE,
    network_id TEXT REFERENCES networks(id) ON DELETE RESTRICT,
    mac_address TEXT NOT NULL,
    ip_address TEXT,
    nic_index INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (vm_id, nic_index)
);

-- Snapshots (external qcow2 with backing file)
CREATE TABLE IF NOT EXISTS snapshots (
    id TEXT PRIMARY KEY,
    vm_id TEXT REFERENCES virtual_machines(id) ON DELETE CASCADE,
    volume_id TEXT REFERENCES volumes(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    path TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'creating',
    size_bytes BIGINT NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Operations tracking
CREATE TABLE IF NOT EXISTS operations (
    id TEXT PRIMARY KEY,
    resource_type VARCHAR(64) NOT NULL,
    resource_id TEXT NOT NULL,
    operation_type VARCHAR(64) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    request_payload TEXT NOT NULL DEFAULT '{}',
    result_payload TEXT,
    error_payload TEXT,
    started_at TEXT,
    finished_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Resource Quotas (per-user limits)
CREATE TABLE IF NOT EXISTS resource_quotas (
    user_id TEXT PRIMARY KEY,
    max_cpus INTEGER NOT NULL DEFAULT 8,
    max_memory_mb INTEGER NOT NULL DEFAULT 16384,
    max_vm_count INTEGER NOT NULL DEFAULT 5,
    max_disk_gb INTEGER NOT NULL DEFAULT 100,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Resource Usage (tracks actual usage per user)
CREATE TABLE IF NOT EXISTS resource_usage (
    user_id TEXT PRIMARY KEY,
    cpus_used INTEGER NOT NULL DEFAULT 0,
    memory_mb_used INTEGER NOT NULL DEFAULT 0,
    vm_count INTEGER NOT NULL DEFAULT 0,
    disk_gb_used INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for performance
CREATE INDEX idx_nodes_status ON nodes(status);
CREATE INDEX idx_nodes_maintenance ON nodes(maintenance_mode);
CREATE INDEX idx_vms_node_id ON virtual_machines(node_id);
CREATE INDEX idx_vms_created_by ON virtual_machines(created_by);
CREATE INDEX idx_vms_desired_state ON virtual_machines(desired_state);
CREATE INDEX idx_vms_actual_state ON virtual_machines(actual_state);
CREATE INDEX idx_volumes_vm_id ON volumes(vm_id);
CREATE INDEX idx_volumes_pool_id ON volumes(pool_id);
CREATE INDEX idx_snapshots_vm_id ON snapshots(vm_id);
CREATE INDEX idx_ops_resource ON operations(resource_type, resource_id);
CREATE INDEX idx_ops_status ON operations(status);

-- Insert default roles
INSERT OR IGNORE INTO roles (id, name) VALUES 
    (lower(hex(randomblob(16))), 'admin'),
    (lower(hex(randomblob(16))), 'operator'),
    (lower(hex(randomblob(16))), 'viewer');
