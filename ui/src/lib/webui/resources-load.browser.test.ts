import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('$env/dynamic/public', () => ({
	env: {}
}));

afterEach(() => {
	vi.resetModules();
});

describe('resource load helpers in browser mode', () => {
	it('load VM pages without node-scoped placement fan-out requests', async () => {
		vi.doMock('$app/environment', () => ({
			browser: true
		}));
		vi.doMock('$lib/api/client', () => ({
			getStoredToken: () => 'test-token'
		}));

		const { loadVmDetailPageData, loadVmsPageData } = await import('$lib/webui/resources-load');
		const calls: string[] = [];
		const payloads = new Map<string, unknown>([
			[
				'/api/v1/nodes',
				[
					{
						id: 'node-1',
						name: 'ber-01',
						hostname: 'ber-01.local',
						ip_address: '10.0.0.11',
						status: 'online',
						is_local: true,
						resources: { vms: 1, images: 1, storage_pools: 1, networks: 1 }
					}
				]
			],
			[
				'/api/v1/vms',
				[
					{
						id: 'vm-1',
						name: 'api-01',
						image_id: 'img-1',
						storage_pool_id: 'pool-1',
						network_id: 'net-1',
						desired_state: 'running',
						actual_state: 'running',
						vcpu: 2,
						memory_mb: 4096,
						disk_path: '/var/lib/chv/api-01.qcow2',
						seed_iso_path: '/var/lib/chv/api-01-seed.iso',
						workspace_path: '/workspaces/api-01'
					}
				]
			],
			[
				'/api/v1/vms/vm-1',
				{
					id: 'vm-1',
					name: 'api-01',
					image_id: 'img-1',
					storage_pool_id: 'pool-1',
					network_id: 'net-1',
					desired_state: 'running',
					actual_state: 'running',
					vcpu: 2,
					memory_mb: 4096,
					disk_path: '/var/lib/chv/api-01.qcow2',
					seed_iso_path: '/var/lib/chv/api-01-seed.iso',
					workspace_path: '/workspaces/api-01'
				}
			],
			[
				'/api/v1/storage-pools',
				[
					{
						id: 'pool-1',
						name: 'fast',
						pool_type: 'localdisk',
						path: '/var/lib/chv/fast',
						is_default: true,
						status: 'ready',
						capacity_bytes: 500_000_000_000,
						allocatable_bytes: 200_000_000_000,
						created_at: '2026-04-01T00:00:00.000Z'
					}
				]
			],
			[
				'/api/v1/networks',
				[
					{
						id: 'net-1',
						name: 'prod',
						mode: 'bridge',
						bridge_name: 'br0',
						cidr: '192.168.10.0/24',
						gateway_ip: '192.168.10.1',
						is_system_managed: true,
						status: 'ready',
						created_at: '2026-04-01T00:00:00.000Z'
					}
				]
			],
			['/api/v1/operations', []],
			['/api/v1/events', []]
		]);
		const fetcher = vi.fn(async (input: RequestInfo | URL) => {
			const path = typeof input === 'string' ? input : input instanceof URL ? input.pathname : input.url;
			calls.push(path);
			const payload = payloads.get(path);

			return {
				ok: payload !== undefined,
				json: async () => payload
			} as Response;
		});

		await loadVmsPageData(fetcher as typeof fetch, new URL('https://example.test/vms'));
		await loadVmDetailPageData(fetcher as typeof fetch, 'vm-1', new URL('https://example.test/vms/vm-1'));

		expect(calls).toEqual(
			expect.arrayContaining([
				'/api/v1/nodes',
				'/api/v1/vms',
				'/api/v1/vms/vm-1',
				'/api/v1/storage-pools',
				'/api/v1/networks',
				'/api/v1/operations',
				'/api/v1/events'
			])
		);
		expect(calls.some((path) => /^\/api\/v1\/nodes\/[^/]+\/vms$/.test(path))).toBe(false);
	});
});
