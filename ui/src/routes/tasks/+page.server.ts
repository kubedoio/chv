import type { PageServerLoad } from './$types';
import { listTasks } from '$lib/bff/tasks';
import type { ListTasksRequest, TaskListItem } from '$lib/bff/types';

const PAGE_SIZE = 50;
const DEFAULT_WINDOW = '7d';

export const load: PageServerLoad = async ({ url, cookies }) => {
	const token = cookies.get('chv_session') ?? undefined;
	const page = Math.max(Number(url.searchParams.get('page') ?? '1') || 1, 1);

	const current: Record<string, string> = {
		status: url.searchParams.get('status')?.trim() || 'all',
		resourceKind: url.searchParams.get('resourceKind')?.trim() || 'all',
		query: url.searchParams.get('query')?.trim() || '',
		window: url.searchParams.get('window')?.trim() || DEFAULT_WINDOW
	};

	const filters: Record<string, string> = {};
	if (current.status !== 'all') filters.status = current.status;
	if (current.resourceKind !== 'all') filters.resource_kind = current.resourceKind;
	if (current.query) filters.query = current.query;
	if (current.window !== 'all') filters.window = current.window;

	const req: ListTasksRequest = {
		page,
		page_size: PAGE_SIZE,
		filters
	};

	const options = {
		statuses: ['queued', 'running', 'succeeded', 'failed', 'cancelled'] as string[],
		resourceKinds: [] as string[],
		windows: ['active', '24h', '7d', '30d', 'all'] as string[]
	};

	try {
		const res = await listTasks(req, token);
		options.resourceKinds = Array.from(new Set(res.items.map((i) => i.resource_kind))).sort();

		const applied = res.filters?.applied ?? {};
		const state: 'ready' | 'empty' | 'error' = res.items.length > 0 ? 'ready' : 'empty';

		return {
			tasks: {
				items: res.items,
				state,
				filters: {
					current,
					applied,
					options
				},
				page: {
					page,
					pageSize: PAGE_SIZE,
					totalItems: res.page.total_items
				}
			}
		};
	} catch (err) {
		// eslint-disable-next-line no-console
		console.error('BFF listTasks error:', err);
		return {
			tasks: {
				items: [] as TaskListItem[],
				state: 'error' as const,
				filters: {
					current,
					applied: {},
					options
				},
				page: {
					page,
					pageSize: PAGE_SIZE,
					totalItems: 0
				}
			}
		};
	}
};
