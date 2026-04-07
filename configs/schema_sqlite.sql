CREATE TABLE IF NOT EXISTS api_tokens (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    expires_at TEXT NULL,
    revoked_at TEXT NULL
);

CREATE TABLE IF NOT EXISTS install_status (
    id TEXT PRIMARY KEY,
    data_root TEXT NOT NULL,
    database_path TEXT NOT NULL,
    bridge_name TEXT NOT NULL,
    bridge_exists INTEGER NOT NULL,
    bridge_ip_expected TEXT NOT NULL,
    bridge_ip_actual TEXT NULL,
    bridge_up INTEGER NOT NULL,
    localdisk_path TEXT NOT NULL,
    localdisk_ready INTEGER NOT NULL,
    cloud_hypervisor_path TEXT NOT NULL,
    cloud_hypervisor_found INTEGER NOT NULL,
    cloudinit_supported INTEGER NOT NULL,
    overall_state TEXT NOT NULL,
    last_checked_at TEXT NOT NULL,
    last_bootstrapped_at TEXT NULL,
    last_error TEXT NULL
);

CREATE TABLE IF NOT EXISTS networks (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    mode TEXT NOT NULL,
    bridge_name TEXT NOT NULL,
    cidr TEXT NOT NULL,
    gateway_ip TEXT NOT NULL,
    is_system_managed INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS storage_pools (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    pool_type TEXT NOT NULL,
    path TEXT NOT NULL,
    is_default INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL,
    capacity_bytes INTEGER NULL,
    allocatable_bytes INTEGER NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS images (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    os_family TEXT NOT NULL,
    architecture TEXT NOT NULL,
    format TEXT NOT NULL,
    source_url TEXT NOT NULL,
    checksum TEXT NULL,
    local_path TEXT NOT NULL,
    cloud_init_supported INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS virtual_machines (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    image_id TEXT NOT NULL,
    storage_pool_id TEXT NOT NULL,
    network_id TEXT NOT NULL,
    desired_state TEXT NOT NULL,
    actual_state TEXT NOT NULL,
    vcpu INTEGER NOT NULL,
    memory_mb INTEGER NOT NULL,
    disk_path TEXT NOT NULL,
    seed_iso_path TEXT NULL,
    workspace_path TEXT NOT NULL,
    cloud_hypervisor_pid INTEGER NULL,
    ip_address TEXT NULL,
    mac_address TEXT NULL,
    last_error TEXT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS operations (
    id TEXT PRIMARY KEY,
    resource_type TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    operation_type TEXT NOT NULL,
    state TEXT NOT NULL,
    request_payload TEXT NULL,
    result_payload TEXT NULL,
    error_payload TEXT NULL,
    started_at TEXT NULL,
    finished_at TEXT NULL,
    created_at TEXT NOT NULL
);

