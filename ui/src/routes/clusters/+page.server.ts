import type { PageServerLoad } from './$types';

type ClusterListItem = {
	cluster_id: string;
	name: string;
	datacenter: string;
	node_count: number;
	state: string;
	maintenance: boolean;
	version: string;
	version_skew: boolean;
	cpu_percent: number;
	memory_percent: number;
	storage_percent: number;
	active_tasks: number;
	alerts: number;
	top_issue?: string;
};

type ClustersListModel = {
	items: ClusterListItem[];
	state: 'ready' | 'loading' | 'empty' | 'error' | 'degraded';
	filters: { current: Record<string, string>; applied: Record<string, string> };
	page: { page: number; pageSize: number; totalItems: number };
};

const mockClusters: ClusterListItem[] = [
	{
		cluster_id: 'c-ber-1',
		name: 'eu-west-core',
		datacenter: 'Berlin-1',
		node_count: 12,
		state: 'healthy',
		maintenance: false,
		version: '1.4.2',
		version_skew: false,
		cpu_percent: 62,
		memory_percent: 58,
		storage_percent: 45,
		active_tasks: 1,
		alerts: 0
	},
	{
		cluster_id: 'c-ams-1',
		name: 'eu-west-edge',
		datacenter: 'Amsterdam-1',
		node_count: 8,
		state: 'degraded',
		maintenance: false,
		version: '1.4.1',
		version_skew: true,
		cpu_percent: 78,
		memory_percent: 82,
		storage_percent: 71,
		active_tasks: 3,
		alerts: 2,
		top_issue: 'Storage pressure'
	},
	{
		cluster_id: 'c-sin-1',
		name: 'apac-prod',
		datacenter: 'Singapore-1',
		node_count: 16,
		state: 'healthy',
		maintenance: false,
		version: '1.4.2',
		version_skew: false,
		cpu_percent: 54,
		memory_percent: 49,
		storage_percent: 52,
		active_tasks: 0,
		alerts: 0
	},
	{
		cluster_id: 'c-ash-1',
		name: 'us-east-core',
		datacenter: 'Ashburn-1',
		node_count: 10,
		state: 'warning',
		maintenance: true,
		version: '1.4.2',
		version_skew: false,
		cpu_percent: 45,
		memory_percent: 44,
		storage_percent: 38,
		active_tasks: 2,
		alerts: 1,
		top_issue: 'Scheduling paused'
	},
	{
		cluster_id: 'c-sjc-1',
		name: 'us-west-dev',
		datacenter: 'San Jose-1',
		node_count: 4,
		state: 'degraded',
		maintenance: false,
		version: '1.3.9',
		version_skew: true,
		cpu_percent: 91,
		memory_percent: 87,
		storage_percent: 83,
		active_tasks: 4,
		alerts: 5,
		top_issue: 'Version skew'
	}
];

function filterClusters(items: ClusterListItem[], current: Record<string, string>): ClusterListItem[] {
	let result = [...items];
	const query = (current.query ?? '').toLowerCase();
	if (query) {
		result = result.filter(
			(c) => c.name.toLowerCase().includes(query) || c.datacenter.toLowerCase().includes(query)
		);
	}
	const state = current.state;
	if (state && state !== 'all') {
		result = result.filter((c) => c.state.toLowerCase() === state.toLowerCase());
	}
	const maintenance = current.maintenance;
	if (maintenance && maintenance !== 'all') {
		result = result.filter((c) => (c.maintenance ? 'true' : 'false') === maintenance);
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

	const filtered = filterClusters(mockClusters, current);

	const model: ClustersListModel = {
		items: filtered,
		state: filtered.length === 0 ? 'empty' : 'ready',
		filters: { current, applied: current },
		page: { page, pageSize, totalItems: filtered.length }
	};

	return { clusters: model };
};
