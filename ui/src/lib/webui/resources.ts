import type { Event, Network, NodeWithResources, Operation, StoragePool, VM } from '$lib/api/types';
import type { ShellTone } from '$lib/shell/app-shell';
import { buildTaskList, getTaskStatusMeta, type TaskTimelineItemModel } from '$lib/webui/tasks';

export interface ResourceListFilters {
	query?: string;
	state?: string;
	maintenance?: string;
	powerState?: string;
	health?: string;
	nodeId?: string;
}

export interface ResourceFilterModel {
	current: Record<string, string>;
	applied: Record<string, string>;
}

export interface NodeListItemModel {
	nodeId: string;
	name: string;
	cluster: string;
	stateLabel: string;
	stateTone: ShellTone;
	healthLabel: string;
	healthTone: ShellTone;
	cpuLabel: string;
	memoryLabel: string;
	storageLabel: string;
	networkLabel: string;
	versionLabel: string;
	maintenanceLabel: string;
	maintenanceTone: ShellTone;
	href: string;
}

export interface VmListItemModel {
	vmId: string;
	name: string;
	nodeId: string | null;
	nodeName: string;
	powerStateLabel: string;
	powerStateTone: ShellTone;
	healthLabel: string;
	healthTone: ShellTone;
	cpuLabel: string;
	memoryLabel: string;
	storageCount: number;
	networkCount: number;
	tagsLabel: string;
	lastTaskId: string | null;
	lastTaskLabel: string;
	lastTaskTone: ShellTone;
	href: string;
}

export interface EventItemModel {
	id: string;
	label: string;
	tone: ShellTone;
	message: string;
	timestampLabel: string;
}

export interface DetailTab {
	id: string;
	label: string;
	count?: number;
}

export interface SummaryCardModel {
	label: string;
	value: string;
	note: string;
	tone?: ShellTone;
}

export interface KeyValueItemModel {
	label: string;
	value: string;
}

export interface NodeDetailModel {
	state: 'ready' | 'empty' | 'error';
	currentTab: string;
	summary: {
		nodeId: string;
		name: string;
		cluster: string;
		stateLabel: string;
		stateTone: ShellTone;
		healthLabel: string;
		healthTone: ShellTone;
		hostname: string;
		ipAddress: string;
		versionLabel: string;
		maintenanceLabel: string;
		maintenanceTone: ShellTone;
	};
	sections: DetailTab[];
	summaryCards: SummaryCardModel[];
	hostedVms: VmListItemModel[];
	storagePools: Array<{
		id: string;
		name: string;
		statusLabel: string;
		statusTone: ShellTone;
		capacityLabel: string;
		path: string;
	}>;
	networks: Array<{
		id: string;
		name: string;
		statusLabel: string;
		statusTone: ShellTone;
		scopeLabel: string;
		cidr: string;
	}>;
	recentTasks: TaskTimelineItemModel[];
	events: EventItemModel[];
	configuration: KeyValueItemModel[];
	alerts: string[];
}

export interface VmDetailModel {
	state: 'ready' | 'empty' | 'error';
	currentTab: string;
	summary: {
		vmId: string;
		name: string;
		nodeId: string | null;
		nodeName: string;
		powerStateLabel: string;
		powerStateTone: ShellTone;
		healthLabel: string;
		healthTone: ShellTone;
		ipAddress: string;
		consoleLabel: string;
	};
	sections: DetailTab[];
	summaryCards: SummaryCardModel[];
	storageItems: Array<{
		id: string;
		name: string;
		statusLabel: string;
		statusTone: ShellTone;
		sizeLabel: string;
		path: string;
	}>;
	networkItems: Array<{
		id: string;
		name: string;
		statusLabel: string;
		statusTone: ShellTone;
		scopeLabel: string;
		cidr: string;
		gateway: string;
	}>;
	recentTasks: TaskTimelineItemModel[];
	events: EventItemModel[];
	configuration: KeyValueItemModel[];
	alerts: string[];
}

export interface NodesListModel {
	items: NodeListItemModel[];
	state: 'ready' | 'empty' | 'error';
	filters: ResourceFilterModel;
}

export interface VmsListModel {
	items: VmListItemModel[];
	state: 'ready' | 'empty' | 'error';
	filters: ResourceFilterModel;
}

export interface NodesListSnapshot {
	nodes: NodeWithResources[];
	operations: Operation[];
	events: Event[];
}

export interface VmsListSnapshot {
	vms: VM[];
	nodes: NodeWithResources[];
	vmPlacements?: Record<string, string>;
	operations: Operation[];
	events: Event[];
}

export interface NodeDetailSnapshot {
	nodes: NodeWithResources[];
	nodeVms: VM[];
	nodeStoragePools: StoragePool[];
	nodeNetworks: Network[];
	operations: Operation[];
	events: Event[];
}

export interface VmDetailSnapshot {
	vm: VM | null;
	nodes: NodeWithResources[];
	vmPlacements?: Record<string, string>;
	storagePools: StoragePool[];
	networks: Network[];
	operations: Operation[];
	events: Event[];
}

interface BuildOptions {
	now?: Date;
	fetchFailed?: boolean;
}

export function buildNodesList(
	snapshot: NodesListSnapshot,
	filters: ResourceListFilters = {},
	options: BuildOptions = {}
): NodesListModel {
	const items = snapshot.nodes
		.map((node) => mapNodeListItem(node))
		.filter((item) => matchesNodeFilters(item, filters))
		.sort((left, right) => left.name.localeCompare(right.name));
	const current = getCurrentNodeFilters(filters);

	return {
		items,
		state: items.length > 0 ? 'ready' : options.fetchFailed ? 'error' : 'empty',
		filters: {
			current,
			applied: getAppliedNodeFilters(current)
		}
	};
}

export function buildVmsList(
	snapshot: VmsListSnapshot,
	filters: ResourceListFilters = {},
	options: BuildOptions = {}
): VmsListModel {
	const now = options.now ?? new Date();
	const items = snapshot.vms
		.map((vm) =>
			mapVmListItem(
				vm,
				snapshot.nodes,
				snapshot.vmPlacements ?? {},
				snapshot.operations,
				snapshot.events,
				now
			)
		)
		.filter((item) => matchesVmFilters(item, filters))
		.sort((left, right) => left.name.localeCompare(right.name));
	const current = getCurrentVmFilters(filters);

	return {
		items,
		state: items.length > 0 ? 'ready' : options.fetchFailed ? 'error' : 'empty',
		filters: {
			current,
			applied: getAppliedVmFilters(current)
		}
	};
}

export function buildNodeDetail(
	snapshot: NodeDetailSnapshot,
	nodeId: string,
	currentTab = 'summary',
	options: BuildOptions = {}
): NodeDetailModel {
	const node = snapshot.nodes.find((item) => item.id === nodeId);
	if (!node) {
		return emptyNodeDetail(nodeId, currentTab, options.fetchFailed ? 'error' : 'empty');
	}

	const recentTasks = getRelatedTasks('node', nodeId, snapshot.operations, snapshot.events, options.now);
	const events = getRelatedEvents('node', nodeId, snapshot.events);
	const sections: DetailTab[] = [
		{ id: 'summary', label: 'Summary' },
		{ id: 'vms', label: 'Virtual Machines', count: snapshot.nodeVms.length },
		{ id: 'volumes', label: 'Volumes', count: snapshot.nodeStoragePools.length },
		{ id: 'networks', label: 'Networks', count: snapshot.nodeNetworks.length },
		{ id: 'tasks', label: 'Tasks', count: recentTasks.length },
		{ id: 'events', label: 'Events', count: events.length },
		{ id: 'configuration', label: 'Configuration' }
	];
	const state = sections.some((section) => (section.count ?? 0) > 0) || node ? 'ready' : options.fetchFailed ? 'error' : 'empty';

	return {
		state,
		currentTab,
		summary: {
			nodeId: node.id,
			name: node.name,
			cluster: getClusterLabel(node),
			stateLabel: toTitle(node.status),
			stateTone: mapNodeTone(node.status),
			healthLabel: getNodeHealthLabel(node.status),
			healthTone: mapNodeTone(node.status),
			hostname: node.hostname,
			ipAddress: node.ip_address,
			versionLabel: getVersionLabel(node.capabilities),
			maintenanceLabel: node.status === 'maintenance' ? 'In maintenance' : 'Scheduling enabled',
			maintenanceTone: node.status === 'maintenance' ? 'warning' : 'healthy'
		},
		sections,
		summaryCards: [
			{
				label: 'Hosted VMs',
				value: String(snapshot.nodeVms.length),
				note: `${snapshot.nodeVms.filter((vm) => vm.actual_state === 'running').length} running`,
				tone: snapshot.nodeVms.some((vm) => vm.actual_state === 'failed') ? 'failed' : 'healthy'
			},
				{
					label: 'Storage pools',
					value: String(snapshot.nodeStoragePools.length),
					note: snapshot.nodeStoragePools.length > 0 ? 'Node-attached capacity surfaces here' : 'No storage reported yet',
					tone:
						snapshot.nodeStoragePools.length === 0
							? 'unknown'
							: snapshot.nodeStoragePools.some((pool) => pool.status.toLowerCase() === 'degraded')
								? 'degraded'
								: 'healthy'
				},
			{
				label: 'Networks',
				value: String(snapshot.nodeNetworks.length),
				note: snapshot.nodeNetworks.length > 0 ? 'Bridge health and scope stay visible' : 'No network attachments reported',
				tone: snapshot.nodeNetworks.some((network) => network.status.toLowerCase() === 'degraded')
					? 'degraded'
					: 'healthy'
			}
		],
		hostedVms: snapshot.nodeVms.map((vm) =>
			mapVmListItem(vm, snapshot.nodes, Object.fromEntries(snapshot.nodeVms.map((item) => [item.id, nodeId])), snapshot.operations, snapshot.events, options.now ?? new Date())
		),
		storagePools: snapshot.nodeStoragePools.map((pool) => ({
			id: pool.id,
			name: pool.name,
			statusLabel: toTitle(pool.status),
			statusTone: mapHealthTone(pool.status),
			capacityLabel: pool.capacity_bytes ? formatBytes(pool.capacity_bytes) : 'Not yet reported',
			path: pool.path
		})),
		networks: snapshot.nodeNetworks.map((network) => ({
			id: network.id,
			name: network.name,
			statusLabel: toTitle(network.status),
			statusTone: mapHealthTone(network.status),
			scopeLabel: network.is_system_managed ? 'System managed' : 'Operator managed',
			cidr: network.cidr
		})),
		recentTasks,
		events,
		configuration: [
			{ label: 'Hostname', value: node.hostname },
			{ label: 'IP address', value: node.ip_address },
			{ label: 'Agent URL', value: node.agent_url ?? 'Not exposed through the current API' },
			{ label: 'Capabilities', value: node.capabilities ?? 'Not yet reported' },
			{ label: 'Last seen', value: node.last_seen_at ? formatDateTime(node.last_seen_at) : 'Not yet reported' }
		],
		alerts: events.filter((event) => event.tone === 'failed' || event.tone === 'warning').map((event) => event.message).slice(0, 4)
	};
}

export function buildVmDetail(
	snapshot: VmDetailSnapshot,
	currentTab = 'summary',
	options: BuildOptions = {}
): VmDetailModel {
	if (!snapshot.vm) {
		return emptyVmDetail(currentTab, options.fetchFailed ? 'error' : 'empty');
	}

	const vm = snapshot.vm;
	const nodeId = (snapshot.vmPlacements ?? {})[vm.id] ?? null;
	const node = nodeId ? snapshot.nodes.find((item) => item.id === nodeId) ?? null : null;
	const recentTasks = getRelatedTasks('vm', vm.id, snapshot.operations, snapshot.events, options.now);
	const events = getRelatedEvents('vm', vm.id, snapshot.events);
	const health = getVmHealth(vm);

	return {
		state: 'ready',
		currentTab,
		summary: {
			vmId: vm.id,
			name: vm.name,
			nodeId,
			nodeName: node?.name ?? 'Placement not reported yet',
			powerStateLabel: toTitle(vm.actual_state),
			powerStateTone: mapVmPowerTone(vm.actual_state),
			healthLabel: health.label,
			healthTone: health.tone,
			ipAddress: vm.ip_address ?? 'No guest IP reported',
			consoleLabel: vm.console_type === 'serial' ? 'Serial console' : 'Console access not yet reported'
		},
		sections: [
			{ id: 'summary', label: 'Summary' },
			{ id: 'console', label: 'Console / Access' },
			{ id: 'volumes', label: 'Volumes', count: 1 },
			{ id: 'networks', label: 'Networks', count: 1 },
			{ id: 'configuration', label: 'Configuration' },
			{ id: 'tasks', label: 'Tasks', count: recentTasks.length },
			{ id: 'events', label: 'Events', count: events.length }
		],
		summaryCards: [
			{
				label: 'Power state',
				value: toTitle(vm.actual_state),
				note: `Desired state: ${toTitle(vm.desired_state)}`,
				tone: mapVmPowerTone(vm.actual_state)
			},
			{
				label: 'CPU',
				value: `${vm.vcpu} vCPU`,
				note: 'Requested guest compute',
				tone: 'healthy'
			},
			{
				label: 'Memory',
				value: formatMegabytes(vm.memory_mb),
				note: 'Configured guest memory',
				tone: 'healthy'
			}
		],
		storageItems: [
			{
				id: vm.storage_pool_id,
				name: snapshot.storagePools.find((pool) => pool.id === vm.storage_pool_id)?.name ?? vm.storage_pool_id,
				statusLabel: toTitle(snapshot.storagePools.find((pool) => pool.id === vm.storage_pool_id)?.status ?? 'unknown'),
				statusTone: mapHealthTone(snapshot.storagePools.find((pool) => pool.id === vm.storage_pool_id)?.status ?? 'unknown'),
				sizeLabel: snapshot.storagePools.find((pool) => pool.id === vm.storage_pool_id)?.capacity_bytes
					? formatBytes(snapshot.storagePools.find((pool) => pool.id === vm.storage_pool_id)?.capacity_bytes ?? 0)
					: 'Managed by pool policy',
				path: vm.disk_path
			}
		],
		networkItems: [
			{
				id: vm.network_id,
				name: snapshot.networks.find((network) => network.id === vm.network_id)?.name ?? vm.network_id,
				statusLabel: toTitle(snapshot.networks.find((network) => network.id === vm.network_id)?.status ?? 'unknown'),
				statusTone: mapHealthTone(snapshot.networks.find((network) => network.id === vm.network_id)?.status ?? 'unknown'),
				scopeLabel: snapshot.networks.find((network) => network.id === vm.network_id)?.is_system_managed ? 'System managed' : 'Operator managed',
				cidr: snapshot.networks.find((network) => network.id === vm.network_id)?.cidr ?? 'Not reported',
				gateway: snapshot.networks.find((network) => network.id === vm.network_id)?.gateway_ip ?? 'Not reported'
			}
		],
		recentTasks,
		events,
		configuration: [
			{ label: 'VM ID', value: vm.id },
			{ label: 'Node', value: node?.name ?? 'Placement not reported yet' },
			{ label: 'Image', value: vm.image_id },
			{ label: 'Workspace path', value: vm.workspace_path },
			{ label: 'Seed ISO', value: vm.seed_iso_path },
			{ label: 'Last error', value: vm.last_error ?? 'No error recorded' }
		],
		alerts: [vm.last_error, ...events.filter((event) => event.tone === 'failed').map((event) => event.message)]
			.filter(Boolean)
			.slice(0, 4) as string[]
	};
}

function mapNodeListItem(node: NodeWithResources): NodeListItemModel {
	const tone = mapNodeTone(node.status);
	return {
		nodeId: node.id,
		name: node.name,
		cluster: getClusterLabel(node),
		stateLabel: toTitle(node.status),
		stateTone: tone,
		healthLabel: getNodeHealthLabel(node.status),
		healthTone: tone,
		cpuLabel: node.status === 'online' ? `${node.resources.vms} hosted VMs` : 'Telemetry unavailable',
		memoryLabel: 'Not yet reported',
		storageLabel: `${node.resources.storage_pools} pools`,
		networkLabel: `${node.resources.networks} networks`,
		versionLabel: getVersionLabel(node.capabilities),
		maintenanceLabel: node.status === 'maintenance' ? 'In maintenance' : 'Active',
		maintenanceTone: node.status === 'maintenance' ? 'warning' : 'healthy',
		href: `/nodes/${node.id}`
	};
}

function mapVmListItem(
	vm: VM,
	nodes: NodeWithResources[],
	vmPlacements: Record<string, string>,
	operations: Operation[],
	events: Event[],
	now: Date
): VmListItemModel {
	const nodeId = vmPlacements[vm.id] ?? null;
	const node = nodeId ? nodes.find((item) => item.id === nodeId) ?? null : null;
	const relatedTasks = getRelatedTasks('vm', vm.id, operations, events, now, 1);
	const health = getVmHealth(vm);

	return {
		vmId: vm.id,
		name: vm.name,
		nodeId,
		nodeName: node?.name ?? 'Placement not reported yet',
		powerStateLabel: toTitle(vm.actual_state),
		powerStateTone: mapVmPowerTone(vm.actual_state),
		healthLabel: health.label,
		healthTone: health.tone,
		cpuLabel: `${vm.vcpu} vCPU`,
		memoryLabel: formatMegabytes(vm.memory_mb),
		storageCount: vm.storage_pool_id ? 1 : 0,
		networkCount: vm.network_id ? 1 : 0,
		tagsLabel: 'No tags',
		lastTaskId: relatedTasks[0]?.taskId ?? null,
		lastTaskLabel: relatedTasks[0]?.label ?? 'No recent task',
		lastTaskTone: relatedTasks[0]?.tone ?? 'unknown',
		href: `/vms/${vm.id}`
	};
}

function matchesNodeFilters(item: NodeListItemModel, filters: ResourceListFilters): boolean {
	const query = filters.query?.trim().toLowerCase();
	if (query) {
		const haystack = [item.name, item.cluster].join(' ').toLowerCase();
		if (!haystack.includes(query)) {
			return false;
		}
	}

	if (filters.state && filters.state !== 'all' && item.stateLabel.toLowerCase() !== filters.state.toLowerCase()) {
		return false;
	}

	if (filters.maintenance && filters.maintenance !== 'all') {
		const inMaintenance = item.maintenanceTone === 'warning';
		if ((filters.maintenance === 'true' && !inMaintenance) || (filters.maintenance === 'false' && inMaintenance)) {
			return false;
		}
	}

	return true;
}

function matchesVmFilters(item: VmListItemModel, filters: ResourceListFilters): boolean {
	const query = filters.query?.trim().toLowerCase();
	if (query) {
		const haystack = [item.name, item.nodeName, item.powerStateLabel].join(' ').toLowerCase();
		if (!haystack.includes(query)) {
			return false;
		}
	}

	if (
		filters.powerState &&
		filters.powerState !== 'all' &&
		item.powerStateLabel.toLowerCase() !== filters.powerState.toLowerCase()
	) {
		return false;
	}

	if (filters.health && filters.health !== 'all' && item.healthLabel.toLowerCase() !== filters.health.toLowerCase()) {
		return false;
	}

	if (filters.nodeId && filters.nodeId !== 'all' && item.nodeId !== filters.nodeId) {
		return false;
	}

	return true;
}

function getRelatedTasks(
	resourceKind: string,
	resourceId: string,
	operations: Operation[],
	events: Event[],
	now = new Date(),
	limit = 6
): TaskTimelineItemModel[] {
	return buildTaskList(
		{ operations, events },
		{},
		{ now, pageSize: Math.max(operations.length, limit) || limit }
	).items
		.filter((item) => item.resourceKind === resourceKind && item.resourceId === resourceId)
		.slice(0, limit);
}

function getRelatedEvents(resourceKind: string, resourceId: string, events: Event[]): EventItemModel[] {
	return events
		.filter((event) => event.resource.toLowerCase() === resourceKind && event.resource_id === resourceId)
		.sort((left, right) => Date.parse(right.timestamp) - Date.parse(left.timestamp))
		.map((event) => ({
			id: event.id,
			label: event.status === 'failed' ? 'Failed' : event.status === 'pending' ? 'Pending' : 'Successful',
			tone: event.status === 'failed' ? 'failed' : event.status === 'pending' ? 'warning' : 'healthy',
			message: event.message ?? `${toTitle(event.operation)} ${resourceKind}`,
			timestampLabel: formatDateTime(event.timestamp)
		}));
}

function getCurrentNodeFilters(filters: ResourceListFilters): Record<string, string> {
	return {
		query: filters.query?.trim() || '',
		state: filters.state?.trim() || 'all',
		maintenance: filters.maintenance?.trim() || 'all'
	};
}

function getAppliedNodeFilters(current: Record<string, string>): Record<string, string> {
	const applied: Record<string, string> = {};
	if (current.query) applied.query = current.query;
	if (current.state !== 'all') applied.state = current.state.toLowerCase();
	if (current.maintenance !== 'all') applied.maintenance = current.maintenance;
	return applied;
}

function getCurrentVmFilters(filters: ResourceListFilters): Record<string, string> {
	return {
		query: filters.query?.trim() || '',
		powerState: filters.powerState?.trim() || 'all',
		health: filters.health?.trim() || 'all',
		nodeId: filters.nodeId?.trim() || 'all'
	};
}

function getAppliedVmFilters(current: Record<string, string>): Record<string, string> {
	const applied: Record<string, string> = {};
	if (current.query) applied.query = current.query;
	if (current.powerState !== 'all') applied.powerState = current.powerState.toLowerCase();
	if (current.health !== 'all') applied.health = current.health.toLowerCase();
	if (current.nodeId !== 'all') applied.nodeId = current.nodeId;
	return applied;
}

function getClusterLabel(node: NodeWithResources): string {
	return node.is_local ? 'Local control cluster' : 'Attached compute cluster';
}

function getVersionLabel(capabilities: string | undefined): string {
	if (!capabilities) {
		return 'Not yet reported';
	}

	return capabilities.split(',')[0] || capabilities;
}

function getNodeHealthLabel(status: string): string {
	switch (status) {
		case 'online':
			return 'Healthy';
		case 'maintenance':
			return 'Maintenance';
		case 'offline':
			return 'Degraded';
		case 'error':
			return 'Failed';
		default:
			return 'Unknown';
	}
}

function mapNodeTone(status: string): ShellTone {
	switch (status) {
		case 'online':
			return 'healthy';
		case 'maintenance':
			return 'warning';
		case 'offline':
			return 'degraded';
		case 'error':
			return 'failed';
		default:
			return 'unknown';
	}
}

function mapVmPowerTone(state: string): ShellTone {
	switch (state.toLowerCase()) {
		case 'running':
			return 'healthy';
		case 'starting':
		case 'stopping':
		case 'rebooting':
		case 'creating':
		case 'deleting':
			return 'degraded';
		case 'failed':
			return 'failed';
		case 'unknown':
			return 'unknown';
		default:
			return 'warning';
	}
}

function mapHealthTone(state: string): ShellTone {
	const normalized = state.toLowerCase();
	if (['ready', 'healthy', 'online', 'running', 'success'].includes(normalized)) return 'healthy';
	if (['maintenance', 'pending'].includes(normalized)) return 'warning';
	if (['degraded', 'offline', 'starting', 'stopping', 'rebooting'].includes(normalized)) return 'degraded';
	if (['failed', 'error'].includes(normalized)) return 'failed';
	return 'unknown';
}

function getVmHealth(vm: VM): { label: string; tone: ShellTone } {
	switch (vm.actual_state.toLowerCase()) {
		case 'failed':
			return { label: 'Failed', tone: 'failed' };
		case 'unknown':
			return { label: 'Unknown', tone: 'unknown' };
		case 'creating':
		case 'starting':
		case 'stopping':
		case 'rebooting':
		case 'deleting':
			return { label: 'Degraded', tone: 'degraded' };
		default:
			return { label: 'Healthy', tone: 'healthy' };
	}
}

function emptyNodeDetail(nodeId: string, currentTab: string, state: 'empty' | 'error'): NodeDetailModel {
	return {
		state,
		currentTab,
		summary: {
			nodeId,
			name: 'Node unavailable',
			cluster: 'Unavailable',
			stateLabel: 'Unknown',
			stateTone: 'unknown',
			healthLabel: 'Unknown',
			healthTone: 'unknown',
			hostname: 'Unavailable',
			ipAddress: 'Unavailable',
			versionLabel: 'Unavailable',
			maintenanceLabel: 'Unavailable',
			maintenanceTone: 'unknown'
		},
		sections: [
			{ id: 'summary', label: 'Summary' },
			{ id: 'vms', label: 'Virtual Machines' },
			{ id: 'volumes', label: 'Volumes' },
			{ id: 'networks', label: 'Networks' },
			{ id: 'tasks', label: 'Tasks' },
			{ id: 'events', label: 'Events' },
			{ id: 'configuration', label: 'Configuration' }
		],
		summaryCards: [],
		hostedVms: [],
		storagePools: [],
		networks: [],
		recentTasks: [],
		events: [],
		configuration: [],
		alerts: []
	};
}

function emptyVmDetail(currentTab: string, state: 'empty' | 'error'): VmDetailModel {
	return {
		state,
		currentTab,
		summary: {
			vmId: '',
			name: 'Virtual machine unavailable',
			nodeId: null,
			nodeName: 'Unavailable',
			powerStateLabel: 'Unknown',
			powerStateTone: 'unknown',
			healthLabel: 'Unknown',
			healthTone: 'unknown',
			ipAddress: 'Unavailable',
			consoleLabel: 'Unavailable'
		},
		sections: [
			{ id: 'summary', label: 'Summary' },
			{ id: 'console', label: 'Console / Access' },
			{ id: 'volumes', label: 'Volumes' },
			{ id: 'networks', label: 'Networks' },
			{ id: 'configuration', label: 'Configuration' },
			{ id: 'tasks', label: 'Tasks' },
			{ id: 'events', label: 'Events' }
		],
		summaryCards: [],
		storageItems: [],
		networkItems: [],
		recentTasks: [],
		events: [],
		configuration: [],
		alerts: []
	};
}

function toTitle(value: string): string {
	return value
		.replace(/[_-]+/g, ' ')
		.replace(/\s+/g, ' ')
		.trim()
		.replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function formatDateTime(value: string): string {
	return new Intl.DateTimeFormat('en-US', {
		month: 'short',
		day: 'numeric',
		hour: 'numeric',
		minute: '2-digit'
	}).format(new Date(value));
}

function formatMegabytes(value: number): string {
	if (value >= 1024) {
		return `${(value / 1024).toFixed(value >= 10_240 ? 0 : 1)} GB`;
	}
	return `${value} MB`;
}

function formatBytes(value: number): string {
	const units = ['B', 'KB', 'MB', 'GB', 'TB'];
	let current = value;
	let index = 0;

	while (current >= 1024 && index < units.length - 1) {
		current /= 1024;
		index += 1;
	}

	return `${current.toFixed(current >= 100 || index === 0 ? 0 : 1)} ${units[index]}`;
}
