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
		const listStoragePools = vi.fn(async () => ({
			items: [
				{
					pool_id: 'pool-1',
					name: 'fast',
					pool_type: 'localdisk',
					path: '/var/lib/chv/fast',
					is_default: true,
					status: 'ready',
					capacity_bytes: 500_000_000_000,
					allocatable_bytes: 200_000_000_000,
					created_at: '2026-04-01T00:00:00.000Z'
				}
			],
			page: { page: 1, page_size: 50, total_items: 1 }
		}));
		vi.doMock('$lib/bff/storage', () => ({
			listStoragePools
		}));
		const listNodes = vi.fn(async () => ({
			items: [
				{
					node_id: 'node-1',
					name: 'ber-01',
					cluster: '',
					state: 'TenantReady',
					health: 'healthy',
					cpu: '8',
					memory: '32.0 GiB',
					storage: '1.0 TiB',
					network: '',
					version: 'dev',
					maintenance: false,
					active_tasks: 0,
					alerts: 0
				}
			],
			page: { page: 1, page_size: 50, total_items: 1 },
			filters: { applied: {} }
		}));
		vi.doMock('$lib/bff/nodes', () => ({
			listNodes
		}));
		const listVms = vi.fn(async () => ({
			items: [
				{
					vm_id: 'vm-1',
					name: 'api-01',
					node_id: 'node-1',
					power_state: 'running',
					health: 'healthy',
					cpu: '2',
					memory: '4.0 GiB',
					volume_count: 1,
					nic_count: 1,
					last_task: ''
				}
			],
			page: { page: 1, page_size: 50, total_items: 1 },
			filters: { applied: {} }
		}));
		const getVm = vi.fn(async () => ({
			summary: {
				vm_id: 'vm-1',
				name: 'api-01',
				node_id: 'node-1',
				power_state: 'running',
				health: 'healthy',
				cpu: '2',
				memory: '4.0 GiB',
				recent_tasks: []
			}
		}));
		vi.doMock('$lib/bff/vms', () => ({
			listVms,
			getVm
		}));
		const listNetworks = vi.fn(async () => ({
			items: [
				{
					network_id: 'net-1',
					name: 'prod',
					health: 'ready',
					exposure: 'bridge',
					cidr: '192.168.10.0/24',
					gateway: '192.168.10.1'
				}
			],
			page: { page: 1, page_size: 50, total_items: 1 },
			filters: { applied: {} }
		}));
		vi.doMock('$lib/bff/networks', () => ({
			listNetworks
		}));
		const listTasks = vi.fn(async () => ({
			items: [],
			page: { page: 1, page_size: 50, total_items: 0 },
			filters: { applied: {} }
		}));
		vi.doMock('$lib/bff/tasks', () => ({
			listTasks
		}));
		const listEvents = vi.fn(async () => ({
			items: [],
			page: { page: 1, page_size: 50, total_items: 0 },
			filters: { applied: {} }
		}));
		vi.doMock('$lib/bff/events', () => ({
			listEvents
		}));

		const { loadVmDetailPageData, loadVmsPageData } = await import('$lib/webui/resources-load');
		const calls: string[] = [];
		const fetcher = vi.fn(async (input: RequestInfo | URL) => {
			const path = typeof input === 'string' ? input : input instanceof URL ? input.pathname : input.url;
			calls.push(path);

			return {
				ok: false,
				json: async () => null
			} as Response;
		});

		await loadVmsPageData(fetcher as typeof fetch, new URL('https://example.test/vms'));
		await loadVmDetailPageData(fetcher as typeof fetch, 'vm-1', new URL('https://example.test/vms/vm-1'));

		expect(calls).toEqual([]);
		expect(listNodes).toHaveBeenCalledWith({ page: 1, page_size: 200, filters: {} }, 'test-token');
		expect(listVms).toHaveBeenCalledWith({ page: 1, page_size: 200, filters: {} }, 'test-token');
		expect(getVm).toHaveBeenCalledWith({ vm_id: 'vm-1' }, 'test-token');
		expect(listNetworks).toHaveBeenCalledWith('test-token');
		expect(listTasks).toHaveBeenCalledWith({ page: 1, page_size: 200, filters: {} }, 'test-token');
		expect(listEvents).toHaveBeenCalledWith('test-token');
		expect(listStoragePools).toHaveBeenCalledWith('test-token');
	});
});
