-- Project CHV - MVP-1 Database Schema

CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Roles for API access control
CREATE TABLE roles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(64) UNIQUE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- API Tokens (opaque bearer tokens, SHA-256 hashed)
CREATE TABLE api_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(128) NOT NULL,
    token_hash VARCHAR(64) UNIQUE NOT NULL,
    role_id UUID REFERENCES roles(id) ON DELETE SET NULL,
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Hypervisor Nodes
CREATE TABLE nodes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    hostname VARCHAR(128) UNIQUE NOT NULL,
    management_ip INET NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'offline',
    maintenance_mode BOOLEAN NOT NULL DEFAULT false,
    total_cpu_cores INTEGER NOT NULL,
    total_ram_mb BIGINT NOT NULL,
    allocatable_cpu_cores INTEGER NOT NULL,
    allocatable_ram_mb BIGINT NOT NULL,
    labels JSONB NOT NULL DEFAULT '{}'::jsonb,
    capabilities JSONB NOT NULL DEFAULT '{}'::jsonb,
    agent_version VARCHAR(64),
    hypervisor_version VARCHAR(64),
    last_heartbeat_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Networks (Linux bridge based)
CREATE TABLE networks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(128) UNIQUE NOT NULL,
    bridge_name VARCHAR(64) UNIQUE NOT NULL,
    cidr CIDR NOT NULL,
    gateway_ip INET NOT NULL,
    dns_servers JSONB NOT NULL DEFAULT '[]'::jsonb,
    mtu INTEGER NOT NULL DEFAULT 1500,
    mode VARCHAR(32) NOT NULL DEFAULT 'bridge',
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Storage Pools (local or NFS)
CREATE TABLE storage_pools (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    node_id UUID REFERENCES nodes(id) ON DELETE CASCADE,
    name VARCHAR(128) NOT NULL,
    pool_type VARCHAR(32) NOT NULL CHECK (pool_type IN ('local', 'nfs')),
    path_or_export TEXT NOT NULL,
    capacity_bytes BIGINT,
    allocatable_bytes BIGINT,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    supports_online_resize BOOLEAN NOT NULL DEFAULT false,
    supports_clone BOOLEAN NOT NULL DEFAULT false,
    supports_snapshot BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(node_id, name)
);

-- Cloud Images (templates)
CREATE TABLE images (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(128) UNIQUE NOT NULL,
    os_family VARCHAR(64) NOT NULL,
    source_format VARCHAR(32) NOT NULL CHECK (source_format IN ('qcow2', 'raw')),
    normalized_format VARCHAR(32) NOT NULL CHECK (normalized_format IN ('raw')),
    architecture VARCHAR(32) NOT NULL,
    cloud_init_supported BOOLEAN NOT NULL DEFAULT true,
    default_username VARCHAR(64),
    checksum VARCHAR(128),
    status VARCHAR(32) NOT NULL DEFAULT 'importing',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Virtual Machines
CREATE TABLE virtual_machines (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(128) UNIQUE NOT NULL,
    node_id UUID REFERENCES nodes(id) ON DELETE SET NULL,
    desired_state VARCHAR(32) NOT NULL DEFAULT 'present',
    actual_state VARCHAR(32) NOT NULL DEFAULT 'provisioning',
    placement_status VARCHAR(32) NOT NULL DEFAULT 'pending',
    spec JSONB NOT NULL,
    last_error JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Volumes (runtime disks)
CREATE TABLE volumes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    vm_id UUID REFERENCES virtual_machines(id) ON DELETE CASCADE,
    pool_id UUID REFERENCES storage_pools(id) ON DELETE RESTRICT,
    backing_image_id UUID REFERENCES images(id) ON DELETE SET NULL,
    format VARCHAR(32) NOT NULL CHECK (format IN ('raw')),
    size_bytes BIGINT NOT NULL,
    path TEXT,
    attachment_state VARCHAR(32) NOT NULL DEFAULT 'detached',
    resize_state VARCHAR(32) NOT NULL DEFAULT 'idle',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- VM Network Attachments
CREATE TABLE vm_network_attachments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    vm_id UUID REFERENCES virtual_machines(id) ON DELETE CASCADE,
    network_id UUID REFERENCES networks(id) ON DELETE RESTRICT,
    mac_address MACADDR NOT NULL,
    ip_address INET,
    nic_index INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (vm_id, nic_index)
);

-- Operations tracking
CREATE TABLE operations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    resource_type VARCHAR(64) NOT NULL,
    resource_id UUID NOT NULL,
    operation_type VARCHAR(64) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    request_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    result_payload JSONB,
    error_payload JSONB,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Indexes for performance
CREATE INDEX idx_nodes_status ON nodes(status);
CREATE INDEX idx_nodes_maintenance ON nodes(maintenance_mode);
CREATE INDEX idx_vms_node_id ON virtual_machines(node_id);
CREATE INDEX idx_vms_desired_state ON virtual_machines(desired_state);
CREATE INDEX idx_vms_actual_state ON virtual_machines(actual_state);
CREATE INDEX idx_volumes_vm_id ON volumes(vm_id);
CREATE INDEX idx_volumes_pool_id ON volumes(pool_id);
CREATE INDEX idx_ops_resource ON operations(resource_type, resource_id);
CREATE INDEX idx_ops_status ON operations(status);

-- Insert default roles
INSERT INTO roles (name) VALUES 
    ('admin'),
    ('operator'),
    ('viewer')
ON CONFLICT (name) DO NOTHING;
