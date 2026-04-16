import type { PageLoad } from './$types';
import { getStoredToken } from '$lib/api/client';
import { listEvents } from '$lib/bff/events';

export type EventListItem = {
	event_id: string;
	severity: 'critical' | 'warning' | 'info';
	type: string;
	resource_kind: string;
	resource_id: string;
	resource_name: string;
	summary: string;
	state: 'open' | 'acknowledged' | 'resolved';
	occurred_at: string;
};

export type EventsListModel = {
	items: EventListItem[];
	state: 'ready' | 'empty' | 'error';
	filters: { current: Record<string, string>; applied: Record<string, string> };
	page: { page: number; pageSize: number; totalItems: number };
};

function filterEvents(items: EventListItem[], current: Record<string, string>): EventListItem[] {
	let result = [...items];
	const query = (current.query ?? '').toLowerCase();
	if (query) {
		result = result.filter(
			(e) =>
				e.summary.toLowerCase().includes(query) ||
				e.resource_name.toLowerCase().includes(query) ||
				e.type.toLowerCase().includes(query)
		);
	}
	const severity = current.severity;
	if (severity && severity !== 'all') {
		result = result.filter((e) => e.severity === severity);
	}
	const state = current.state;
	if (state && state !== 'all') {
		result = result.filter((e) => e.state === state);
	}
	return result;
}

export const load: PageLoad = async ({ url }) => {
	const token = getStoredToken() ?? undefined;
	const page = Math.max(1, parseInt(url.searchParams.get('page') ?? '1', 10) || 1);
	const pageSize = 50;

	const current: Record<string, string> = {};
	const query = url.searchParams.get('query');
	const severity = url.searchParams.get('severity');
	const state = url.searchParams.get('state');

	if (query) current.query = query;
	if (severity) current.severity = severity;
	if (state) current.state = state;

	try {
		const res = await listEvents(token);
		const items = (res.items ?? []) as EventListItem[];
		const filtered = filterEvents(items, current);

		const model: EventsListModel = {
			items: filtered,
			state: filtered.length === 0 ? 'empty' : 'ready',
			filters: { current, applied: res.filters?.applied ?? current },
			page: { page, pageSize, totalItems: res.page.total_items }
		};

		return { events: model };
	} catch (err) {
		// eslint-disable-next-line no-console
		console.error('BFF listEvents error:', err);
		const model: EventsListModel = {
			items: [],
			state: 'error',
			filters: { current, applied: {} },
			page: { page, pageSize, totalItems: 0 }
		};
		return { events: model };
	}
};
