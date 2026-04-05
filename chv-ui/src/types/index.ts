// CHV API Type Definitions

// Common
export interface APIError {
  code: string;
  message: string;
  resource_type?: string;
  resource_id?: string;
  retryable?: boolean;
  hint?: string;
}

export interface APIResponse<T> {
  data?: T;
  error?: APIError;
}

export interface ListResponse<T> {
  items: T[];
  total?: number;
}

// Auth
export interface Token {
  id: string;
  name: string;
  token: string;
  created_at: string;
  expires_at: string;
}

export interface TokenCreateRequest {
  name: string;
  expires_in?: string;
}

// Nodes
export type NodeState = 'online' | 'degraded' | 'offline' | 'maintenance';

export interface Node {
  id: string;
  hostname: string;
  management_ip: string;
  status: NodeState;
  total_cpu_cores: number;
  total_ram_mb: number;
  allocatable_cpu_cores: number;
  allocatable_ram_mb: number;
  agent_version?: string;
  hypervisor_version?: string;
  last_heartbeat?: string;
  maintenance_mode: boolean;
  capabilities?: string[];
  created_at: string;
  updated_at: string;
}

export interface NodeCreateRequest {
  hostname: string;
  management_ip: string;
  total_cpu_cores: number;
  total_ram_mb: number;
}

// Networks
export type NetworkMode = 'bridge' | 'vxlan' | 'macvlan';
export type NetworkStatus = 'active' | 'inactive' | 'error';

export interface Network {
  id: string;
  name: string;
  bridge_name: string;
  cidr: string;
  gateway_ip: string;
  dns_servers: string[];
  mtu: number;
  mode: NetworkMode;
  status: NetworkStatus;
  created_at: string;
}

export interface NetworkCreateRequest {
  name: string;
  bridge_name: string;
  cidr: string;
  gateway_ip: string;
  dns_servers?: string[];
  mtu?: number;
}

// Storage Pools
export type PoolType = 'local' | 'nfs';
export type PoolStatus = 'active' | 'inactive' | 'error' | 'full';

export interface StoragePool {
  id: string;
  name: string;
  node_id: string;
  pool_type: PoolType;
  path_or_export: string;
  capacity_bytes?: number;
  allocatable_bytes?: number;
  status: PoolStatus;
  supports_online_resize: boolean;
  supports_clone: boolean;
  supports_snapshot: boolean;
  created_at: string;
}

export interface StoragePoolCreateRequest {
  name: string;
  pool_type: PoolType;
  node_id: string;
  path_or_export: string;
}

// Images
export type ImageFormat = 'qcow2' | 'raw';
export type ImageStatus = 'importing' | 'normalizing' | 'ready' | 'error';

export interface Image {
  id: string;
  name: string;
  os_family: string;
  source_format: ImageFormat;
  normalized_format: ImageFormat;
  architecture: string;
  cloud_init_supported: boolean;
  default_username?: string;
  checksum?: string;
  status: ImageStatus;
  metadata?: Record<string, unknown>;
  created_at: string;
}

export interface ImageImportRequest {
  name: string;
  os_family: string;
  source_url: string;
  source_format?: ImageFormat;
  architecture?: string;
  checksum?: string;
  cloud_init_supported?: boolean;
  default_username?: string;
}

// VMs
export type VMDesiredState = 'present' | 'running' | 'stopped' | 'deleted';
export type VMActualState = 'provisioning' | 'starting' | 'running' | 'stopping' | 'stopped' | 'deleting' | 'error' | 'unknown';
export type PlacementStatus = 'pending' | 'placed' | 'failed';

export interface VM {
  id: string;
  name: string;
  node_id?: string;
  desired_state: VMDesiredState;
  actual_state: VMActualState;
  placement_status: PlacementStatus;
  spec: VMSpec;
  last_error?: string;
  created_at: string;
  updated_at: string;
}

export interface VMSpec {
  cpu: number;
  memory_mb: number;
  boot: BootSpec;
  disks: DiskAttachment[];
  networks: NetworkAttachment[];
  cloud_init?: CloudInitSpec;
}

export interface BootSpec {
  mode: 'cloud_image' | 'uefi' | 'direct_kernel';
  kernel_path?: string;
  initrd_path?: string;
  cmdline?: string;
  firmware_path?: string;
}

export interface DiskAttachment {
  volume_id: string;
  bus: string;
  boot: boolean;
}

export interface NetworkAttachment {
  network_id: string;
  mac_address?: string;
  ip_address?: string;
  dhcp: boolean;
}

export interface CloudInitSpec {
  user_data?: string;
  meta_data?: string;
  network_config?: string;
}

export interface VMCreateRequest {
  name: string;
  vcpu: number;
  memory_mb: number;
  disk_size_bytes: number;
  image_id: string;
  networks: { network_id: string }[];
  cloud_init?: CloudInitSpec;
}

// Operations
export type OperationType = 'vm_create' | 'vm_start' | 'vm_stop' | 'vm_reboot' | 'vm_delete' | 'image_import' | 'node_register';
export type OperationCategory = 'sync' | 'async';
export type OperationStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';
export type ActorType = 'user' | 'system' | 'scheduler' | 'reconciler';

export interface Operation {
  id: string;
  type: OperationType;
  category: OperationCategory;
  status: OperationStatus;
  status_message: string;
  resource_type?: string;
  resource_id?: string;
  actor_type: ActorType;
  actor_id: string;
  progress_percent: number;
  progress_message?: string;
  request_payload?: unknown;
  result_payload?: unknown;
  error_details?: unknown;
  started_at?: string;
  completed_at?: string;
  created_at: string;
  updated_at: string;
}

// Dashboard
export interface DashboardStats {
  nodes: {
    total: number;
    online: number;
    degraded: number;
    offline: number;
    maintenance: number;
  };
  networks: number;
  storage_pools: number;
  images: number;
  vms: {
    total: number;
    by_state: Record<VMActualState, number>;
  };
}
