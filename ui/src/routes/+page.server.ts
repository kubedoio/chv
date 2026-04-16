import type { PageServerLoad } from './$types';
import { loadOverview } from '$lib/bff/overview';

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

export const load: PageServerLoad = async ({ cookies }) => {
	const token = cookies.get('chv_session') ?? undefined;

	try {
		const res = await loadOverview(token);
		const overview: OverviewModel = {
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
			alerts: (res.alerts ?? []).map((a: { summary: string; scope: string; severity: string }) => ({
				summary: a.summary,
				scope: a.scope,
				severity: a.severity as OverviewAlert['severity']
			})),
			recent_tasks: (res.recent_tasks ?? []).map(
				(t: {
					task_id: string;
					status: string;
					summary: string;
					resource_kind: string;
					resource_id: string;
					operation: string;
					started_unix_ms: number;
				}) => ({
					task_id: t.task_id,
					status: t.status,
					summary: t.summary,
					resource_kind: t.resource_kind,
					resource_id: t.resource_id,
					operation: t.operation,
					started_unix_ms: t.started_unix_ms
				})
			),
			state: 'ready'
		};
		return { overview };
	} catch (err) {
		// eslint-disable-next-line no-console
		console.error('BFF loadOverview error:', err);
		const overview: OverviewModel = {
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
		};
		return { overview };
	}
};
