CREATE TABLE host_identity (
    singleton_key INTEGER PRIMARY KEY NOT NULL DEFAULT 1 CHECK (singleton_key = 1),
    host_id TEXT NOT NULL UNIQUE CHECK (length(trim(host_id)) > 0),
    capabilities_json TEXT NOT NULL CHECK (json_valid(capabilities_json)),
    resource_version INTEGER NOT NULL CHECK (resource_version >= 1),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE TABLE vms (
    vm_id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(vm_id)) > 0),
    definition_json TEXT NOT NULL CHECK (json_valid(definition_json)),
    requested_power_state TEXT NOT NULL CHECK (requested_power_state IN ('running', 'stopped')),
    observed_power_state TEXT NOT NULL CHECK (observed_power_state IN ('unknown', 'created', 'running', 'stopped', 'paused', 'failed')),
    resource_version INTEGER NOT NULL CHECK (resource_version >= 1),
    deleted_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE TABLE attachments (
    attachment_id TEXT NOT NULL,
    vm_id TEXT NOT NULL REFERENCES vms(vm_id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('network', 'storage')),
    provider_ref TEXT NOT NULL,
    requested_state TEXT NOT NULL,
    observed_state TEXT NOT NULL,
    resource_version INTEGER NOT NULL CHECK (resource_version >= 1),
    PRIMARY KEY (vm_id, attachment_id),
    UNIQUE (vm_id, kind, provider_ref)
) STRICT;

CREATE TABLE operations (
    operation_id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(operation_id)) > 0),
    kind TEXT NOT NULL CHECK (kind IN ('create_vm', 'update_vm', 'delete_vm', 'start_vm', 'stop_vm', 'reboot_vm', 'attach_volume', 'detach_volume', 'attach_network', 'detach_network')),
    vm_id TEXT NOT NULL REFERENCES vms(vm_id) ON DELETE RESTRICT,
    request_fingerprint TEXT NOT NULL CHECK (length(trim(request_fingerprint)) > 0),
    request_json TEXT NOT NULL CHECK (json_valid(request_json)),
    result_json TEXT CHECK (result_json IS NULL OR json_valid(result_json)),
    error_json TEXT CHECK (error_json IS NULL OR json_valid(error_json)),
    status TEXT NOT NULL CHECK (status IN ('accepted', 'running', 'succeeded', 'failed', 'unsupported')),
    retry_count INTEGER NOT NULL DEFAULT 0 CHECK (retry_count >= 0),
    max_retries INTEGER NOT NULL DEFAULT 0 CHECK (max_retries >= 0),
    accepted_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    completed_at TEXT,
    CHECK (retry_count <= max_retries)
) STRICT;

CREATE TABLE operation_steps (
    operation_id TEXT NOT NULL REFERENCES operations(operation_id) ON DELETE CASCADE,
    step_index INTEGER NOT NULL CHECK (step_index >= 0),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    status TEXT NOT NULL CHECK (status IN ('pending','running','succeeded','failed','skipped')),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    last_error TEXT,
    PRIMARY KEY (operation_id, step_index)
) STRICT;

CREATE TABLE idempotency_keys (
    scope TEXT NOT NULL CHECK (length(trim(scope)) > 0),
    idempotency_key TEXT NOT NULL CHECK (length(trim(idempotency_key)) > 0),
    request_fingerprint TEXT NOT NULL,
    operation_id TEXT NOT NULL UNIQUE REFERENCES operations(operation_id) ON DELETE RESTRICT,
    accepted_resource_version INTEGER NOT NULL CHECK (accepted_resource_version >= 1),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (scope, idempotency_key)
) STRICT;

CREATE TABLE events (
    event_id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(event_id)) > 0),
    sequence INTEGER NOT NULL UNIQUE CHECK (sequence >= 1),
    operation_id TEXT REFERENCES operations(operation_id) ON DELETE SET NULL,
    vm_id TEXT REFERENCES vms(vm_id) ON DELETE SET NULL,
    kind TEXT NOT NULL,
    payload_json TEXT NOT NULL CHECK (json_valid(payload_json)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE TABLE ownership_markers (
    vm_id TEXT PRIMARY KEY NOT NULL REFERENCES vms(vm_id) ON DELETE CASCADE,
    owner_id TEXT NOT NULL CHECK (length(trim(owner_id)) > 0),
    ownership TEXT NOT NULL CHECK (ownership IN ('cell_hv','external','unclaimed')),
    recovery TEXT NOT NULL CHECK (recovery IN ('owned_recoverable','owned_inconsistent','foreign','ambiguous','missing')),
    marker_version INTEGER NOT NULL CHECK (marker_version >= 1)
) STRICT;

CREATE TABLE migration_state (
    source TEXT PRIMARY KEY NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('pending', 'imported', 'cutover')),
    source_checksum TEXT NOT NULL,
    imported_host_id TEXT,
    imported_vm_ids_json TEXT CHECK (imported_vm_ids_json IS NULL OR json_valid(imported_vm_ids_json)),
    imported_at TEXT,
    cutover_at TEXT,
    CHECK (length(trim(source)) > 0),
    CHECK (length(trim(source_checksum)) > 0),
    CHECK (state = 'pending' OR (imported_at IS NOT NULL AND imported_host_id IS NOT NULL AND imported_vm_ids_json IS NOT NULL)),
    CHECK (cutover_at IS NULL OR state = 'cutover')
) STRICT;

CREATE INDEX operations_vm_id_idx ON operations(vm_id);
CREATE INDEX events_operation_id_idx ON events(operation_id);
CREATE INDEX events_vm_id_idx ON events(vm_id);
