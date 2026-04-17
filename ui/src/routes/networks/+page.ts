import type { PageLoad } from './$types';
import { getStoredToken } from '$lib/api/client';
import { listNetworks } from '$lib/bff/networks';

export type NetworkListItem = {
	network_id: string;
	name: string;
	scope: string;
	health: string;
	attached_vms: number;
	exposure: 'private' | 'nat' | 'public';
	policy: string;
	last_task: string;
	alerts: number;
};

export type NetworksListModel = {
	items: NetworkListItem[];
	state: 'ready' | 'empty' | 'error';
	filters: { current: Record<string, string>; applied: Record<string, string> };
	page: { page: number; pageSize: number; totalItems: number };
};

function filterNetworks(items: NetworkListItem[], current: Record<string, string>): NetworkListItem[] {
	let result = [...items];
	const query = (current.query ?? '').toLowerCase();
	if (query) {
		result = result.filter((n) => n.name.toLowerCase().includes(query) || n.scope.toLowerCase().includes(query));
	}
	const health = current.health;
	if (health && health !== 'all') {
		result = result.filter((n) => n.health.toLowerCase() === health.toLowerCase());
	}
	const exposure = current.exposure;
	if (exposure && exposure !== 'all') {
		result = result.filter((n) => n.exposure === exposure);
	}
	return result;
}

export const load: PageLoad = async ({ url }) => {
	const token = getStoredToken() ?? undefined;
	const page = Math.max(1, parseInt(url.searchParams.get('page') ?? '1', 10) || 1);
	const pageSize = 50;

	const current: Record<string, string> = {};
	const query = url.searchParams.get('query');
	const health = url.searchParams.get('health');
	const exposure = url.searchParams.get('exposure');

	if (query) current.query = query;
	if (health) current.health = health;
	if (exposure) current.exposure = exposure;

	try {
		const res = await listNetworks(token);
		const fetchedItems = (res.items ?? []) as NetworkListItem[];

		const filtered = filterNetworks(fetchedItems, current);

		const model: NetworksListModel = {
			items: filtered,
			state: filtered.length === 0 ? 'empty' : 'ready',
			filters: { current, applied: res.filters?.applied ?? current },
			page: { page, pageSize, totalItems: res.page.total_items }
		};

		return { networks: model };
	} catch (err) {
		// eslint-disable-next-line no-console
		console.error('BFF listNetworks error:', err);
		const model: NetworksListModel = {
			items: [],
			state: 'error',
			filters: { current, applied: {} },
			page: { page, pageSize, totalItems: 0 }
		};
		return { networks: model };
	}
};
