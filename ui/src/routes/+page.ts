import { browser } from '$app/environment';
import { getStoredToken } from '$lib/api/client';
import type { OverviewResponse } from '$lib/bff/types';
import type { PageLoad } from './$types';

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
	alerts: [],
	recent_tasks: []
};

function createOverview(state: OverviewModel['state']): OverviewModel {
	return {
		...EMPTY_OVERVIEW,
		state
	};
}

function toOverviewModel(res: OverviewResponse): OverviewModel {
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

export const load: PageLoad = async ({ fetch }) => {
	// In static builds, avoid server-pass data fetches that can become HTML fallback responses.
	if (!browser) {
		return { overview: createOverview('loading') };
	}

	const token = getStoredToken();
	const headers = new Headers();
	headers.set('Content-Type', 'application/json');
	if (token) {
		headers.set('Authorization', `Bearer ${token}`);
	}

	try {
		const response = await fetch('/api/v1/overview', {
			method: 'POST',
			headers,
			body: JSON.stringify({}),
			cache: 'no-store'
		});

		if (!response.ok) {
			return { overview: createOverview('error') };
		}

		const contentType = response.headers.get('content-type')?.toLowerCase() ?? '';
		if (!contentType.includes('application/json')) {
			return { overview: createOverview('error') };
		}

		const payload = (await response.json()) as OverviewResponse;
		return { overview: toOverviewModel(payload) };
	} catch {
		return { overview: createOverview('error') };
	}
};
