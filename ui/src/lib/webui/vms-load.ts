import { listVms } from '$lib/bff/vms';
import type { ListVmsRequest, VmListItem } from '$lib/bff/types';

export type VmsListModel = {
	items: VmListItem[];
	state: 'ready' | 'empty' | 'error';
	filters: {
		current: Record<string, string>;
		applied: Record<string, string>;
	};
	page: {
		page: number;
		pageSize: number;
		totalItems: number;
	};
};

export interface VmsLoadDeps {
	searchParams: URLSearchParams;
	token: string | undefined;
}

export async function buildVmsLoad({ searchParams, token }: VmsLoadDeps): Promise<VmsListModel> {
	const page = Math.max(1, parseInt(searchParams.get('page') ?? '1', 10));
	const page_size = 50;

	const filters: Record<string, string> = {};
	const currentFilters: Record<string, string> = {};

	const query = searchParams.get('query');
	if (query) {
		filters.query = query;
		currentFilters.query = query;
	}

	const powerState = searchParams.get('powerState');
	if (powerState && powerState !== 'all') {
		filters.powerState = powerState;
		currentFilters.powerState = powerState;
	}

	const health = searchParams.get('health');
	if (health && health !== 'all') {
		filters.health = health;
		currentFilters.health = health;
	}

	const nodeId = searchParams.get('nodeId');
	if (nodeId) {
		filters.nodeId = nodeId;
		currentFilters.nodeId = nodeId;
	}

	const req: ListVmsRequest = {
		page,
		page_size,
		filters
	};

	try {
		const res = await listVms(req, token);
		const fetchedItems = res.items;

		return {
			items: fetchedItems,
			state: fetchedItems.length === 0 ? 'empty' : 'ready',
			filters: {
				current: currentFilters,
				applied: res.filters.applied
			},
			page: {
				page: res.page.page,
				pageSize: res.page.page_size,
				totalItems: res.page.total_items
			}
		};
	} catch (err) {
		// eslint-disable-next-line no-console
		console.error('BFF listVms error:', err);
		return {
			items: [],
			state: 'error',
			filters: {
				current: currentFilters,
				applied: {}
			},
			page: {
				page,
				pageSize: page_size,
				totalItems: 0
			}
		};
	}
}
