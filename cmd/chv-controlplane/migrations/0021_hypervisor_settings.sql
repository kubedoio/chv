-- Hypervisor Settings: global defaults, profiles, and per-VM overrides

CREATE TABLE hypervisor_profiles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    cpu_nested BOOLEAN,
    cpu_amx BOOLEAN,
    cpu_kvm_hyperv BOOLEAN,
    memory_mergeable BOOLEAN,
    memory_hugepages BOOLEAN,
    memory_shared BOOLEAN,
    memory_prefault BOOLEAN,
    iommu BOOLEAN,
    rng_src TEXT,
    watchdog BOOLEAN,
    landlock_enable BOOLEAN,
    serial_mode TEXT,
    console_mode TEXT,
    pvpanic BOOLEAN,
    tpm_type TEXT,
    tpm_socket_path TEXT,
    is_builtin BOOLEAN NOT NULL DEFAULT false,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Built-in profiles
INSERT INTO hypervisor_profiles (
    id, name, description,
    cpu_nested, cpu_amx, cpu_kvm_hyperv,
    memory_mergeable, memory_hugepages, memory_shared, memory_prefault,
    iommu, rng_src, watchdog, landlock_enable,
    serial_mode, console_mode, pvpanic,
    tpm_type, tpm_socket_path,
    is_builtin
) VALUES (
    'balanced', 'Balanced', 'Safe defaults for general use.',
    true, false, false,
    false, false, false, false,
    false, '/dev/urandom', false, false,
    'Pty', 'Off', false,
    NULL, NULL,
    true
);

INSERT INTO hypervisor_profiles (
    id, name, description,
    cpu_nested, cpu_amx, cpu_kvm_hyperv,
    memory_mergeable, memory_hugepages, memory_shared, memory_prefault,
    iommu, rng_src, watchdog, landlock_enable,
    serial_mode, console_mode, pvpanic,
    tpm_type, tpm_socket_path,
    is_builtin
) VALUES (
    'performance', 'Performance', 'Optimized for compute workloads.',
    true, true, false,
    false, true, false, false,
    false, '/dev/urandom', false, false,
    'Pty', 'Off', false,
    NULL, NULL,
    true
);

INSERT INTO hypervisor_profiles (
    id, name, description,
    cpu_nested, cpu_amx, cpu_kvm_hyperv,
    memory_mergeable, memory_hugepages, memory_shared, memory_prefault,
    iommu, rng_src, watchdog, landlock_enable,
    serial_mode, console_mode, pvpanic,
    tpm_type, tpm_socket_path,
    is_builtin
) VALUES (
    'security-hardened', 'Security Hardened', 'Maximum isolation and auditing.',
    false, false, false,
    false, false, false, false,
    true, '/dev/urandom', true, true,
    'Pty', 'Off', false,
    'swtpm', NULL,
    true
);

CREATE TABLE hypervisor_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    cpu_nested BOOLEAN NOT NULL DEFAULT true,
    cpu_amx BOOLEAN NOT NULL DEFAULT false,
    cpu_kvm_hyperv BOOLEAN NOT NULL DEFAULT false,
    memory_mergeable BOOLEAN NOT NULL DEFAULT false,
    memory_hugepages BOOLEAN NOT NULL DEFAULT false,
    memory_shared BOOLEAN NOT NULL DEFAULT false,
    memory_prefault BOOLEAN NOT NULL DEFAULT false,
    iommu BOOLEAN NOT NULL DEFAULT false,
    rng_src TEXT NOT NULL DEFAULT '/dev/urandom',
    watchdog BOOLEAN NOT NULL DEFAULT false,
    landlock_enable BOOLEAN NOT NULL DEFAULT false,
    serial_mode TEXT NOT NULL DEFAULT 'Pty',
    console_mode TEXT NOT NULL DEFAULT 'Off',
    pvpanic BOOLEAN NOT NULL DEFAULT false,
    tpm_type TEXT,
    tpm_socket_path TEXT,
    profile_id TEXT REFERENCES hypervisor_profiles(id),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Seed the singleton row with defaults
INSERT INTO hypervisor_settings (id) VALUES (1);

-- Per-VM override columns
ALTER TABLE vms ADD COLUMN hv_cpu_nested BOOLEAN;
ALTER TABLE vms ADD COLUMN hv_cpu_amx BOOLEAN;
ALTER TABLE vms ADD COLUMN hv_cpu_kvm_hyperv BOOLEAN;
ALTER TABLE vms ADD COLUMN hv_memory_mergeable BOOLEAN;
ALTER TABLE vms ADD COLUMN hv_memory_hugepages BOOLEAN;
ALTER TABLE vms ADD COLUMN hv_memory_shared BOOLEAN;
ALTER TABLE vms ADD COLUMN hv_memory_prefault BOOLEAN;
ALTER TABLE vms ADD COLUMN hv_iommu BOOLEAN;
ALTER TABLE vms ADD COLUMN hv_rng_src TEXT;
ALTER TABLE vms ADD COLUMN hv_watchdog BOOLEAN;
ALTER TABLE vms ADD COLUMN hv_landlock_enable BOOLEAN;
ALTER TABLE vms ADD COLUMN hv_serial_mode TEXT;
ALTER TABLE vms ADD COLUMN hv_console_mode TEXT;
ALTER TABLE vms ADD COLUMN hv_pvpanic BOOLEAN;
ALTER TABLE vms ADD COLUMN hv_tpm_type TEXT;
ALTER TABLE vms ADD COLUMN hv_tpm_socket_path TEXT;
