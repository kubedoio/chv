CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    email TEXT NULL,
    role TEXT NOT NULL DEFAULT 'user',
    is_active INTEGER NOT NULL DEFAULT 1,
    last_login_at TEXT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

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

-- Nodes table for multi-node support
CREATE TABLE IF NOT EXISTS nodes (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    hostname TEXT NOT NULL,
    ip_address TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'offline',
    is_local INTEGER NOT NULL DEFAULT 1,
    agent_url TEXT NULL,
    agent_token TEXT NULL,
    agent_token_hash TEXT NULL,
    capabilities TEXT NULL,
    last_seen_at TEXT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Create index on node status for quick filtering
CREATE INDEX IF NOT EXISTS idx_nodes_status ON nodes(status);

CREATE TABLE IF NOT EXISTS networks (
    id TEXT PRIMARY KEY,
    node_id TEXT NOT NULL,
    name TEXT NOT NULL,
    mode TEXT NOT NULL,
    bridge_name TEXT NOT NULL,
    cidr TEXT NOT NULL,
    gateway_ip TEXT NOT NULL,
    is_system_managed INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(node_id) REFERENCES nodes(id) ON DELETE CASCADE,
    UNIQUE(node_id, name)
);

-- Create index for node-scoped queries
CREATE INDEX IF NOT EXISTS idx_networks_node_id ON networks(node_id);

CREATE TABLE IF NOT EXISTS storage_pools (
    id TEXT PRIMARY KEY,
    node_id TEXT NOT NULL,
    name TEXT NOT NULL,
    pool_type TEXT NOT NULL,
    path TEXT NOT NULL,
    is_default INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL,
    capacity_bytes INTEGER NULL,
    allocatable_bytes INTEGER NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(node_id) REFERENCES nodes(id) ON DELETE CASCADE,
    UNIQUE(node_id, name)
);

CREATE INDEX IF NOT EXISTS idx_storage_pools_node_id ON storage_pools(node_id);

CREATE TABLE IF NOT EXISTS images (
    id TEXT PRIMARY KEY,
    node_id TEXT NOT NULL,
    name TEXT NOT NULL,
    os_family TEXT NOT NULL,
    architecture TEXT NOT NULL,
    format TEXT NOT NULL,
    source_format TEXT NOT NULL DEFAULT 'qcow2',
    normalized_format TEXT NOT NULL DEFAULT 'qcow2',
    source_url TEXT NOT NULL,
    checksum TEXT NULL,
    local_path TEXT NOT NULL,
    cloud_init_supported INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(node_id) REFERENCES nodes(id) ON DELETE CASCADE,
    UNIQUE(node_id, name)
);

CREATE INDEX IF NOT EXISTS idx_images_node_id ON images(node_id);

CREATE TABLE IF NOT EXISTS virtual_machines (
    id TEXT PRIMARY KEY,
    node_id TEXT NOT NULL,
    name TEXT NOT NULL,
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
    updated_at TEXT NOT NULL,
    FOREIGN KEY(node_id) REFERENCES nodes(id) ON DELETE CASCADE,
    FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE RESTRICT,
    FOREIGN KEY(storage_pool_id) REFERENCES storage_pools(id) ON DELETE RESTRICT,
    FOREIGN KEY(network_id) REFERENCES networks(id) ON DELETE RESTRICT,
    UNIQUE(node_id, name)
);

CREATE INDEX IF NOT EXISTS idx_vms_node_id ON virtual_machines(node_id);
CREATE INDEX IF NOT EXISTS idx_vms_actual_state ON virtual_machines(actual_state);
CREATE INDEX IF NOT EXISTS idx_vms_desired_state ON virtual_machines(desired_state);

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
 
CREATE TABLE IF NOT EXISTS vm_snapshots (
    id TEXT PRIMARY KEY,
    vm_id TEXT NOT NULL,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    status TEXT NOT NULL,
    FOREIGN KEY(vm_id) REFERENCES virtual_machines(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_snapshots_vm_id ON vm_snapshots(vm_id);

-- Boot logs table for VM serial console output
CREATE TABLE IF NOT EXISTS vm_boot_logs (
    id TEXT PRIMARY KEY,
    vm_id TEXT NOT NULL,
    line_number INTEGER NOT NULL,
    content TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    FOREIGN KEY(vm_id) REFERENCES virtual_machines(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_boot_logs_vm ON vm_boot_logs(vm_id, line_number);

-- Node metrics table for health monitoring
CREATE TABLE IF NOT EXISTS node_metrics (
    id TEXT PRIMARY KEY,
    node_id TEXT NOT NULL,
    cpu_percent REAL,
    memory_used_mb INTEGER,
    memory_total_mb INTEGER,
    disk_used_gb INTEGER,
    disk_total_gb INTEGER,
    timestamp TEXT NOT NULL,
    FOREIGN KEY(node_id) REFERENCES nodes(id) ON DELETE CASCADE
);

-- Create index for node metrics queries
CREATE INDEX IF NOT EXISTS idx_node_metrics_node_id ON node_metrics(node_id);
CREATE INDEX IF NOT EXISTS idx_node_metrics_timestamp ON node_metrics(timestamp);

-- VM boot logs table
CREATE TABLE IF NOT EXISTS vm_boot_logs (
    id TEXT PRIMARY KEY,
    vm_id TEXT NOT NULL,
    line_number INTEGER NOT NULL,
    content TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    FOREIGN KEY(vm_id) REFERENCES virtual_machines(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_vm_boot_logs_vm_id ON vm_boot_logs(vm_id);
CREATE INDEX IF NOT EXISTS idx_vm_boot_logs_timestamp ON vm_boot_logs(timestamp);

-- Roles table for RBAC
CREATE TABLE IF NOT EXISTS roles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    permissions TEXT NOT NULL,
    created_at TEXT NOT NULL
);

-- Insert default roles
INSERT OR IGNORE INTO roles (id, name, permissions, created_at) VALUES
('role-admin', 'admin', '[{"resource": "*", "action": "*}]', datetime('now')),
('role-operator', 'operator', '[{"resource": "vms", "action": "*"}, {"resource": "images", "action": "*"}, {"resource": "networks", "action": "*"}, {"resource": "storage-pools", "action": "*"}, {"resource": "nodes", "action": "read"}]', datetime('now')),
('role-viewer', 'viewer', '[{"resource": "*", "action": "read"}]', datetime('now'));

-- Audit logs table
CREATE TABLE IF NOT EXISTS audit_logs (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    user_name TEXT NOT NULL,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT,
    details TEXT,
    ip_address TEXT,
    success INTEGER NOT NULL,
    error TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_logs_user ON audit_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_resource ON audit_logs(resource_type, resource_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_created ON audit_logs(created_at);

-- VM Templates table for rapid VM provisioning
CREATE TABLE IF NOT EXISTS vm_templates (
    id TEXT PRIMARY KEY,
    node_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    vcpu INTEGER NOT NULL,
    memory_mb INTEGER NOT NULL,
    image_id TEXT NOT NULL,
    network_id TEXT NOT NULL,
    storage_pool_id TEXT NOT NULL,
    cloud_init_config TEXT,
    tags TEXT, -- JSON array
    created_at TEXT NOT NULL,
    FOREIGN KEY(node_id) REFERENCES nodes(id) ON DELETE CASCADE,
    FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE RESTRICT,
    FOREIGN KEY(network_id) REFERENCES networks(id) ON DELETE RESTRICT,
    FOREIGN KEY(storage_pool_id) REFERENCES storage_pools(id) ON DELETE RESTRICT,
    UNIQUE(node_id, name)
);

CREATE INDEX IF NOT EXISTS idx_vm_templates_node_id ON vm_templates(node_id);

-- Cloud-init Templates table for reusable cloud-init configurations
CREATE TABLE IF NOT EXISTS cloud_init_templates (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    content TEXT NOT NULL,
    variables TEXT, -- JSON array of variable names
    created_at TEXT NOT NULL
);

-- Backup jobs for scheduled VM backups
CREATE TABLE IF NOT EXISTS backup_jobs (
    id TEXT PRIMARY KEY,
    vm_id TEXT NOT NULL,
    name TEXT NOT NULL,
    schedule TEXT NOT NULL,
    retention INTEGER DEFAULT 7,
    destination TEXT NOT NULL,
    enabled INTEGER DEFAULT 1,
    last_run TEXT,
    next_run TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY(vm_id) REFERENCES virtual_machines(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_backup_jobs_vm_id ON backup_jobs(vm_id);

-- Backup history for tracking all backup operations
CREATE TABLE IF NOT EXISTS backup_history (
    id TEXT PRIMARY KEY,
    job_id TEXT,
    vm_id TEXT NOT NULL,
    snapshot_id TEXT NOT NULL,
    status TEXT NOT NULL,
    size_bytes INTEGER,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    error TEXT,
    FOREIGN KEY(job_id) REFERENCES backup_jobs(id) ON DELETE SET NULL,
    FOREIGN KEY(vm_id) REFERENCES virtual_machines(id) ON DELETE CASCADE,
    FOREIGN KEY(snapshot_id) REFERENCES vm_snapshots(id)
);

CREATE INDEX IF NOT EXISTS idx_backup_history_vm ON backup_history(vm_id, started_at);
CREATE INDEX IF NOT EXISTS idx_backup_history_job ON backup_history(job_id);

-- VLAN networks table
CREATE TABLE IF NOT EXISTS vlan_networks (
    id TEXT PRIMARY KEY,
    network_id TEXT NOT NULL,
    vlan_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    cidr TEXT NOT NULL,
    gateway_ip TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(network_id) REFERENCES networks(id) ON DELETE CASCADE,
    UNIQUE(network_id, vlan_id)
);

CREATE INDEX IF NOT EXISTS idx_vlan_networks_network ON vlan_networks(network_id);

-- DHCP servers table
CREATE TABLE IF NOT EXISTS dhcp_servers (
    id TEXT PRIMARY KEY,
    network_id TEXT NOT NULL,
    range_start TEXT NOT NULL,
    range_end TEXT NOT NULL,
    lease_time_seconds INTEGER DEFAULT 3600,
    is_running INTEGER DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(network_id) REFERENCES networks(id) ON DELETE CASCADE,
    UNIQUE(network_id)
);

CREATE INDEX IF NOT EXISTS idx_dhcp_servers_network ON dhcp_servers(network_id);

-- DHCP leases table
CREATE TABLE IF NOT EXISTS dhcp_leases (
    id TEXT PRIMARY KEY,
    network_id TEXT NOT NULL,
    mac_address TEXT NOT NULL,
    ip_address TEXT NOT NULL,
    hostname TEXT,
    lease_start TEXT NOT NULL,
    lease_end TEXT NOT NULL,
    FOREIGN KEY(network_id) REFERENCES networks(id) ON DELETE CASCADE,
    UNIQUE(network_id, mac_address),
    UNIQUE(network_id, ip_address)
);

CREATE INDEX IF NOT EXISTS idx_dhcp_leases_network ON dhcp_leases(network_id);
CREATE INDEX IF NOT EXISTS idx_dhcp_leases_mac ON dhcp_leases(network_id, mac_address);

-- Firewall rules table
CREATE TABLE IF NOT EXISTS firewall_rules (
    id TEXT PRIMARY KEY,
    vm_id TEXT NOT NULL,
    direction TEXT NOT NULL,
    protocol TEXT NOT NULL,
    port_range TEXT,
    source_cidr TEXT NOT NULL,
    action TEXT NOT NULL,
    priority INTEGER NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY(vm_id) REFERENCES virtual_machines(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_firewall_rules_vm ON firewall_rules(vm_id, priority);
CREATE INDEX IF NOT EXISTS idx_firewall_rules_direction ON firewall_rules(vm_id, direction);

-- Insert default cloud-init templates
INSERT OR IGNORE INTO cloud_init_templates (id, name, description, content, variables, created_at) VALUES
('cit-basic', 'Basic User Setup', 'Creates a user with sudo access and SSH key', '#cloud-config
hostname: {{.Hostname}}
manage_etc_hosts: true
users:
  - name: {{.Username}}
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - {{.SSHKey}}
chpasswd:
  list: |
    {{.Username}}:{{.Password}}
  expire: False
package_update: true
packages:
  - qemu-guest-agent', '["Hostname", "Username", "SSHKey", "Password"]', datetime('now')),
('cit-docker', 'Docker Ready', 'Ubuntu with Docker pre-installed', '#cloud-config
package_update: true
packages:
  - docker.io
  - qemu-guest-agent
users:
  - name: {{.Username}}
    groups: docker
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - {{.SSHKey}}
runcmd:
  - systemctl enable docker
  - systemctl start docker', '["Username", "SSHKey"]', datetime('now')),
('cit-kubernetes', 'Kubernetes Node', 'Ubuntu with containerd and Kubernetes tools', '#cloud-config
package_update: true
packages:
  - apt-transport-https
  - ca-certificates
  - curl
  - gnupg
  - lsb-release
  - qemu-guest-agent
users:
  - name: {{.Username}}
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - {{.SSHKey}}
runcmd:
  - sysctl -w net.ipv4.ip_forward=1
  - echo "net.ipv4.ip_forward=1" >> /etc/sysctl.conf', '["Username", "SSHKey"]', datetime('now'));
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    email TEXT NULL,
    role TEXT NOT NULL DEFAULT 'user',
    is_active INTEGER NOT NULL DEFAULT 1,
    last_login_at TEXT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

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

-- Nodes table for multi-node support
CREATE TABLE IF NOT EXISTS nodes (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    hostname TEXT NOT NULL,
    ip_address TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'offline',
    is_local INTEGER NOT NULL DEFAULT 1,
    agent_url TEXT NULL,
    agent_token TEXT NULL,
    agent_token_hash TEXT NULL,
    capabilities TEXT NULL,
    last_seen_at TEXT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Create index on node status for quick filtering
CREATE INDEX IF NOT EXISTS idx_nodes_status ON nodes(status);

CREATE TABLE IF NOT EXISTS networks (
    id TEXT PRIMARY KEY,
    node_id TEXT NOT NULL,
    name TEXT NOT NULL,
    mode TEXT NOT NULL,
    bridge_name TEXT NOT NULL,
    cidr TEXT NOT NULL,
    gateway_ip TEXT NOT NULL,
    is_system_managed INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(node_id) REFERENCES nodes(id) ON DELETE CASCADE,
    UNIQUE(node_id, name)
);

-- Create index for node-scoped queries
CREATE INDEX IF NOT EXISTS idx_networks_node_id ON networks(node_id);

CREATE TABLE IF NOT EXISTS storage_pools (
    id TEXT PRIMARY KEY,
    node_id TEXT NOT NULL,
    name TEXT NOT NULL,
    pool_type TEXT NOT NULL,
    path TEXT NOT NULL,
    is_default INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL,
    capacity_bytes INTEGER NULL,
    allocatable_bytes INTEGER NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(node_id) REFERENCES nodes(id) ON DELETE CASCADE,
    UNIQUE(node_id, name)
);

CREATE INDEX IF NOT EXISTS idx_storage_pools_node_id ON storage_pools(node_id);

CREATE TABLE IF NOT EXISTS images (
    id TEXT PRIMARY KEY,
    node_id TEXT NOT NULL,
    name TEXT NOT NULL,
    os_family TEXT NOT NULL,
    architecture TEXT NOT NULL,
    format TEXT NOT NULL,
    source_format TEXT NOT NULL DEFAULT 'qcow2',
    normalized_format TEXT NOT NULL DEFAULT 'qcow2',
    source_url TEXT NOT NULL,
    checksum TEXT NULL,
    local_path TEXT NOT NULL,
    cloud_init_supported INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(node_id) REFERENCES nodes(id) ON DELETE CASCADE,
    UNIQUE(node_id, name)
);

CREATE INDEX IF NOT EXISTS idx_images_node_id ON images(node_id);

CREATE TABLE IF NOT EXISTS virtual_machines (
    id TEXT PRIMARY KEY,
    node_id TEXT NOT NULL,
    name TEXT NOT NULL,
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
    user_id TEXT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(node_id) REFERENCES nodes(id) ON DELETE CASCADE,
    FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE RESTRICT,
    FOREIGN KEY(storage_pool_id) REFERENCES storage_pools(id) ON DELETE RESTRICT,
    FOREIGN KEY(network_id) REFERENCES networks(id) ON DELETE RESTRICT,
    UNIQUE(node_id, name)
);

CREATE INDEX IF NOT EXISTS idx_vms_node_id ON virtual_machines(node_id);
CREATE INDEX IF NOT EXISTS idx_vms_actual_state ON virtual_machines(actual_state);
CREATE INDEX IF NOT EXISTS idx_vms_desired_state ON virtual_machines(desired_state);

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
 
CREATE TABLE IF NOT EXISTS vm_snapshots (
    id TEXT PRIMARY KEY,
    vm_id TEXT NOT NULL,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    status TEXT NOT NULL,
    FOREIGN KEY(vm_id) REFERENCES virtual_machines(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_snapshots_vm_id ON vm_snapshots(vm_id);

-- Boot logs table for VM serial console output
CREATE TABLE IF NOT EXISTS vm_boot_logs (
    id TEXT PRIMARY KEY,
    vm_id TEXT NOT NULL,
    line_number INTEGER NOT NULL,
    content TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    FOREIGN KEY(vm_id) REFERENCES virtual_machines(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_boot_logs_vm ON vm_boot_logs(vm_id, line_number);

-- Node metrics table for health monitoring
CREATE TABLE IF NOT EXISTS node_metrics (
    id TEXT PRIMARY KEY,
    node_id TEXT NOT NULL,
    cpu_percent REAL,
    memory_used_mb INTEGER,
    memory_total_mb INTEGER,
    disk_used_gb INTEGER,
    disk_total_gb INTEGER,
    timestamp TEXT NOT NULL,
    FOREIGN KEY(node_id) REFERENCES nodes(id) ON DELETE CASCADE
);

-- Create index for node metrics queries
CREATE INDEX IF NOT EXISTS idx_node_metrics_node_id ON node_metrics(node_id);
CREATE INDEX IF NOT EXISTS idx_node_metrics_timestamp ON node_metrics(timestamp);

-- VM boot logs table
CREATE TABLE IF NOT EXISTS vm_boot_logs (
    id TEXT PRIMARY KEY,
    vm_id TEXT NOT NULL,
    line_number INTEGER NOT NULL,
    content TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    FOREIGN KEY(vm_id) REFERENCES virtual_machines(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_vm_boot_logs_vm_id ON vm_boot_logs(vm_id);
CREATE INDEX IF NOT EXISTS idx_vm_boot_logs_timestamp ON vm_boot_logs(timestamp);

-- Roles table for RBAC
CREATE TABLE IF NOT EXISTS roles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    permissions TEXT NOT NULL,
    created_at TEXT NOT NULL
);

-- Insert default roles
INSERT OR IGNORE INTO roles (id, name, permissions, created_at) VALUES
('role-admin', 'admin', '[{"resource": "*", "action": "*}]', datetime('now')),
('role-operator', 'operator', '[{"resource": "vms", "action": "*"}, {"resource": "images", "action": "*"}, {"resource": "networks", "action": "*"}, {"resource": "storage-pools", "action": "*"}, {"resource": "nodes", "action": "read"}]', datetime('now')),
('role-viewer', 'viewer', '[{"resource": "*", "action": "read"}]', datetime('now'));

-- Audit logs table
CREATE TABLE IF NOT EXISTS audit_logs (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    user_name TEXT NOT NULL,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT,
    details TEXT,
    ip_address TEXT,
    success INTEGER NOT NULL,
    error TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_logs_user ON audit_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_resource ON audit_logs(resource_type, resource_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_created ON audit_logs(created_at);

-- VM Templates table for rapid VM provisioning
CREATE TABLE IF NOT EXISTS vm_templates (
    id TEXT PRIMARY KEY,
    node_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    vcpu INTEGER NOT NULL,
    memory_mb INTEGER NOT NULL,
    image_id TEXT NOT NULL,
    network_id TEXT NOT NULL,
    storage_pool_id TEXT NOT NULL,
    cloud_init_config TEXT,
    tags TEXT, -- JSON array
    created_at TEXT NOT NULL,
    FOREIGN KEY(node_id) REFERENCES nodes(id) ON DELETE CASCADE,
    FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE RESTRICT,
    FOREIGN KEY(network_id) REFERENCES networks(id) ON DELETE RESTRICT,
    FOREIGN KEY(storage_pool_id) REFERENCES storage_pools(id) ON DELETE RESTRICT,
    UNIQUE(node_id, name)
);

CREATE INDEX IF NOT EXISTS idx_vm_templates_node_id ON vm_templates(node_id);

-- Cloud-init Templates table for reusable cloud-init configurations
CREATE TABLE IF NOT EXISTS cloud_init_templates (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    content TEXT NOT NULL,
    variables TEXT, -- JSON array of variable names
    created_at TEXT NOT NULL
);

-- Backup jobs for scheduled VM backups
CREATE TABLE IF NOT EXISTS backup_jobs (
    id TEXT PRIMARY KEY,
    vm_id TEXT NOT NULL,
    name TEXT NOT NULL,
    schedule TEXT NOT NULL,
    retention INTEGER DEFAULT 7,
    destination TEXT NOT NULL,
    enabled INTEGER DEFAULT 1,
    last_run TEXT,
    next_run TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY(vm_id) REFERENCES virtual_machines(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_backup_jobs_vm_id ON backup_jobs(vm_id);

-- Backup history for tracking all backup operations
CREATE TABLE IF NOT EXISTS backup_history (
    id TEXT PRIMARY KEY,
    job_id TEXT,
    vm_id TEXT NOT NULL,
    snapshot_id TEXT NOT NULL,
    status TEXT NOT NULL,
    size_bytes INTEGER,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    error TEXT,
    FOREIGN KEY(job_id) REFERENCES backup_jobs(id) ON DELETE SET NULL,
    FOREIGN KEY(vm_id) REFERENCES virtual_machines(id) ON DELETE CASCADE,
    FOREIGN KEY(snapshot_id) REFERENCES vm_snapshots(id)
);

CREATE INDEX IF NOT EXISTS idx_backup_history_vm ON backup_history(vm_id, started_at);
CREATE INDEX IF NOT EXISTS idx_backup_history_job ON backup_history(job_id);

-- VLAN networks table
CREATE TABLE IF NOT EXISTS vlan_networks (
    id TEXT PRIMARY KEY,
    network_id TEXT NOT NULL,
    vlan_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    cidr TEXT NOT NULL,
    gateway_ip TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(network_id) REFERENCES networks(id) ON DELETE CASCADE,
    UNIQUE(network_id, vlan_id)
);

CREATE INDEX IF NOT EXISTS idx_vlan_networks_network ON vlan_networks(network_id);

-- DHCP servers table
CREATE TABLE IF NOT EXISTS dhcp_servers (
    id TEXT PRIMARY KEY,
    network_id TEXT NOT NULL,
    range_start TEXT NOT NULL,
    range_end TEXT NOT NULL,
    lease_time_seconds INTEGER DEFAULT 3600,
    is_running INTEGER DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(network_id) REFERENCES networks(id) ON DELETE CASCADE,
    UNIQUE(network_id)
);

CREATE INDEX IF NOT EXISTS idx_dhcp_servers_network ON dhcp_servers(network_id);

-- DHCP leases table
CREATE TABLE IF NOT EXISTS dhcp_leases (
    id TEXT PRIMARY KEY,
    network_id TEXT NOT NULL,
    mac_address TEXT NOT NULL,
    ip_address TEXT NOT NULL,
    hostname TEXT,
    lease_start TEXT NOT NULL,
    lease_end TEXT NOT NULL,
    FOREIGN KEY(network_id) REFERENCES networks(id) ON DELETE CASCADE,
    UNIQUE(network_id, mac_address),
    UNIQUE(network_id, ip_address)
);

CREATE INDEX IF NOT EXISTS idx_dhcp_leases_network ON dhcp_leases(network_id);
CREATE INDEX IF NOT EXISTS idx_dhcp_leases_mac ON dhcp_leases(network_id, mac_address);

-- Firewall rules table
CREATE TABLE IF NOT EXISTS firewall_rules (
    id TEXT PRIMARY KEY,
    vm_id TEXT NOT NULL,
    direction TEXT NOT NULL,
    protocol TEXT NOT NULL,
    port_range TEXT,
    source_cidr TEXT NOT NULL,
    action TEXT NOT NULL,
    priority INTEGER NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY(vm_id) REFERENCES virtual_machines(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_firewall_rules_vm ON firewall_rules(vm_id, priority);
CREATE INDEX IF NOT EXISTS idx_firewall_rules_direction ON firewall_rules(vm_id, direction);

-- Quotas table for resource limits per user
CREATE TABLE IF NOT EXISTS quotas (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL UNIQUE,
    max_vms INTEGER DEFAULT 10,
    max_cpu INTEGER DEFAULT 20,
    max_memory_gb INTEGER DEFAULT 64,
    max_storage_gb INTEGER DEFAULT 500,
    max_networks INTEGER DEFAULT 5,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_quotas_user_id ON quotas(user_id);

-- Usage cache table for tracking resource consumption per user
CREATE TABLE IF NOT EXISTS usage_cache (
    user_id TEXT PRIMARY KEY,
    vms INTEGER DEFAULT 0,
    cpus INTEGER DEFAULT 0,
    memory_gb INTEGER DEFAULT 0,
    storage_gb INTEGER DEFAULT 0,
    networks INTEGER DEFAULT 0,
    last_updated TEXT NOT NULL,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_usage_cache_user_id ON usage_cache(user_id);

-- Insert default cloud-init templates
INSERT OR IGNORE INTO cloud_init_templates (id, name, description, content, variables, created_at) VALUES
('cit-basic', 'Basic User Setup', 'Creates a user with sudo access and SSH key', '#cloud-config
hostname: {{.Hostname}}
manage_etc_hosts: true
users:
  - name: {{.Username}}
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - {{.SSHKey}}
chpasswd:
  list: |
    {{.Username}}:{{.Password}}
  expire: False
package_update: true
packages:
  - qemu-guest-agent', '["Hostname", "Username", "SSHKey", "Password"]', datetime('now')),
('cit-docker', 'Docker Ready', 'Ubuntu with Docker pre-installed', '#cloud-config
package_update: true
packages:
  - docker.io
  - qemu-guest-agent
users:
  - name: {{.Username}}
    groups: docker
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - {{.SSHKey}}
runcmd:
  - systemctl enable docker
  - systemctl start docker', '["Username", "SSHKey"]', datetime('now')),
('cit-kubernetes', 'Kubernetes Node', 'Ubuntu with containerd and Kubernetes tools', '#cloud-config
package_update: true
packages:
  - apt-transport-https
  - ca-certificates
  - curl
  - gnupg
  - lsb-release
  - qemu-guest-agent
users:
  - name: {{.Username}}
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - {{.SSHKey}}
runcmd:
  - sysctl -w net.ipv4.ip_forward=1
  - echo "net.ipv4.ip_forward=1" >> /etc/sysctl.conf', '["Username", "SSHKey"]', datetime('now'));
