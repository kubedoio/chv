import type { PageServerLoad } from './$types';
import { getMaintenance } from '$lib/bff/maintenance';

export type MaintenanceNode = {
	node_id: string;
	name: string;
	cluster: string;
	state: 'in_maintenance' | 'draining' | 'scheduled';
	task_id?: string;
	window_start?: string;
	window_end?: string;
};

export type MaintenanceWindow = {
	window_id: string;
	name: string;
	cluster: string;
	status: 'active' | 'scheduled' | 'completed';
	start_time: string;
	end_time: string;
	affected_nodes: number;
};

export type MaintenanceModel = {
	windows: MaintenanceWindow[];
	nodes: MaintenanceNode[];
	pending_actions: number;
	state: 'ready' | 'empty' | 'error';
	filters: { current: Record<string, string>; applied: Record<string, string> };
};

function filterNodes(items: MaintenanceNode[], current: Record<string, string>): MaintenanceNode[] {
	let result = [...items];
	const query = (current.query ?? '').toLowerCase();
	if (query) {
		result = result.filter(
			(n) => n.name.toLowerCase().includes(query) || n.cluster.toLowerCase().includes(query)
		);
	}
	const state = current.state;
	if (state && state !== 'all') {
		result = result.filter((n) => n.state === state);
	}
	return result;
}

export const load: PageServerLoad = async ({ url, cookies }) => {
	const token = cookies.get('chv_session') ?? undefined;

	const current: Record<string, string> = {};
	const query = url.searchParams.get('query');
	const state = url.searchParams.get('state');

	if (query) current.query = query;
	if (state) current.state = state;

	try {
		const res = await getMaintenance(token);
		const nodes = filterNodes((res.nodes ?? []) as MaintenanceNode[], current);
		const windows = (res.windows ?? []) as MaintenanceWindow[];

		const model: MaintenanceModel = {
			windows,
			nodes,
			pending_actions: res.pending_actions ?? 0,
			state: nodes.length === 0 && windows.length === 0 ? 'empty' : 'ready',
			filters: { current, applied: current }
		};

		return { maintenance: model };
	} catch (err) {
		// eslint-disable-next-line no-console
		console.error('BFF getMaintenance error:', err);
		const model: MaintenanceModel = {
			windows: [],
			nodes: [],
			pending_actions: 0,
			state: 'error',
			filters: { current, applied: {} }
		};
		return { maintenance: model };
	}
};
