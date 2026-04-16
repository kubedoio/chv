import type { PageServerLoad } from './$types';
import { listClusters } from '$lib/bff/clusters';

export type ClusterListItem = {
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

export const load: PageServerLoad = async ({ url, cookies }) => {
	const token = cookies.get('chv_session') ?? undefined;
	const page = Math.max(1, parseInt(url.searchParams.get('page') ?? '1', 10) || 1);
	const pageSize = 50;

	const current: Record<string, string> = {};
	const query = url.searchParams.get('query');
	const state = url.searchParams.get('state');
	const maintenance = url.searchParams.get('maintenance');

	if (query) current.query = query;
	if (state) current.state = state;
	if (maintenance) current.maintenance = maintenance;

	try {
		const res = await listClusters(token);
		const items = (res.items ?? []) as ClusterListItem[];
		const filtered = filterClusters(items, current);

		const model: ClustersListModel = {
			items: filtered,
			state: filtered.length === 0 ? 'empty' : 'ready',
			filters: { current, applied: res.filters?.applied ?? current },
			page: { page, pageSize, totalItems: res.page.total_items }
		};

		return { clusters: model };
	} catch (err) {
		// eslint-disable-next-line no-console
		console.error('BFF listClusters error:', err);
		const model: ClustersListModel = {
			items: [],
			state: 'error',
			filters: { current, applied: {} },
			page: { page, pageSize, totalItems: 0 }
		};
		return { clusters: model };
	}
};
