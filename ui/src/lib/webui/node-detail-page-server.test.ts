import { describe, expect, it, vi } from 'vitest';
import { load } from '../../routes/nodes/[id]/+page.server';

vi.mock('$lib/bff/nodes', () => ({
	getNode: vi.fn()
}));

import { getNode } from '$lib/bff/nodes';

function createCookies(token?: string) {
	return {
		get: vi.fn().mockReturnValue(token)
	} as unknown as import('@sveltejs/kit').Cookies;
}

function createUrl(tab?: string) {
	const url = new URL('http://localhost/nodes/node-1');
	if (tab) url.searchParams.set('tab', tab);
	return url;
}

describe('node detail page server load', () => {
	it('returns ready state when getNode succeeds', async () => {
		const mockedGetNode = vi.mocked(getNode);
		mockedGetNode.mockResolvedValue({
			state: 'ready',
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
				network: '10 Gbps'
			},
			sections: [
				{ id: 'summary', label: 'Summary' },
				{ id: 'vms', label: 'VMs', count: 2 },
				{ id: 'tasks', label: 'Tasks', count: 1 },
				{ id: 'configuration', label: 'Configuration' }
			],
			hostedVms: [
				{
					vm_id: 'vm-1',
					name: 'vm-one',
					power_state: 'running',
					health: 'healthy',
					cpu: '2',
					memory: '4 GiB'
				}
			],
			recentTasks: [
				{
					task_id: 'task-1',
					status: 'succeeded',
					summary: 'Health check',
					operation: 'health_check',
					started_unix_ms: 1713000000000
				}
			],
			configuration: [
				{ label: 'Node ID', value: 'node-1' },
				{ label: 'Version', value: '1.0.0' }
			]
		} as import('$lib/bff/types').GetNodeResponse);

		const result = await load({
			params: { id: 'node-1' },
			url: createUrl('vms'),
			cookies: createCookies('token-123')
		} as unknown as import('./$types').PageServerLoadEvent);

		expect(mockedGetNode).toHaveBeenCalledWith({ node_id: 'node-1' }, 'token-123');
		expect(result.detail.state).toBe('ready');
		expect(result.detail.currentTab).toBe('vms');
		expect(result.detail.summary.nodeId).toBe('node-1');
		expect(result.detail.hostedVms).toHaveLength(1);
		expect(result.requestedNodeId).toBe('node-1');
	});

	it('returns error state when getNode throws', async () => {
		const mockedGetNode = vi.mocked(getNode);
		mockedGetNode.mockRejectedValue(new Error('BFF down'));

		const result = await load({
			params: { id: 'node-1' },
			url: createUrl(),
			cookies: createCookies('token-123')
		} as unknown as import('./$types').PageServerLoadEvent);

		expect(result.detail.state).toBe('error');
		expect(result.detail.summary.nodeId).toBe('node-1');
		expect(result.requestedNodeId).toBe('node-1');
	});

	it('defaults tab to summary when not provided', async () => {
		const mockedGetNode = vi.mocked(getNode);
		mockedGetNode.mockResolvedValue({
			state: 'ready',
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
				network: '10 Gbps'
			}
		} as import('$lib/bff/types').GetNodeResponse);

		const result = await load({
			params: { id: 'node-1' },
			url: createUrl(),
			cookies: createCookies()
		} as unknown as import('./$types').PageServerLoadEvent);

		expect(result.detail.currentTab).toBe('summary');
	});
});
