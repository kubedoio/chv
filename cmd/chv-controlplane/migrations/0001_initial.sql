CREATE TABLE IF NOT EXISTS nodes (
    node_id text PRIMARY KEY,
    hostname text NOT NULL,
    display_name text NOT NULL,
    enrollment_token_id text,
    certificate_serial text,
    agent_version text,
    control_plane_version text,
    enrolled_at text,
    last_seen_at text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE TABLE IF NOT EXISTS node_versions (
    node_version_id text PRIMARY KEY DEFAULT (lower(hex(randomblob(4)))||'-'||lower(hex(randomblob(2)))||'-4'||substr(lower(hex(randomblob(2))),2)||'-'||substr('89ab',abs(random())%4+1,1)||substr(lower(hex(randomblob(2))),2)||'-'||lower(hex(randomblob(6)))),
    node_id text NOT NULL REFERENCES nodes (node_id) ON DELETE CASCADE,
    component_name text NOT NULL,
    version text NOT NULL,
    source text,
    reported_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS node_versions_node_component_reported_at_idx
    ON node_versions (node_id, component_name, reported_at DESC);

CREATE TABLE IF NOT EXISTS node_inventory (
    node_id text PRIMARY KEY REFERENCES nodes (node_id) ON DELETE CASCADE,
    architecture text NOT NULL,
    kernel_version text,
    os_release text,
    cpu_count integer NOT NULL,
    memory_bytes integer NOT NULL,
    disk_bytes integer,
    cloud_hypervisor_version text,
    chv_agent_version text,
    chv_stord_version text,
    chv_nwd_version text,
    host_bundle_version text,
    inventory_status text,
    last_reported_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE TABLE IF NOT EXISTS node_desired_state (
    node_id text PRIMARY KEY REFERENCES nodes (node_id) ON DELETE CASCADE,
    desired_generation integer NOT NULL,
    desired_state text NOT NULL,
    requested_by text,
    updated_by text,
    state_reason text,
    requested_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS node_desired_state_state_idx
    ON node_desired_state (desired_state);

CREATE TABLE IF NOT EXISTS node_observed_state (
    node_id text PRIMARY KEY REFERENCES nodes (node_id) ON DELETE CASCADE,
    observed_generation integer NOT NULL,
    observed_state text NOT NULL,
    health_status text,
    runtime_status text,
    state_reason text,
    entered_at text,
    observed_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS node_observed_state_state_idx
    ON node_observed_state (observed_state);

CREATE TABLE IF NOT EXISTS vms (
    vm_id text PRIMARY KEY,
    node_id text REFERENCES nodes (node_id) ON DELETE SET NULL,
    display_name text NOT NULL,
    tenant_id text,
    placement_policy text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE TABLE IF NOT EXISTS vm_desired_state (
    vm_id text PRIMARY KEY REFERENCES vms (vm_id) ON DELETE CASCADE,
    desired_generation integer NOT NULL,
    desired_status text,
    requested_by text,
    updated_by text,
    target_node_id text REFERENCES nodes (node_id) ON DELETE SET NULL,
    cpu_count integer,
    memory_bytes integer,
    image_ref text,
    boot_mode text,
    desired_power_state text,
    requested_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS vm_desired_state_status_idx
    ON vm_desired_state (desired_status);

CREATE TABLE IF NOT EXISTS vm_observed_state (
    vm_id text PRIMARY KEY REFERENCES vms (vm_id) ON DELETE CASCADE,
    observed_generation integer NOT NULL,
    runtime_status text NOT NULL,
    health_status text,
    node_id text REFERENCES nodes (node_id) ON DELETE SET NULL,
    cloud_hypervisor_pid integer,
    api_socket_path text,
    last_transition_at text,
    last_error text,
    observed_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS vm_observed_state_runtime_status_idx
    ON vm_observed_state (runtime_status);

CREATE TABLE IF NOT EXISTS volumes (
    volume_id text PRIMARY KEY,
    node_id text REFERENCES nodes (node_id) ON DELETE SET NULL,
    display_name text NOT NULL,
    capacity_bytes integer NOT NULL,
    volume_kind text,
    storage_class text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE TABLE IF NOT EXISTS volume_desired_state (
    volume_id text PRIMARY KEY REFERENCES volumes (volume_id) ON DELETE CASCADE,
    desired_generation integer NOT NULL,
    desired_status text,
    requested_by text,
    updated_by text,
    attached_vm_id text REFERENCES vms (vm_id) ON DELETE SET NULL,
    attachment_mode text,
    device_name text,
    read_only integer NOT NULL DEFAULT 0,
    requested_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS volume_desired_state_status_idx
    ON volume_desired_state (desired_status);

CREATE TABLE IF NOT EXISTS volume_observed_state (
    volume_id text PRIMARY KEY REFERENCES volumes (volume_id) ON DELETE CASCADE,
    observed_generation integer NOT NULL,
    runtime_status text NOT NULL,
    health_status text,
    attached_vm_id text REFERENCES vms (vm_id) ON DELETE SET NULL,
    device_path text,
    export_path text,
    last_transition_at text,
    observed_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS volume_observed_state_runtime_status_idx
    ON volume_observed_state (runtime_status);

CREATE TABLE IF NOT EXISTS networks (
    network_id text PRIMARY KEY,
    node_id text REFERENCES nodes (node_id) ON DELETE SET NULL,
    display_name text NOT NULL,
    network_class text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE TABLE IF NOT EXISTS network_desired_state (
    network_id text PRIMARY KEY REFERENCES networks (network_id) ON DELETE CASCADE,
    desired_generation integer NOT NULL,
    desired_status text,
    requested_by text,
    updated_by text,
    requested_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS network_desired_state_status_idx
    ON network_desired_state (desired_status);

CREATE TABLE IF NOT EXISTS network_observed_state (
    network_id text PRIMARY KEY REFERENCES networks (network_id) ON DELETE CASCADE,
    observed_generation integer NOT NULL,
    runtime_status text NOT NULL,
    health_status text,
    exposure_status text,
    applied_at text,
    observed_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS network_observed_state_runtime_status_idx
    ON network_observed_state (runtime_status);

CREATE TABLE IF NOT EXISTS operations (
    operation_id text PRIMARY KEY,
    idempotency_key text NOT NULL UNIQUE,
    resource_kind text NOT NULL,
    resource_id text,
    operation_type text NOT NULL,
    status text NOT NULL,
    requested_by text,
    updated_by text,
    desired_generation integer,
    observed_generation integer,
    error_code text,
    error_message text,
    correlation_id text,
    requested_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    started_at text,
    completed_at text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS operations_resource_idx
    ON operations (resource_kind, resource_id);

CREATE INDEX IF NOT EXISTS operations_status_idx
    ON operations (status);

CREATE TABLE IF NOT EXISTS events (
    event_id text PRIMARY KEY DEFAULT (lower(hex(randomblob(4)))||'-'||lower(hex(randomblob(2)))||'-4'||substr(lower(hex(randomblob(2))),2)||'-'||substr('89ab',abs(random())%4+1,1)||substr(lower(hex(randomblob(2))),2)||'-'||lower(hex(randomblob(6)))),
    occurred_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    event_type text NOT NULL,
    severity text NOT NULL,
    resource_kind text,
    resource_id text,
    node_id text REFERENCES nodes (node_id) ON DELETE SET NULL,
    operation_id text REFERENCES operations (operation_id) ON DELETE SET NULL,
    actor_id text,
    requested_by text,
    correlation_id text,
    message text NOT NULL,
    details text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS events_resource_occurred_at_idx
    ON events (resource_kind, resource_id, occurred_at DESC);

CREATE INDEX IF NOT EXISTS events_severity_idx
    ON events (severity);

CREATE TABLE IF NOT EXISTS alerts (
    alert_id text PRIMARY KEY DEFAULT (lower(hex(randomblob(4)))||'-'||lower(hex(randomblob(2)))||'-4'||substr(lower(hex(randomblob(2))),2)||'-'||substr('89ab',abs(random())%4+1,1)||substr(lower(hex(randomblob(2))),2)||'-'||lower(hex(randomblob(6)))),
    alert_type text NOT NULL,
    severity text NOT NULL,
    resource_kind text,
    resource_id text,
    node_id text REFERENCES nodes (node_id) ON DELETE SET NULL,
    status text NOT NULL,
    requested_by text,
    updated_by text,
    operation_id text REFERENCES operations (operation_id) ON DELETE SET NULL,
    message text NOT NULL,
    opened_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    acknowledged_at text,
    resolved_at text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS alerts_status_idx
    ON alerts (status);

CREATE INDEX IF NOT EXISTS alerts_resource_idx
    ON alerts (resource_kind, resource_id);

CREATE TABLE IF NOT EXISTS maintenance_windows (
    maintenance_window_id text PRIMARY KEY DEFAULT (lower(hex(randomblob(4)))||'-'||lower(hex(randomblob(2)))||'-4'||substr(lower(hex(randomblob(2))),2)||'-'||substr('89ab',abs(random())%4+1,1)||substr(lower(hex(randomblob(2))),2)||'-'||lower(hex(randomblob(6)))),
    scope_kind text NOT NULL,
    scope_id text NOT NULL,
    window_status text NOT NULL,
    requested_by text,
    updated_by text,
    reason text NOT NULL,
    starts_at text NOT NULL,
    ends_at text NOT NULL,
    started_at text,
    ended_at text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    CONSTRAINT maintenance_windows_time_range_chk CHECK (ends_at > starts_at)
);

CREATE INDEX IF NOT EXISTS maintenance_windows_scope_idx
    ON maintenance_windows (scope_kind, scope_id, starts_at);

CREATE TABLE IF NOT EXISTS compatibility_matrix (
    compatibility_matrix_id text PRIMARY KEY DEFAULT (lower(hex(randomblob(4)))||'-'||lower(hex(randomblob(2)))||'-4'||substr(lower(hex(randomblob(2))),2)||'-'||substr('89ab',abs(random())%4+1,1)||substr(lower(hex(randomblob(2))),2)||'-'||lower(hex(randomblob(6)))),
    control_plane_version text NOT NULL,
    chv_agent_version text NOT NULL,
    chv_stord_version text NOT NULL,
    chv_nwd_version text NOT NULL,
    cloud_hypervisor_version text NOT NULL,
    host_bundle_version text NOT NULL,
    status text NOT NULL,
    notes text,
    created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE INDEX IF NOT EXISTS compatibility_matrix_status_idx
    ON compatibility_matrix (status);
