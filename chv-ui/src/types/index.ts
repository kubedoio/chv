// CHV API Types

export interface Token {
  id: string
  token: string
  name: string
  created_at: string
  expires_at: string
}

export type VMState = 'provisioning' | 'starting' | 'running' | 'stopping' | 'stopped' | 'error' | 'deleting'
export type VMDesiredState = 'running' | 'stopped' | 'deleted'

export interface VM {
  id: string
  name: string
  desired_state: VMDesiredState
  actual_state: VMState
  node_id?: string
  created_at: string
  updated_at: string
  last_error?: string
  spec?: VMSpec
}

export interface VMSpec {
  cpu: number
  memory_mb: number
  disks: Disk[]
  networks: NetworkAttachment[]
  cloud_init?: CloudInitSpec
}

export interface Disk {
  volume_id: string
  bus: string
  boot: boolean
}

export interface NetworkAttachment {
  network_id: string
  dhcp: boolean
}

export interface CloudInitSpec {
  user_data?: string
  meta_data?: string
  network_config?: string
}

export type NodeState = 'online' | 'offline' | 'maintenance'

export interface Node {
  id: string
  hostname: string
  management_ip: string
  state: NodeState
  maintenance_mode: boolean
  total_cpu_cores: number
  total_ram_mb: number
  allocatable_cpu_cores: number
  allocatable_ram_mb: number
  labels?: Record<string, string>
  capabilities?: Record<string, string>
  agent_version?: string
  hypervisor_version?: string
  last_heartbeat_at?: string
  created_at: string
  updated_at: string
}

export interface Network {
  id: string
  name: string
  bridge_name: string
  cidr: string
  gateway_ip: string
  created_at: string
}

export interface StoragePool {
  id: string
  name: string
  pool_type: 'local' | 'nfs'
  path_or_export: string
  node_id?: string
  total_bytes: number
  used_bytes: number
  free_bytes: number
  supports_online_resize: boolean
  created_at: string
}

export interface Image {
  id: string
  name: string
  os_family: string
  status: 'importing' | 'ready' | 'failed'
  architecture: string
  size_bytes: number
  created_at: string
}

export interface VMCreateRequest {
  name: string
  cpu: number
  memory_mb: number
  image_id: string
  disk_size_bytes: number
  networks: { network_id: string }[]
  cloud_init?: CloudInitSpec
}

export interface APIError {
  error: {
    code: string
    message: string
    details?: Record<string, unknown>
  }
}

export interface DashboardStats {
  total_vms: number
  running_vms: number
  stopped_vms: number
  total_nodes: number
  online_nodes: number
  total_storage_pools: number
  total_networks: number
}

export interface CreateNetworkRequest {
  name: string
  bridge_name: string
  cidr: string
  gateway_ip: string
}

export interface CreateStoragePoolRequest {
  name: string
  pool_type: 'local' | 'nfs'
  path_or_export: string
  supports_online_resize: boolean
}

export interface RegisterNodeRequest {
  hostname: string
  management_ip: string
  total_cpu_cores: number
  total_ram_mb: number
}

export interface ImportImageRequest {
  name: string
  os_family: string
  source_url: string
  source_format: 'qcow2' | 'raw' | 'vmdk'
  architecture: string
  cloud_init_supported: boolean
}
