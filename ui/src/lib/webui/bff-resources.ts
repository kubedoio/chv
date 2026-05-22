import { listEvents } from '$lib/bff/events';
import { listImages } from '$lib/bff/images';
import { listNetworks } from '$lib/bff/networks';
import { listNodes } from '$lib/bff/nodes';
import { listTasks } from '$lib/bff/tasks';
import { getVm, listVms } from '$lib/bff/vms';
import type {
	InfrastructureEvent,
	NodeListItem,
	TaskListItem,
	VmListItem
} from '$lib/bff/types';
import type { Event, Image, Network, NodeWithResources, Operation, VM } from '$lib/api/types';

const DEFAULT_PAGE = { page: 1, page_size: 200, filters: {} };

export async function loadNodesFromBff(token?: string): Promise<NodeWithResources[]> {
	const response = await listNodes(DEFAULT_PAGE, token);
	return response.items.map(mapNode);
}

export async function loadVmsFromBff(token?: string): Promise<VM[]> {
	const response = await listVms(DEFAULT_PAGE, token);
	return response.items.map(mapVmListItem);
}

export async function loadVmFromBff(vmId: string, token?: string): Promise<VM | null> {
	const response = await getVm({ vm_id: vmId }, token);
	return response.summary ? mapVmSummary(response.summary) : null;
}

export async function loadNetworksFromBff(token?: string): Promise<Network[]> {
	const response = await listNetworks(token);
	return response.items.map(mapNetwork);
}

export async function loadImagesFromBff(token?: string): Promise<Image[]> {
	const response = await listImages(token);
	return response.items.map(mapImage);
}

export async function loadOperationsFromBff(token?: string): Promise<Operation[]> {
	const response = await listTasks(DEFAULT_PAGE, token);
	return response.items.map(mapTask);
}

export async function loadEventsFromBff(token?: string): Promise<Event[]> {
	const response = await listEvents(token);
	return response.items.map(mapEvent);
}

function mapNode(node: NodeListItem): NodeWithResources {
	return {
		id: node.node_id,
		name: node.name,
		hostname: node.name,
		ip_address: '',
		status: mapNodeStatus(node.state, node.health, node.maintenance),
		is_local: false,
		capabilities: node.hypervisor_capabilities?.join(','),
		resources: {
			vms: 0,
			images: 0,
			storage_pools: 0,
			networks: 0
		}
	};
}

function mapVmListItem(vm: VmListItem): VM {
	return {
		id: vm.vm_id,
		name: vm.name,
		node_id: vm.node_id,
		image_id: '',
		storage_pool_id: '',
		network_id: '',
		desired_state: vm.power_state,
		actual_state: vm.power_state,
		vcpu: parsePositiveInt(vm.cpu),
		memory_mb: parseMemoryMb(vm.memory),
		disk_path: '',
		seed_iso_path: '',
		workspace_path: ''
	};
}

function mapVmSummary(vm: { vm_id: string; name: string; node_id: string; power_state: string; cpu: string; memory: string }): VM {
	return {
		id: vm.vm_id,
		name: vm.name,
		node_id: vm.node_id,
		image_id: '',
		storage_pool_id: '',
		network_id: '',
		desired_state: vm.power_state,
		actual_state: vm.power_state,
		vcpu: parsePositiveInt(vm.cpu),
		memory_mb: parseMemoryMb(vm.memory),
		disk_path: '',
		seed_iso_path: '',
		workspace_path: ''
	};
}

function mapNetwork(network: Record<string, unknown>): Network {
	return {
		id: String(network.network_id ?? network.id ?? ''),
		name: String(network.name ?? ''),
		mode: String(network.exposure ?? network.scope ?? 'bridge'),
		bridge_name: '',
		cidr: String(network.cidr ?? ''),
		gateway_ip: String(network.gateway ?? ''),
		is_system_managed: Boolean(network.is_default ?? false),
		status: String(network.health ?? network.status ?? 'unknown'),
		created_at: String(network.created_at ?? '')
	};
}

function mapImage(image: Record<string, unknown>): Image {
	return {
		id: String(image.image_id ?? image.id ?? ''),
		name: String(image.name ?? ''),
		os_family: String(image.os ?? ''),
		architecture: String(image.version ?? ''),
		format: '',
		source_url: '',
		local_path: '',
		cloud_init_supported: true,
		status: String(image.status ?? 'unknown'),
		created_at: String(image.last_updated ?? '')
	};
}

function mapTask(task: TaskListItem): Operation {
	return {
		id: task.task_id,
		resource_type: task.resource_kind,
		resource_id: task.resource_id,
		operation_type: task.operation,
		state: task.status,
		created_at: new Date(task.started_unix_ms || Date.now()).toISOString()
	};
}

function mapEvent(event: InfrastructureEvent): Event {
	return {
		id: event.event_id,
		timestamp: event.occurred_at,
		operation: event.type,
		status: event.state === 'resolved' ? 'success' : event.severity === 'critical' ? 'failed' : 'pending',
		resource: event.resource_kind,
		resource_id: event.resource_id,
		message: event.summary
	};
}

function mapNodeStatus(state: string, health: string, maintenance: boolean): NodeWithResources['status'] {
	const normalizedState = state.trim().toLowerCase();
	const normalizedHealth = health.trim().toLowerCase();
	if (maintenance || normalizedState === 'maintenance') return 'maintenance';
	if (normalizedHealth === 'failed' || normalizedHealth === 'error' || normalizedState === 'failed') return 'error';
	if (normalizedState === 'unknown' || normalizedState === 'disconnected') return 'offline';
	return 'online';
}

function parsePositiveInt(value: string): number {
	const parsed = Number.parseInt(value, 10);
	return Number.isFinite(parsed) && parsed > 0 ? parsed : 0;
}

function parseMemoryMb(value: string): number {
	const trimmed = value.trim().toLowerCase();
	const parsed = Number.parseFloat(trimmed);
	if (!Number.isFinite(parsed)) return 0;
	if (trimmed.includes('gib')) return Math.round(parsed * 1024);
	if (trimmed.includes('kib')) return Math.round(parsed / 1024);
	return Math.round(parsed);
}
