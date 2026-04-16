import type { PageServerLoad } from './$types';

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
	alerts: OverviewAlert[];
	recent_tasks: OverviewTask[];
	state: 'ready' | 'loading' | 'empty' | 'error';
};

function mockOverview(): OverviewModel {
	return {
		clusters_total: 5,
		clusters_healthy: 3,
		clusters_degraded: 2,
		nodes_total: 50,
		nodes_degraded: 4,
		vms_running: 2847,
		vms_total: 3152,
		active_tasks: 12,
		unresolved_alerts: 8,
		maintenance_nodes: 3,
		capacity_hotspots: 2,
		alerts: [
			{ summary: 'Storage pressure on eu-west-edge', scope: 'Cluster', severity: 'critical' },
			{ summary: 'Version skew detected in us-west-dev', scope: 'Cluster', severity: 'warning' },
			{ summary: 'Node ber-1-c03 network degraded', scope: 'Node', severity: 'warning' },
			{ summary: 'VM vm-8842 failed reboot task', scope: 'VM', severity: 'critical' },
			{ summary: 'Scheduling paused on Ashburn core', scope: 'Cluster', severity: 'info' }
		],
		recent_tasks: [
			{
				task_id: 't-1001',
				status: 'running',
				summary: 'Resize volume vol-9912',
				resource_kind: 'volume',
				resource_id: 'vol-9912',
				operation: 'resize',
				started_unix_ms: Date.now() - 1000 * 60 * 5
			},
			{
				task_id: 't-1002',
				status: 'failed',
				summary: 'Reboot VM vm-8842',
				resource_kind: 'vm',
				resource_id: 'vm-8842',
				operation: 'reboot',
				started_unix_ms: Date.now() - 1000 * 60 * 32
			},
			{
				task_id: 't-1003',
				status: 'succeeded',
				summary: 'Migrate VM vm-1104 to node ber-1-c09',
				resource_kind: 'vm',
				resource_id: 'vm-1104',
				operation: 'migrate',
				started_unix_ms: Date.now() - 1000 * 60 * 120
			},
			{
				task_id: 't-1004',
				status: 'running',
				summary: 'Drain node ams-1-n02',
				resource_kind: 'node',
				resource_id: 'ams-1-n02',
				operation: 'drain',
				started_unix_ms: Date.now() - 1000 * 60 * 18
			}
		],
		state: 'ready'
	};
}

export const load: PageServerLoad = async () => {
	try {
		const overview = mockOverview();
		return { overview };
	} catch {
		return {
			overview: {
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
				alerts: [],
				recent_tasks: [],
				state: 'error'
			} as OverviewModel
		};
	}
};
