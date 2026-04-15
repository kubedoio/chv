import { describe, expect, it, vi } from 'vitest';
import { buildVmsLoad } from './vms-load';

vi.mock('$lib/bff/vms', () => ({
	listVms: vi.fn()
}));

import { listVms } from '$lib/bff/vms';

describe('buildVmsLoad', () => {
	it('returns ready state with VMs', async () => {
		const mockedListVms = vi.mocked(listVms);
		mockedListVms.mockResolvedValue({
			items: [
				{
					vm_id: 'vm-1',
					name: 'test-vm',
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
			filters: { applied: {} },
			page: { page: 1, page_size: 50, total_items: 1 }
		});

		const searchParams = new URL('http://localhost/vms').searchParams;
		const result = await buildVmsLoad({ searchParams, token: 'token-123' });

		expect(result.state).toBe('ready');
		expect(result.items).toHaveLength(1);
		expect(result.page.page).toBe(1);
	});

	it('returns error state when listVms throws', async () => {
		const mockedListVms = vi.mocked(listVms);
		mockedListVms.mockRejectedValue(new Error('BFF down'));

		const searchParams = new URL('http://localhost/vms').searchParams;
		const result = await buildVmsLoad({ searchParams, token: 'token-123' });

		expect(result.state).toBe('error');
		expect(result.items).toHaveLength(0);
	});

	it('passes filters through to listVms', async () => {
		const mockedListVms = vi.mocked(listVms);
		mockedListVms.mockResolvedValue({
			items: [],
			filters: { applied: { powerState: 'running' } },
			page: { page: 2, page_size: 50, total_items: 0 }
		});

		const searchParams = new URL('http://localhost/vms?page=2&powerState=running&query=api').searchParams;
		const result = await buildVmsLoad({ searchParams, token: 'token-123' });

		expect(mockedListVms).toHaveBeenCalledWith(
			expect.objectContaining({
				page: 2,
				filters: expect.objectContaining({ powerState: 'running', query: 'api' })
			}),
			'token-123'
		);
		expect(result.page.page).toBe(2);
	});
});
