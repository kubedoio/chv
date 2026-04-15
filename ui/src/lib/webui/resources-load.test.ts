import { describe, expect, it, vi } from 'vitest';

vi.mock('$env/dynamic/public', () => ({
	env: {}
}));

import {
	loadNodeDetailPageData,
	loadNodesPageData,
	loadVmDetailPageData,
	loadVmsPageData
} from '$lib/webui/resources-load';

describe('resource load helpers', () => {
	it('defer protected nodes and VMs fetches during the server-side pass', async () => {
		const fetcher = vi.fn();

		const nodes = await loadNodesPageData(fetcher as typeof fetch, new URL('https://example.test/nodes'));
		const nodeDetail = await loadNodeDetailPageData(
			fetcher as typeof fetch,
			'node-1',
			new URL('https://example.test/nodes/node-1')
		);
		const vms = await loadVmsPageData(fetcher as typeof fetch, new URL('https://example.test/vms'));
		const vmDetail = await loadVmDetailPageData(
			fetcher as typeof fetch,
			'vm-1',
			new URL('https://example.test/vms/vm-1')
		);

		expect(fetcher).not.toHaveBeenCalled();
		expect(nodes.meta.deferred).toBe(true);
		expect(nodeDetail.meta.deferred).toBe(true);
		expect(vms.meta.deferred).toBe(true);
		expect(vmDetail.meta.deferred).toBe(true);
		expect(vmDetail.requestedVmId).toBe('vm-1');
	});
});
