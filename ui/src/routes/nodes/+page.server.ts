import type { PageServerLoad } from './$types';

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

type NodesListModel = {
	items: NodeListItem[];
	state: 'ready' | 'empty' | 'error';
	filters: { current: Record<string, string>; applied: Record<string, string> };
	page: { page: number; pageSize: number; totalItems: number };
};

const mockNodes: NodeListItem[] = [
	{
		node_id: 'n-ber-1-c01',
		name: 'ber-1-c01',
		cluster: 'eu-west-core',
		state: 'host_ready',
		health: 'healthy',
		cpu: '62%',
		memory: '58%',
		storage: 'healthy',
		network: 'healthy',
		version: '1.4.2',
		maintenance: false,
		active_tasks: 1,
		alerts: 0
	},
	{
		node_id: 'n-ber-1-c03',
		name: 'ber-1-c03',
		cluster: 'eu-west-core',
		state: 'host_ready',
		health: 'degraded',
		cpu: '45%',
		memory: '52%',
		storage: 'healthy',
		network: 'degraded',
		version: '1.4.2',
		maintenance: false,
		active_tasks: 0,
		alerts: 1
	},
	{
		node_id: 'n-ams-1-n01',
		name: 'ams-1-n01',
		cluster: 'eu-west-edge',
		state: 'host_ready',
		health: 'healthy',
		cpu: '78%',
		memory: '82%',
		storage: 'warning',
		network: 'healthy',
		version: '1.4.1',
		maintenance: false,
		active_tasks: 2,
		alerts: 1
	},
	{
		node_id: 'n-ams-1-n02',
		name: 'ams-1-n02',
		cluster: 'eu-west-edge',
		state: 'draining',
		health: 'warning',
		cpu: '34%',
		memory: '41%',
		storage: 'healthy',
		network: 'healthy',
		version: '1.4.1',
		maintenance: true,
		active_tasks: 3,
		alerts: 0
	},
	{
		node_id: 'n-sin-1-n01',
		name: 'sin-1-n01',
		cluster: 'apac-prod',
		state: 'host_ready',
		health: 'healthy',
		cpu: '54%',
		memory: '49%',
		storage: 'healthy',
		network: 'healthy',
		version: '1.4.2',
		maintenance: false,
		active_tasks: 0,
		alerts: 0
	},
	{
		node_id: 'n-ash-1-n01',
		name: 'ash-1-n01',
		cluster: 'us-east-core',
		state: 'host_ready',
		health: 'healthy',
		cpu: '45%',
		memory: '44%',
		storage: 'healthy',
		network: 'healthy',
		version: '1.4.2',
		maintenance: true,
		active_tasks: 1,
		alerts: 0
	},
	{
		node_id: 'n-sjc-1-n01',
		name: 'sjc-1-n01',
		cluster: 'us-west-dev',
		state: 'host_ready',
		health: 'degraded',
		cpu: '91%',
		memory: '87%',
		storage: 'degraded',
		network: 'warning',
		version: '1.3.9',
		maintenance: false,
		active_tasks: 4,
		alerts: 3
	},
	{
		node_id: 'n-sjc-1-n02',
		name: 'sjc-1-n02',
		cluster: 'us-west-dev',
		state: 'host_ready',
		health: 'healthy',
		cpu: '23%',
		memory: '31%',
		storage: 'healthy',
		network: 'healthy',
		version: '1.3.9',
		maintenance: false,
		active_tasks: 0,
		alerts: 0
	}
];

function filterNodes(items: NodeListItem[], current: Record<string, string>): NodeListItem[] {
	let result = [...items];
	const query = (current.query ?? '').toLowerCase();
	if (query) {
		result = result.filter(
			(n) => n.name.toLowerCase().includes(query) || n.cluster.toLowerCase().includes(query)
		);
	}
	const state = current.state;
	if (state && state !== 'all') {
		result = result.filter((n) => n.state.toLowerCase() === state.toLowerCase());
	}
	const maintenance = current.maintenance;
	if (maintenance && maintenance !== 'all') {
		result = result.filter((n) => (n.maintenance ? 'true' : 'false') === maintenance);
	}
	return result;
}

export const load: PageServerLoad = async ({ url }) => {
	const page = Math.max(1, parseInt(url.searchParams.get('page') ?? '1', 10) || 1);
	const pageSize = 50;

	const current: Record<string, string> = {};
	const query = url.searchParams.get('query');
	const state = url.searchParams.get('state');
	const maintenance = url.searchParams.get('maintenance');

	if (query) current.query = query;
	if (state) current.state = state;
	if (maintenance) current.maintenance = maintenance;

	const filtered = filterNodes(mockNodes, current);

	const model: NodesListModel = {
		items: filtered,
		state: filtered.length === 0 ? 'empty' : 'ready',
		filters: { current, applied: current },
		page: { page, pageSize, totalItems: filtered.length }
	};

	return { nodes: model };
};
