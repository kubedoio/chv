import type { PageServerLoad } from './$types';

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

const mockEvents: EventListItem[] = [
	{
		event_id: 'e-101',
		severity: 'critical',
		type: 'capacity',
		resource_kind: 'cluster',
		resource_id: 'c-ams-1',
		resource_name: 'eu-west-edge',
		summary: 'Storage pressure exceeded 85% threshold',
		state: 'open',
		occurred_at: '2026-04-16T14:23:00Z'
	},
	{
		event_id: 'e-102',
		severity: 'warning',
		type: 'version',
		resource_kind: 'cluster',
		resource_id: 'c-sjc-1',
		resource_name: 'us-west-dev',
		summary: 'Version skew detected across node fleet',
		state: 'open',
		occurred_at: '2026-04-16T13:45:00Z'
	},
	{
		event_id: 'e-103',
		severity: 'warning',
		type: 'network',
		resource_kind: 'node',
		resource_id: 'n-ber-1-c03',
		resource_name: 'ber-1-c03',
		summary: 'Network interface degraded on host',
		state: 'acknowledged',
		occurred_at: '2026-04-16T12:10:00Z'
	},
	{
		event_id: 'e-104',
		severity: 'critical',
		type: 'lifecycle',
		resource_kind: 'vm',
		resource_id: 'vm-8842',
		resource_name: 'prod-api-42',
		summary: 'Reboot task failed after 3 retries',
		state: 'open',
		occurred_at: '2026-04-16T11:52:00Z'
	},
	{
		event_id: 'e-105',
		severity: 'info',
		type: 'maintenance',
		resource_kind: 'cluster',
		resource_id: 'c-ash-1',
		resource_name: 'us-east-core',
		summary: 'Scheduling paused for planned maintenance',
		state: 'acknowledged',
		occurred_at: '2026-04-16T10:00:00Z'
	},
	{
		event_id: 'e-106',
		severity: 'warning',
		type: 'volume',
		resource_kind: 'volume',
		resource_id: 'vol-4451',
		resource_name: 'db-primary-vol',
		summary: 'Volume attach stalled on node ams-1-n02',
		state: 'open',
		occurred_at: '2026-04-16T09:34:00Z'
	},
	{
		event_id: 'e-107',
		severity: 'info',
		type: 'task',
		resource_kind: 'node',
		resource_id: 'n-ams-1-n02',
		resource_name: 'ams-1-n02',
		summary: 'Drain completed successfully',
		state: 'resolved',
		occurred_at: '2026-04-15T22:15:00Z'
	}
];

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

export const load: PageServerLoad = async ({ url }) => {
	const page = Math.max(1, parseInt(url.searchParams.get('page') ?? '1', 10) || 1);
	const pageSize = 50;

	const current: Record<string, string> = {};
	const query = url.searchParams.get('query');
	const severity = url.searchParams.get('severity');
	const state = url.searchParams.get('state');

	if (query) current.query = query;
	if (severity) current.severity = severity;
	if (state) current.state = state;

	const filtered = filterEvents(mockEvents, current);

	const model: EventsListModel = {
		items: filtered,
		state: filtered.length === 0 ? 'empty' : 'ready',
		filters: { current, applied: current },
		page: { page, pageSize, totalItems: filtered.length }
	};

	return { events: model };
};
