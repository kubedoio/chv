import { describe, expect, it, vi } from 'vitest';

async function importRootLoad(options: {
	browser: boolean;
	token?: string | null;
	overviewResult?: Record<string, unknown>;
	overviewError?: Error;
}) {
	vi.resetModules();
	vi.doMock('$app/environment', () => ({ browser: options.browser }));
	vi.doMock('$lib/api/client', () => ({
		getStoredToken: vi.fn().mockReturnValue(options.token ?? null)
	}));
	vi.doMock('$lib/bff/overview', () => ({
		loadOverview: options.overviewError
			? vi.fn().mockRejectedValue(options.overviewError)
			: vi.fn().mockResolvedValue(options.overviewResult ?? {})
	}));
	return import('../../routes/+page');
}

describe('overview page load', () => {
	it('returns loading state during the server-side pass', async () => {
		const { load } = await importRootLoad({ browser: false });
		const result = await load({} as unknown as import('../../routes/$types').PageLoadEvent);
		expect(result.overview.state).toBe('loading');
	});

	it('returns ready state with overview data in browser', async () => {
		const { load } = await importRootLoad({
			browser: true,
			token: 'token-123',
			overviewResult: {
				clusters_total: 1,
				nodes_total: 5,
				nodes_degraded: 1,
				vms_running: 10,
				vms_total: 12,
				active_tasks: 3,
				unresolved_alerts: 2,
				maintenance_nodes: 0,
				alerts: [{ summary: 'Disk full', scope: 'Node', severity: 'warning' }],
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
			}
		});

		const result = await load({} as unknown as import('../../routes/$types').PageLoadEvent);
		expect(result.overview.state).toBe('ready');
		expect(result.overview.nodes_total).toBe(5);
		expect(result.overview.alerts).toHaveLength(1);
		expect(result.overview.recent_tasks).toHaveLength(1);
	});

	it('returns error state when overview request fails', async () => {
		const { load } = await importRootLoad({
			browser: true,
			token: 'token-123',
			overviewError: new Error('BFF down')
		});
		const result = await load({} as unknown as import('../../routes/$types').PageLoadEvent);

		expect(result.overview.state).toBe('error');
		expect(result.overview.nodes_total).toBe(0);
	});
});
