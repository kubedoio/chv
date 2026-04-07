export interface InstallStatusResponse {
  overall_state: string;
  data_root: string;
  database_path: string;
  bridge: {
    name: string;
    exists: boolean;
    expected_ip: string;
    actual_ip: string;
    up: boolean;
  };
  localdisk: {
    path: string;
    ready: boolean;
  };
  cloud_hypervisor: {
    path: string;
    found: boolean;
  };
  cloudinit: {
    supported: boolean;
  };
  checks: string[];
  warnings: string[];
  errors: string[];
}

export interface InstallActionResponse {
  overall_state: string;
  actions_taken: string[];
  warnings: string[];
  errors: string[];
}

export interface Network {
  id: string;
  name: string;
  mode: string;
  bridge_name: string;
  cidr: string;
  gateway_ip: string;
  is_system_managed: boolean;
  status: string;
}

export interface StoragePool {
  id: string;
  name: string;
  pool_type: string;
  path: string;
  is_default: boolean;
  status: string;
}

export interface Image {
  id: string;
  name: string;
  os_family: string;
  architecture: string;
  format: string;
  source_url: string;
  checksum?: string;
  local_path: string;
  cloud_init_supported: boolean;
  status: string;
}

export interface VM {
  id: string;
  name: string;
  image_id: string;
  storage_pool_id: string;
  network_id: string;
  desired_state: string;
  actual_state: string;
  vcpu: number;
  memory_mb: number;
  disk_path: string;
  seed_iso_path: string;
  workspace_path: string;
  ip_address?: string;
  mac_address?: string;
  last_error?: string;
}

export interface Operation {
  id: string;
  resource_type: string;
  resource_id: string;
  operation_type: string;
  state: string;
  created_at: string;
}

export interface APIErrorEnvelope {
  error: {
    code: string;
    message: string;
    resource_type?: string;
    resource_id?: string;
    retryable: boolean;
    hint?: string;
  };
}

