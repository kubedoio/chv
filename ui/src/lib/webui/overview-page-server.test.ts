import { describe, expect, it, vi } from 'vitest';
import { load } from '../../routes/+page.server';

vi.mock('$lib/bff/overview', () => ({
	loadOverview: vi.fn()
}));

import { loadOverview } from '$lib/bff/overview';

function createCookies(token?: string) {
	return {
		get: vi.fn().mockReturnValue(token)
	} as unknown as import('@sveltejs/kit').Cookies;
}

describe('overview page server load', () => {
	it('returns ready state with overview data', async () => {
		const mockedLoadOverview = vi.mocked(loadOverview);
		mockedLoadOverview.mockResolvedValue({
			nodes_total: 5,
			nodes_degraded: 1,
			vms_running: 10,
			vms_total: 12,
			active_tasks: 3,
			unresolved_alerts: 2,
			maintenance_nodes: 0,
			alerts: [
				{ summary: 'Disk full', scope: 'Node', severity: 'warning' }
			],
			recent_tasks: [
				{
					task_id: 'task-1',
					status: 'succeeded',
					summary: 'Deploy',
					resource_kind: 'vm',
					resource_id: 'vm-1',
					operation: 'deploy',
					started_unix_ms: 1713000000000
				}
			]
		});

		const result = await load({
			cookies: createCookies('token-123')
		} as unknown as import('../../routes/$types').PageServerLoadEvent);

		expect(mockedLoadOverview).toHaveBeenCalledWith('token-123');
		expect(result.overview.state).toBe('ready');
		expect(result.overview.nodes_total).toBe(5);
		expect(result.overview.alerts).toHaveLength(1);
		expect(result.overview.recent_tasks).toHaveLength(1);
	});

	it('returns error state when loadOverview throws', async () => {
		const mockedLoadOverview = vi.mocked(loadOverview);
		mockedLoadOverview.mockRejectedValue(new Error('BFF down'));

		const result = await load({
			cookies: createCookies('token-123')
		} as unknown as import('../../routes/$types').PageServerLoadEvent);

		expect(result.overview.state).toBe('error');
		expect(result.overview.nodes_total).toBe(0);
		expect(result.overview.alerts).toHaveLength(0);
	});
});
