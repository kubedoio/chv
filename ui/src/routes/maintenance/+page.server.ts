import type { PageServerLoad } from './$types';

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

const mockWindows: MaintenanceWindow[] = [
	{
		window_id: 'mw-1',
		name: 'Berlin-1 kernel upgrade',
		cluster: 'eu-west-core',
		status: 'active',
		start_time: '2026-04-16T14:00:00Z',
		end_time: '2026-04-16T18:00:00Z',
		affected_nodes: 4
	},
	{
		window_id: 'mw-2',
		name: 'Amsterdam-1 storage patch',
		cluster: 'eu-west-edge',
		status: 'scheduled',
		start_time: '2026-04-17T02:00:00Z',
		end_time: '2026-04-17T04:00:00Z',
		affected_nodes: 2
	}
];

const mockNodes: MaintenanceNode[] = [
	{
		node_id: 'n-ber-1-c05',
		name: 'ber-1-c05',
		cluster: 'eu-west-core',
		state: 'in_maintenance',
		task_id: 't-2001',
		window_start: '2026-04-16T14:00:00Z',
		window_end: '2026-04-16T18:00:00Z'
	},
	{
		node_id: 'n-ber-1-c06',
		name: 'ber-1-c06',
		cluster: 'eu-west-core',
		state: 'draining',
		task_id: 't-2002',
		window_start: '2026-04-16T14:00:00Z',
		window_end: '2026-04-16T18:00:00Z'
	},
	{
		node_id: 'n-ash-1-n01',
		name: 'ash-1-n01',
		cluster: 'us-east-core',
		state: 'in_maintenance',
		window_start: '2026-04-15T10:00:00Z',
		window_end: '2026-04-15T14:00:00Z'
	},
	{
		node_id: 'n-ams-1-n03',
		name: 'ams-1-n03',
		cluster: 'eu-west-edge',
		state: 'scheduled',
		window_start: '2026-04-17T02:00:00Z',
		window_end: '2026-04-17T04:00:00Z'
	}
];

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

export const load: PageServerLoad = async ({ url }) => {
	const current: Record<string, string> = {};
	const query = url.searchParams.get('query');
	const state = url.searchParams.get('state');

	if (query) current.query = query;
	if (state) current.state = state;

	const filteredNodes = filterNodes(mockNodes, current);

	const model: MaintenanceModel = {
		windows: mockWindows,
		nodes: filteredNodes,
		pending_actions: mockNodes.filter((n) => n.state === 'draining').length,
		state: mockNodes.length === 0 && mockWindows.length === 0 ? 'empty' : 'ready',
		filters: { current, applied: current }
	};

	return { maintenance: model };
};
