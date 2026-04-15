import { describe, expect, it, vi } from 'vitest';
import { load } from '../../routes/nodes/[id]/+page.server';

vi.mock('$lib/bff/nodes', () => ({
	getNode: vi.fn()
}));

vi.mock('$lib/bff/vms', () => ({
	listVms: vi.fn()
}));

import { getNode } from '$lib/bff/nodes';
import { listVms } from '$lib/bff/vms';

describe('node detail page server load', () => {
	it('returns ready state with hosted VMs when both BFF calls succeed', async () => {
		const mockedGetNode = vi.mocked(getNode);
		mockedGetNode.mockResolvedValue({
			summary: {
				node_id: 'node-1',
				name: 'node-one',
				cluster: 'cluster-a',
				state: 'online',
				health: 'healthy',
				version: '1.0.0',
				cpu: '8',
				memory: '32 GiB',
				storage: '1 TiB',
				network: '10 Gbps',
				recent_tasks: []
			}
		});

		const mockedListVms = vi.mocked(listVms);
		mockedListVms.mockResolvedValue({
			items: [
				{
					vm_id: 'vm-1',
					name: 'vm-one',
					node_id: 'node-1',
					power_state: 'running',
					health: 'healthy',
					cpu: '2',
					memory: '4 GiB',
					volume_count: 1,
					nic_count: 1,
					last_task: 'created'
				}
			],
			page: { page: 1, page_size: 1000, total_items: 1 },
			filters: { applied: { nodeId: 'node-1' } }
		});

		const result = await load({
			params: { id: 'node-1' },
			url: new URL('http://localhost/nodes/node-1'),
			cookies: { get: () => 'token-123' }
		} as Parameters<typeof load>[0]);

		expect(result.detail.state).toBe('ready');
		expect(result.detail.summary.nodeId).toBe('node-1');
		expect(result.detail.hostedVms).toHaveLength(1);
		expect(result.detail.sections.find((s) => s.id === 'vms')?.count).toBe(1);
	});

	it('returns error state when getNode throws', async () => {
		const mockedGetNode = vi.mocked(getNode);
		mockedGetNode.mockRejectedValue(new Error('BFF down'));

		const mockedListVms = vi.mocked(listVms);
		mockedListVms.mockResolvedValue({
			items: [],
			page: { page: 1, page_size: 1000, total_items: 0 },
			filters: { applied: {} }
		});

		const result = await load({
			params: { id: 'node-1' },
			url: new URL('http://localhost/nodes/node-1'),
			cookies: { get: () => 'token-123' }
		} as Parameters<typeof load>[0]);

		expect(result.detail.state).toBe('error');
		expect(result.detail.hostedVms).toHaveLength(0);
	});

	it('returns error state when listVms throws', async () => {
		const mockedGetNode = vi.mocked(getNode);
		mockedGetNode.mockResolvedValue({
			summary: {
				node_id: 'node-1',
				name: 'node-one',
				cluster: 'cluster-a',
				state: 'online',
				health: 'healthy',
				version: '1.0.0',
				cpu: '8',
				memory: '32 GiB',
				storage: '1 TiB',
				network: '10 Gbps',
				recent_tasks: []
			}
		});

		const mockedListVms = vi.mocked(listVms);
		mockedListVms.mockRejectedValue(new Error('BFF down'));

		const result = await load({
			params: { id: 'node-1' },
			url: new URL('http://localhost/nodes/node-1'),
			cookies: { get: () => 'token-123' }
		} as Parameters<typeof load>[0]);

		expect(result.detail.state).toBe('error');
		expect(result.detail.hostedVms).toHaveLength(0);
	});
});
