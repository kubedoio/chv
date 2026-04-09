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
  created_at: string;
}

export interface CreateNetworkInput {
  name: string;
  mode: 'bridge';
  bridge_name: string;
  cidr: string;
  gateway_ip: string;
}

export interface StoragePool {
  id: string;
  name: string;
  pool_type: string;
  path: string;
  is_default: boolean;
  status: string;
  capacity_bytes?: number;
  allocatable_bytes?: number;
  created_at: string;
}

export interface CreateStoragePoolInput {
  name: string;
  pool_type: 'localdisk';
  path: string;
  capacity_bytes?: number;
  allocatable_bytes?: number;
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

export interface ImportProgress {
  image_id: string;
  status: 'pending' | 'downloading' | 'validating' | 'ready' | 'failed';
  progress_percent: number;
  bytes_downloaded: number;
  total_bytes: number;
  speed: string;
  error?: string;
  updated_at: string;
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
  user_data?: string;
  meta_data?: string;
  network_config?: string;
}

export interface CreateVMInput {
  name: string;
  image_id: string;
  storage_pool_id: string;
  network_id: string;
  vcpu: number;
  memory_mb: number;
  user_data?: string;
  username?: string;
  ssh_authorized_keys?: string[];
}

export interface Operation {
  id: string;
  resource_type: string;
  resource_id: string;
  operation_type: string;
  state: string;
  created_at: string;
}

export interface Event {
  id: string;
  timestamp: string;
  operation: string;
  status: 'pending' | 'success' | 'failed';
  resource: string;
  resource_id?: string;
  message?: string;
  details?: Record<string, any>;
}

export interface LoginResponse {
  user: {
    id: string;
    username: string;
    email?: string;
    role: string;
    is_active: boolean;
  };
  token: string;
  token_type: string;
  expires_in: number;
}

export interface UserInfo {
  id: string;
  username: string;
  email?: string;
  role: string;
  is_active: boolean;
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

export interface VMMetricsResponse {
  id: string;
  current: VMMetrics | null;
  history: VMMetrics[];
}

export interface BulkVMRequest {
  ids: string[];
}

export interface BulkVMResponse {
  results: Record<string, string>;
}

export interface VMMetrics {
  cpu: {
    usage_percent: number;
    vcpus: number;
  };
  memory: {
    total_mb: number;
    used_mb: number;
    free_mb: number;
    usage_percent: number;
  };
  disk: {
    read_bytes: number;
    write_bytes: number;
    read_ops: number;
    write_ops: number;
  };
  network: {
    rx_bytes: number;
    tx_bytes: number;
    rx_packets: number;
    tx_packets: number;
  };
  uptime: string;
}

export interface VMSnapshot {
  id: string;
  vm_id: string;
  name: string;
  created_at: string;
  status: string;
}

