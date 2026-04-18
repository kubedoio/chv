export type PageMeta = {
	page: number;
	page_size: number;
	total_items: number;
};

export type FilterMeta = {
	applied: Record<string, string>;
};

export type HealthTile = {
	key: string;
	label: string;
	status: string;
	value: string;
};

export type CapacityTile = {
	key: string;
	label: string;
	used: string;
	total: string;
};

export type RecentTask = {
	task_id: string;
	status: string;
	summary: string;
	resource_kind: string;
	resource_id: string;
	operation: string;
	started_unix_ms: number;
};

export type OverviewResponse = {
	clusters_total?: number;
	clusters_healthy?: number;
	clusters_degraded?: number;
	nodes_total?: number;
	nodes_degraded?: number;
	vms_running?: number;
	vms_total?: number;
	active_tasks?: number;
	unresolved_alerts?: number;
	maintenance_nodes?: number;
	capacity_hotspots?: number;
	cpu_usage_percent?: number;
	memory_usage_percent?: number;
	alerts?: { summary: string; scope: string; severity: string }[];
	recent_tasks?: RecentTask[];
};

export type ListNodesRequest = {
	page: number;
	page_size: number;
	filters: Record<string, string>;
};

export type NodeListItem = {
	node_id: string;
	name: string;
	cluster: string;
	state: string;
	health: string;
	cpu: string;
	memory: string;
	storage: string;
	network: string;
	version: string;
	maintenance: boolean;
	active_tasks: number;
	alerts: number;
};

export type ListNodesResponse = {
	items: NodeListItem[];
	page: PageMeta;
	filters: FilterMeta;
};

export type GetNodeRequest = {
	node_id: string;
};

export type RelatedTask = {
	task_id: string;
	status: string;
	summary: string;
	operation: string;
	started_unix_ms: number;
};

export type NodeSummary = {
	node_id: string;
	name: string;
	cluster: string;
	state: string;
	health: string;
	version: string;
	cpu: string;
	memory: string;
	storage: string;
	network: string;
	uptime?: string;
	last_checkin?: string;
	recent_tasks: RelatedTask[];
};

export type NodeHostedVm = {
	vm_id: string;
	name: string;
	power_state: string;
	health: string;
	cpu: string;
	memory: string;
};

export type NodeConfigurationItem = {
	label: string;
	value: string;
};

export type NodeSection = {
	id: string;
	label: string;
	count?: number;
};

export type GetNodeResponse = {
	state: 'ready' | 'empty' | 'error';
	summary: NodeSummary;
	sections: NodeSection[];
	hostedVms: NodeHostedVm[];
	recentTasks: RelatedTask[];
	configuration: NodeConfigurationItem[];
};

export type ListVmsRequest = {
	page: number;
	page_size: number;
	filters: Record<string, string>;
};

export type VmListItem = {
	vm_id: string;
	name: string;
	node_id: string;
	power_state: string;
	health: string;
	cpu: string;
	memory: string;
	volume_count: number;
	nic_count: number;
	last_task: string;
	alerts?: number;
};

export type ListVmsResponse = {
	items: VmListItem[];
	page: PageMeta;
	filters: FilterMeta;
};

export type GetVmRequest = {
	vm_id: string;
};

export type AttachedVolume = {
	volume_id: string;
	name: string;
	size: string;
	device_name: string;
	read_only: boolean;
	health: string;
};

export type AttachedNic = {
	nic_id: string;
	network_id: string;
	network_name: string;
	mac_address: string;
	ip_address: string;
	nic_model: string;
	addressing_mode: string;
};

export type VmEvent = {
	event_id: string;
	severity: string;
	type: string;
	summary: string;
	occurred_at: string;
	state: string;
};

export type VmSummary = {
	vm_id: string;
	name: string;
	node_id: string;
	power_state: string;
	health: string;
	cpu: string;
	memory: string;
	recent_tasks: RelatedTask[];
	attached_volumes?: AttachedVolume[];
	attached_nics?: AttachedNic[];
};

export type GetVmResponse = {
	summary: VmSummary;
};

export type CreateVmRequest = {
	name?: string;
	display_name?: string;
	node_id?: string;
	cpu_count?: number;
	vcpu?: number;
	memory_bytes?: number;
	memory_mb?: number;
	image_ref?: string;
	image_id?: string;
	requested_by?: string;
	network_id?: string;
	volume_size_gb?: number;
};

export type CreateVmResponse = {
	vm_id: string;
	operation_id: string;
	status: string;
};

export type MutateVmRequest = {
	vm_id: string;
	action: string;
	force: boolean;
};

export type MutateVmResponse = {
	accepted: boolean;
	task_id: string;
	vm_id: string;
	summary: string;
};

export type MutateVolumeRequest = {
	volume_id: string;
	action: string;
	force: boolean;
	resize_bytes?: number;
};

export type MutateVolumeResponse = {
	accepted: boolean;
	task_id: string;
	volume_id: string;
	summary: string;
};

export type VolumeListItem = {
	volume_id: string;
	name: string;
	node_id: string;
	health: string;
	size: string;
	attached_vm_id: string;
	attached_vm_name: string;
	status: string;
	last_task: string;
	alerts?: number;
	backend?: string;
	policy?: string;
};

export type ListVolumesRequest = {
	page: number;
	page_size: number;
	filters: Record<string, string>;
};

export type ListVolumesResponse = {
	items: VolumeListItem[];
	page: PageMeta;
	filters: FilterMeta;
};

export type GetVolumeRequest = {
	volume_id: string;
};

export type VolumeSummary = {
	volume_id: string;
	name: string;
	node_id: string;
	health: string;
	size: string;
	status: string;
	attached_vm_id: string;
	attached_vm_name: string;
	device_name: string;
	read_only: boolean;
	volume_kind: string;
	storage_class: string;
	last_task: string;
	recent_tasks: RelatedTask[];
};

export type GetVolumeResponse = {
	summary: VolumeSummary;
};

export type MutateNodeRequest = {
	node_id: string;
	action: string;
};

export type MutateNodeResponse = {
	accepted: boolean;
	task_id: string;
	node_id: string;
	summary: string;
};

export type EnrollNodeResponse = {
	success: boolean;
	cache_cleared: boolean;
	token_ready: boolean;
	message: string;
};

export type ListTasksRequest = {
	page: number;
	page_size: number;
	filters: Record<string, string>;
};

export type TaskListItem = {
	task_id: string;
	status: string;
	operation: string;
	resource_kind: string;
	resource_id: string;
	resource_name?: string;
	actor: string;
	started_unix_ms: number;
	finished_unix_ms?: number;
	failure_summary?: string;
};

export type ListTasksResponse = {
	items: TaskListItem[];
	page: PageMeta;
	filters: FilterMeta;
};

export type InfrastructureEvent = {
	event_id: string;
	severity: 'info' | 'warning' | 'critical';
	type: string;
	resource_kind: string;
	resource_id: string;
	resource_name?: string;
	summary: string;
	state: 'resolved' | 'unresolved';
	occurred_at: string;
};

export type ListEventsResponse = {
	items: InfrastructureEvent[];
	page: PageMeta;
	filters: FilterMeta;
};

export type ImportImageRequest = {
	name: string;
	source_url?: string;
	checksum?: string;
	os?: string;
	architecture?: string;
	format?: string;
};

export type ImportImageResponse = {
	image_id: string;
	name: string;
	source_url: string;
	format: string;
	status: string;
};

export type MaintenanceWindow = {
	window_id: string;
	title: string;
	status: 'active' | 'pending' | 'completed';
	started_at: string;
	expected_end_at: string;
};

export type MaintenanceNode = {
	node_id: string;
	name: string;
	status: 'draining' | 'in_maintenance' | 'ready';
	progress?: number;
};

export type GetMaintenanceResponse = {
	windows: MaintenanceWindow[];
	nodes: MaintenanceNode[];
	pending_actions: number;
	pending_operator_actions?: { id: string; summary: string }[];
	upgrade_available?: string;
	current_version?: string;
	reboot_required_nodes?: string[];
	orchestrator_health?: string;
};

// View-model types used by the UI
export type VmLifecycleAction = 'start' | 'stop' | 'restart';

export type VmLifecycleActionResult = {
	accepted: boolean;
	action: VmLifecycleAction;
	summary: string;
	taskId: string | null;
	taskLabel: string;
	taskTone: 'healthy' | 'warning' | 'degraded' | 'failed' | 'unknown';
	taskHref: string | null;
};

export type MutationActionResult = {
	accepted: boolean;
	action: string;
	summary: string;
	taskId: string | null;
	taskLabel: string;
	taskTone: 'healthy' | 'warning' | 'degraded' | 'failed' | 'unknown';
	taskHref: string | null;
};

export type IpamMode = 'internal' | 'external' | 'none';

export interface CreateNetworkInput {
	name: string;
	cidr: string;
	gateway: string;
	bridge_name?: string;
	dhcp_enabled?: boolean;
	ipam_mode?: IpamMode;
	is_default?: boolean;
}

export interface NetworkListItem {
	network_id: string;
	name: string;
	exposure: string;
	health: string;
	attached_vms: number;
	last_task: string;
	alerts: number;
	dhcp_enabled: boolean;
	ipam_mode: string;
	is_default: boolean;
}

export interface NetworkDetailModel {
	network_id: string;
	name: string;
	scope: string;
	health: string;
	exposure: string;
	policy: string;
	cidr: string;
	gateway: string;
	dhcp_enabled: boolean;
	ipam_mode: string;
	is_default: boolean;
	attached_vms: Array<{
		vm_id: string;
		display_name: string;
		runtime_status: string;
		ip_address?: string;
		mac_address?: string;
	}>;
	created_at: string;
	last_task: string;
	alerts: number;
}
