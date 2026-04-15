import { describe, expect, it, vi } from 'vitest';
import { load } from '../../routes/nodes/+page.server';

vi.mock('$lib/bff/nodes', () => ({
	listNodes: vi.fn()
}));

import { listNodes } from '$lib/bff/nodes';

describe('nodes page server load', () => {
	it('returns ready state with nodes when listNodes succeeds', async () => {
		const mockedListNodes = vi.mocked(listNodes);
		mockedListNodes.mockResolvedValue({
			items: [
				{
					node_id: 'node-1',
					name: 'node-one',
					cluster: 'cluster-a',
					state: 'online',
					health: 'healthy',
					cpu: '8',
					memory: '32 GiB',
					storage: '1 TiB',
					network: '10 Gbps',
					version: '1.0.0',
					maintenance: false
				}
			],
			page: { page: 1, page_size: 50, total_items: 1 },
			filters: { applied: {} }
		});

		const result = await load({
			url: new URL('http://localhost/nodes'),
			cookies: { get: () => 'token-123' }
		} as Parameters<typeof load>[0]);

		expect(result.nodes.state).toBe('ready');
		expect(result.nodes.items).toHaveLength(1);
		expect(result.nodes.page.totalItems).toBe(1);
	});

	it('returns error state when listNodes throws', async () => {
		const mockedListNodes = vi.mocked(listNodes);
		mockedListNodes.mockRejectedValue(new Error('BFF down'));

		const result = await load({
			url: new URL('http://localhost/nodes'),
			cookies: { get: () => 'token-123' }
		} as Parameters<typeof load>[0]);

		expect(result.nodes.state).toBe('error');
		expect(result.nodes.items).toHaveLength(0);
	});
});
