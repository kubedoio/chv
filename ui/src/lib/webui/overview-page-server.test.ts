import { describe, expect, it, vi } from 'vitest';

async function importRootLoad(options: { browser: boolean; token?: string | null }) {
	vi.resetModules();
	vi.doMock('$app/environment', () => ({ browser: options.browser }));
	vi.doMock('$lib/api/client', () => ({
		getStoredToken: vi.fn().mockReturnValue(options.token ?? null)
	}));
	return import('../../routes/+page');
}

describe('overview page load', () => {
	it('returns loading state during the server-side pass', async () => {
		const { load } = await importRootLoad({ browser: false });
		const fetcher = vi.fn();

		const result = await load({
			fetch: fetcher
		} as unknown as import('../../routes/$types').PageLoadEvent);

		expect(fetcher).not.toHaveBeenCalled();
		expect(result.overview.state).toBe('loading');
	});

	it('returns ready state with overview data in browser', async () => {
		const { load } = await importRootLoad({ browser: true, token: 'token-123' });
		const fetcher = vi.fn().mockResolvedValue(
			new Response(
				JSON.stringify({
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
				}),
				{
					status: 200,
					headers: { 'content-type': 'application/json' }
				}
			)
		);

		const result = await load({
			fetch: fetcher
		} as unknown as import('../../routes/$types').PageLoadEvent);

		expect(fetcher).toHaveBeenCalledTimes(1);
		expect(fetcher).toHaveBeenCalledWith(
			'/api/v1/overview',
			expect.objectContaining({
				method: 'POST',
				cache: 'no-store'
			})
		);

		const requestInit = fetcher.mock.calls[0][1] as RequestInit;
		const headers = requestInit.headers as Headers;
		expect(headers.get('Authorization')).toBe('Bearer token-123');
		expect(result.overview.state).toBe('ready');
		expect(result.overview.nodes_total).toBe(5);
		expect(result.overview.alerts).toHaveLength(1);
		expect(result.overview.recent_tasks).toHaveLength(1);
	});

	it('returns error state when overview response is non-json', async () => {
		const { load } = await importRootLoad({ browser: true, token: 'token-123' });
		const fetcher = vi
			.fn()
			.mockResolvedValue(
				new Response('<!doctype html>', { status: 200, headers: { 'content-type': 'text/html' } })
			);

		const result = await load({
			fetch: fetcher
		} as unknown as import('../../routes/$types').PageLoadEvent);

		expect(result.overview.state).toBe('error');
		expect(result.overview.nodes_total).toBe(0);
	});
});
