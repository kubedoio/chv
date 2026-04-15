import type { Event, NodeWithResources, Operation, StoragePool, VM } from '$lib/api/types';
import type { ShellTone } from '$lib/shell/app-shell';
import { buildTaskList, type TaskSnapshot, type TaskTimelineItemModel } from '$lib/webui/tasks';

export interface HealthTileModel {
	key: string;
	label: string;
	status: ShellTone;
	value: string;
	detail: string;
}

export interface CapacityTileModel {
	key: string;
	label: string;
	used: string;
	total: string;
	detail: string;
	status: ShellTone;
}

export interface OverviewModel {
	state: 'ready' | 'empty' | 'error';
	healthTiles: HealthTileModel[];
	capacityTiles: CapacityTileModel[];
	activeAlerts: string[];
	recentTasks: TaskTimelineItemModel[];
}

export interface OverviewSnapshot extends TaskSnapshot {
	nodes: NodeWithResources[];
	vms: VM[];
	storagePools: StoragePool[];
}

interface BuildOverviewOptions {
	now?: Date;
	fetchFailed?: boolean;
}

export function buildOverviewModel(
	snapshot: OverviewSnapshot,
	options: BuildOverviewOptions = {}
): OverviewModel {
	const now = options.now ?? new Date();
	const taskList = buildTaskList(
		{
			operations: snapshot.operations,
			events: snapshot.events
		},
		{},
		{ now, pageSize: 5, fetchFailed: options.fetchFailed }
	);
	const healthTiles = [
		buildFleetHealthTile(snapshot),
		buildNodeHealthTile(snapshot.nodes),
		buildWorkloadHealthTile(snapshot.vms),
		buildStorageHealthTile(snapshot.storagePools)
	];
	const capacityTiles = [
		buildNodeCapacityTile(snapshot.nodes),
		buildVmCapacityTile(snapshot.vms),
		buildStorageCapacityTile(snapshot.storagePools)
	];
	const activeAlerts = buildActiveAlerts(snapshot);
	const hasData =
		snapshot.nodes.length > 0 ||
		snapshot.vms.length > 0 ||
		snapshot.storagePools.length > 0 ||
		snapshot.operations.length > 0 ||
		snapshot.events.length > 0;
	const state = hasData ? 'ready' : options.fetchFailed ? 'error' : 'empty';

	return {
		state,
		healthTiles,
		capacityTiles,
		activeAlerts,
		recentTasks: taskList.items
	};
}

function buildFleetHealthTile(snapshot: OverviewSnapshot): HealthTileModel {
	const statuses = [
		buildNodeHealthTile(snapshot.nodes).status,
		buildWorkloadHealthTile(snapshot.vms).status,
		buildStorageHealthTone(snapshot.storagePools)
	];
	const status = combineTones(statuses);

	return {
		key: 'fleet',
		label: 'Fleet health',
		status,
		value: status === 'healthy' ? 'Nominal' : titleizeTone(status),
		detail: `${snapshot.nodes.length} nodes, ${snapshot.vms.length} VMs, ${snapshot.storagePools.length} pools`
	};
}

function buildNodeHealthTile(nodes: NodeWithResources[]): HealthTileModel {
	const total = nodes.length;
	const errorCount = nodes.filter((node) => node.status === 'error').length;
	const offlineCount = nodes.filter((node) => node.status === 'offline').length;
	const maintenanceCount = nodes.filter((node) => node.status === 'maintenance').length;
	const onlineCount = nodes.filter((node) => node.status === 'online').length;
	const status =
		total === 0
			? 'unknown'
			: errorCount > 0
				? 'failed'
				: offlineCount > 0
					? 'degraded'
					: maintenanceCount > 0
						? 'warning'
						: 'healthy';

	return {
		key: 'nodes',
		label: 'Node readiness',
		status,
		value: `${onlineCount}/${total || 0} ready`,
		detail:
			total === 0
				? 'No nodes enrolled yet'
				: `${errorCount} errors, ${offlineCount} offline, ${maintenanceCount} in maintenance`
	};
}

function buildWorkloadHealthTile(vms: VM[]): HealthTileModel {
	const total = vms.length;
	const runningCount = vms.filter((vm) => vm.actual_state === 'running').length;
	const failedCount = vms.filter((vm) => vm.actual_state === 'failed').length;
	const unknownCount = vms.filter((vm) => vm.actual_state === 'unknown').length;
	const transitionalCount = vms.filter((vm) =>
		['creating', 'starting', 'stopping', 'rebooting', 'deleting'].includes(vm.actual_state)
	).length;
	const status =
		total === 0
			? 'unknown'
			: failedCount > 0
				? 'failed'
				: unknownCount > 0
					? 'unknown'
					: transitionalCount > 0
						? 'degraded'
						: 'healthy';

	return {
		key: 'workloads',
		label: 'Workload state',
		status,
		value: `${runningCount}/${total || 0} running`,
		detail:
			total === 0
				? 'No workloads reported yet'
				: `${failedCount} failed, ${unknownCount} unknown, ${transitionalCount} transitioning`
	};
}

function buildAlertHealthTile(events: Event[]): HealthTileModel {
	const failedCount = events.filter((event) => event.status === 'failed').length;
	const pendingCount = events.filter((event) => event.status === 'pending').length;
	const status = failedCount > 0 ? 'failed' : pendingCount > 0 ? 'warning' : 'healthy';

	return {
		key: 'alerts',
		label: 'Alerts',
		status,
		value: failedCount > 0 ? `${failedCount} active` : pendingCount > 0 ? `${pendingCount} pending` : 'Quiet',
		detail:
			failedCount > 0
				? 'Recent failed events need review'
				: pendingCount > 0
					? 'Background work is still settling'
					: 'No active failure alerts'
	};
}

function buildStorageHealthTile(storagePools: StoragePool[]): HealthTileModel {
	const status = buildStorageHealthTone(storagePools);
	const degradedCount = storagePools.filter((pool) =>
		['degraded', 'offline', 'unknown'].includes(pool.status.trim().toLowerCase())
	).length;
	const failedCount = storagePools.filter((pool) =>
		['failed', 'error'].includes(pool.status.trim().toLowerCase())
	).length;

	return {
		key: 'storage',
		label: 'Storage health',
		status,
		value:
			storagePools.length === 0
				? 'Unknown'
				: failedCount > 0
					? `${failedCount} failed`
					: degradedCount > 0
						? `${degradedCount} degraded`
						: 'Healthy',
		detail:
			storagePools.length === 0
				? 'No storage pools reported yet'
				: `${storagePools.length} pools under management`
	};
}

function buildNodeCapacityTile(nodes: NodeWithResources[]): CapacityTileModel {
	const total = nodes.length;
	const ready = nodes.filter((node) => node.status === 'online').length;
	const status = total === 0 ? 'unknown' : ready === total ? 'healthy' : ready === 0 ? 'failed' : 'degraded';

	return {
		key: 'node-capacity',
		label: 'Cluster capacity',
		used: `${ready} ready`,
		total: `${total} enrolled`,
		detail: 'Enrollment and readiness across available hosts',
		status
	};
}

function buildVmCapacityTile(vms: VM[]): CapacityTileModel {
	const total = vms.length;
	const running = vms.filter((vm) => vm.actual_state === 'running').length;
	const status = total === 0 ? 'unknown' : running === total ? 'healthy' : running === 0 ? 'degraded' : 'warning';

	return {
		key: 'vm-capacity',
		label: 'Workload density',
		used: `${running} active`,
		total: `${total} total`,
		detail: 'Running versus total virtual machines',
		status
	};
}

function buildStorageCapacityTile(storagePools: StoragePool[]): CapacityTileModel {
	const totalCapacity = storagePools.reduce((sum, pool) => sum + (pool.capacity_bytes ?? 0), 0);
	const allocatable = storagePools.reduce((sum, pool) => sum + (pool.allocatable_bytes ?? 0), 0);
	const usedCapacity = Math.max(totalCapacity - allocatable, 0);
	const status = buildStorageHealthTone(storagePools);

	return {
		key: 'storage-capacity',
		label: 'Storage headroom',
		used: totalCapacity > 0 ? formatBytes(usedCapacity) : 'Unknown',
		total: totalCapacity > 0 ? formatBytes(totalCapacity) : `${storagePools.length} pools`,
		detail: 'Allocated capacity across configured storage pools',
		status
	};
}

function buildStorageHealthTone(storagePools: StoragePool[]): ShellTone {
	if (storagePools.length === 0) {
		return 'unknown';
	}

	if (
		storagePools.some((pool) => ['failed', 'error'].includes(pool.status.trim().toLowerCase()))
	) {
		return 'failed';
	}

	if (
		storagePools.some((pool) => ['degraded', 'offline', 'unknown'].includes(pool.status.trim().toLowerCase()))
	) {
		return 'degraded';
	}

	return 'healthy';
}

function buildActiveAlerts(snapshot: OverviewSnapshot): string[] {
	const alerts = new Set<string>();
	const nodeErrors = snapshot.nodes.filter((node) => node.status === 'error').length;
	const nodeOffline = snapshot.nodes.filter((node) => node.status === 'offline').length;
	const vmFailed = snapshot.vms.filter((vm) => vm.actual_state === 'failed').length;
	const vmUnknown = snapshot.vms.filter((vm) => vm.actual_state === 'unknown').length;
	const poolDegraded = snapshot.storagePools.filter((pool) =>
		['degraded', 'offline', 'unknown', 'failed', 'error'].includes(pool.status.trim().toLowerCase())
	).length;

	if (nodeErrors > 0) {
		alerts.add(`${nodeErrors} node reporting errors`);
	}

	if (nodeOffline > 0) {
		alerts.add(`${nodeOffline} node offline or unreachable`);
	}

	if (vmFailed > 0) {
		alerts.add(`${vmFailed} VM failed to reach its desired state`);
	}

	if (vmUnknown > 0) {
		alerts.add(`${vmUnknown} VM reporting unknown state`);
	}

	if (poolDegraded > 0) {
		alerts.add(`${poolDegraded} storage pool below healthy capacity posture`);
	}

	for (const event of snapshot.events) {
		if (event.status === 'failed' && event.message) {
			alerts.add(event.message);
		}
	}

	return Array.from(alerts).slice(0, 6);
}

function combineTones(tones: ShellTone[]): ShellTone {
	if (tones.includes('failed')) {
		return 'failed';
	}

	if (tones.includes('degraded')) {
		return 'degraded';
	}

	if (tones.includes('warning')) {
		return 'warning';
	}

	if (tones.includes('unknown')) {
		return 'unknown';
	}

	return 'healthy';
}

function titleizeTone(value: ShellTone): string {
	return value.charAt(0).toUpperCase() + value.slice(1);
}

function formatBytes(value: number): string {
	if (value <= 0) {
		return '0 B';
	}

	const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
	let unitIndex = 0;
	let current = value;

	while (current >= 1024 && unitIndex < units.length - 1) {
		current /= 1024;
		unitIndex += 1;
	}

	const digits = current >= 100 || unitIndex === 0 ? 0 : 1;
	return `${current.toFixed(digits)} ${units[unitIndex]}`;
}
