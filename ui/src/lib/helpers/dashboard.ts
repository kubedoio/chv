import type { NodeWithResources, VM } from '$lib/api/types';
import type { OverviewResponse } from '$lib/bff/types';
import type { ShellTone } from '$lib/shell/app-shell';
import { getTaskStatusMeta } from '$lib/webui/tasks';

// ------------------------------------------------------------------
// Overview model types
// ------------------------------------------------------------------

export type OverviewAlert = {
	summary: string;
	scope: string;
	severity: 'critical' | 'warning' | 'info';
};

export type OverviewTask = {
	task_id: string;
	status: string;
	summary: string;
	resource_kind: string;
	resource_id: string;
	operation: string;
	started_unix_ms: number;
};

export type OverviewModel = {
	clusters_total: number;
	clusters_healthy: number;
	clusters_degraded: number;
	nodes_total: number;
	nodes_degraded: number;
	vms_running: number;
	vms_total: number;
	active_tasks: number;
	unresolved_alerts: number;
	maintenance_nodes: number;
	capacity_hotspots: number;
	cpu_usage_percent: number;
	memory_usage_percent: number;
	storage_usage_percent: number;
	alerts: OverviewAlert[];
	recent_tasks: OverviewTask[];
	state: 'ready' | 'loading' | 'empty' | 'error';
};

const EMPTY_OVERVIEW: Omit<OverviewModel, 'state'> = {
	clusters_total: 0,
	clusters_healthy: 0,
	clusters_degraded: 0,
	nodes_total: 0,
	nodes_degraded: 0,
	vms_running: 0,
	vms_total: 0,
	active_tasks: 0,
	unresolved_alerts: 0,
	maintenance_nodes: 0,
	capacity_hotspots: 0,
	cpu_usage_percent: 0,
	memory_usage_percent: 0,
	storage_usage_percent: 0,
	alerts: [],
	recent_tasks: []
};

export function createOverview(state: OverviewModel['state']): OverviewModel {
	return { ...EMPTY_OVERVIEW, state };
}

export function toOverviewModel(res: OverviewResponse): OverviewModel {
	const model: OverviewModel = {
		clusters_total: res.clusters_total ?? 0,
		clusters_healthy: res.clusters_healthy ?? 0,
		clusters_degraded: res.clusters_degraded ?? 0,
		nodes_total: res.nodes_total ?? 0,
		nodes_degraded: res.nodes_degraded ?? 0,
		vms_running: res.vms_running ?? 0,
		vms_total: res.vms_total ?? 0,
		active_tasks: res.active_tasks ?? 0,
		unresolved_alerts: res.unresolved_alerts ?? 0,
		maintenance_nodes: res.maintenance_nodes ?? 0,
		capacity_hotspots: res.capacity_hotspots ?? 0,
		cpu_usage_percent: res.cpu_usage_percent ?? 0,
		memory_usage_percent: res.memory_usage_percent ?? 0,
		storage_usage_percent: res.storage_usage_percent ?? 0,
		alerts: (res.alerts ?? []).map((alert) => ({
			summary: alert.summary,
			scope: alert.scope,
			severity: alert.severity as OverviewAlert['severity']
		})),
		recent_tasks: (res.recent_tasks ?? []).map((task) => ({
			task_id: task.task_id,
			status: task.status,
			summary: task.summary,
			resource_kind: task.resource_kind,
			resource_id: task.resource_id,
			operation: task.operation,
			started_unix_ms: task.started_unix_ms
		})),
		state: 'ready'
	};

	const hasData =
		model.clusters_total > 0 ||
		model.nodes_total > 0 ||
		model.vms_total > 0 ||
		model.active_tasks > 0 ||
		model.unresolved_alerts > 0 ||
		model.alerts.length > 0 ||
		model.recent_tasks.length > 0;

	if (!hasData) {
		model.state = 'empty';
	}

	return model;
}

// ------------------------------------------------------------------
// Panel helpers
// ------------------------------------------------------------------

export type PanelId = 'briefing' | 'attention' | 'pipeline' | 'capacity';

export const dashboardPanelStorageKey = 'chv.fleet-overview.panels.v1';

export const defaultPanelState: Record<PanelId, boolean> = {
	briefing: true,
	attention: true,
	pipeline: true,
	capacity: true
};

export const dashboardPanels: { id: PanelId; label: string }[] = [
	{ id: 'briefing', label: 'Fleet Briefing' },
	{ id: 'attention', label: 'Immediate Attention' },
	{ id: 'pipeline', label: 'Operation Pipeline' },
	{ id: 'capacity', label: 'Capacity Pressure' }
];

// ------------------------------------------------------------------
// Display helpers
// ------------------------------------------------------------------

export function getPressureState(value: number): string {
	if (value >= 90) return 'Critical';
	if (value >= 75) return 'Pressure';
	if (value >= 45) return 'Warm';
	return 'Idle';
}

export interface RecentTaskViewModel {
	task_id: string;
	operation: string;
	summary: string;
	status: string;
	resource_kind: string;
	resource_id: string;
	started_at: string;
	tone: ShellTone;
}

export function formatRecentTasks(tasks: OverviewTask[]): RecentTaskViewModel[] {
	return tasks.map((t) => {
		const meta = getTaskStatusMeta(t.status);
		return {
			task_id: t.task_id,
			operation: t.operation,
			summary: t.summary,
			status: t.status,
			resource_kind: t.resource_kind,
			resource_id: t.resource_id,
			started_at: new Date(t.started_unix_ms).toLocaleTimeString([], {
				hour: '2-digit',
				minute: '2-digit'
			}),
			tone: meta.tone as ShellTone
		};
	});
}

export function getActiveTopologyResourceIds(tasks: RecentTaskViewModel[]): string[] {
	return tasks
		.filter((task) =>
			['queued', 'running', 'pending', 'accepted', 'in_progress'].includes(task.status.toLowerCase())
		)
		.map((task) => task.resource_id)
		.filter(Boolean);
}

export interface FleetBriefingItem {
	label: string;
	value: string;
	note: string;
}

export function buildFleetBriefing(
	overview: OverviewModel,
	nodes: NodeWithResources[],
	vms: VM[]
): FleetBriefingItem[] {
	return [
		{
			label: 'Control-plane reach',
			value: `${nodes.length} reporting nodes`,
			note:
				overview.nodes_degraded > 0
					? `${overview.nodes_degraded} node signals need review`
					: 'Fleet reporting is stable'
		},
		{
			label: 'Workload posture',
			value: `${overview.vms_running} active of ${overview.vms_total || vms.length}`,
			note:
				overview.unresolved_alerts > 0
					? `${overview.unresolved_alerts} unresolved operator alerts`
					: 'No blocking workload alarms'
		},
		{
			label: 'Execution queue',
			value: `${overview.active_tasks || 0} active operations`,
			note:
				overview.recent_tasks.length > 0
					? `Latest activity: ${overview.recent_tasks[0].operation}`
					: 'No recent task churn'
		}
	];
}

export interface PressureCard {
	label: string;
	value: string;
	width: number;
	state: string;
}

export function buildPressureCards(overview: OverviewModel): PressureCard[] {
	return [
		{
			label: 'CPU envelope',
			value: `${Math.round(overview.cpu_usage_percent || 0)}%`,
			width: overview.cpu_usage_percent || 0,
			state: getPressureState(overview.cpu_usage_percent || 0)
		},
		{
			label: 'Memory envelope',
			value: `${Math.round(overview.memory_usage_percent || 0)}%`,
			width: overview.memory_usage_percent || 0,
			state: getPressureState(overview.memory_usage_percent || 0)
		},
		{
			label: 'Storage pressure',
			value: `${Math.round(overview.storage_usage_percent || 0)}%`,
			width: overview.storage_usage_percent || 0,
			state: getPressureState(overview.storage_usage_percent || 0)
		}
	];
}
