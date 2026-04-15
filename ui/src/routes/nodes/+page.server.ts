import type { PageServerLoad } from './$types';
import { listNodes } from '$lib/bff/nodes';
import { BFFError } from '$lib/bff/client';
import type { NodeListItem } from '$lib/bff/types';

type NodesListModel = {
	items: NodeListItem[];
	state: 'ready' | 'empty' | 'error';
	filters: { current: Record<string, string>; applied: Record<string, string> };
	page: { page: number; pageSize: number; totalItems: number };
};

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

	const req = { page, page_size: pageSize, filters: current };

	try {
		const res = await listNodes(req, token);
		const items = res.items ?? [];
		const model: NodesListModel = {
			items,
			state: items.length === 0 ? 'empty' : 'ready',
			filters: {
				current,
				applied: res.filters?.applied ?? {}
			},
			page: {
				page: res.page?.page ?? page,
				pageSize: res.page?.page_size ?? pageSize,
				totalItems: res.page?.total_items ?? items.length
			}
		};
		return { nodes: model };
	} catch (err) {
		// eslint-disable-next-line no-console
		console.error('BFF listNodes error:', err);
		const model: NodesListModel = {
			items: [],
			state: 'error',
			filters: {
				current,
				applied: {}
			},
			page: { page, pageSize, totalItems: 0 }
		};
		return { nodes: model };
	}
};
