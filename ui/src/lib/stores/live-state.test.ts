import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$env/dynamic/public', () => ({
	env: {}
}));

vi.mock('$app/navigation', () => ({
	invalidateAll: vi.fn()
}));

import { liveState } from './live-state.svelte';

describe('liveState', () => {
	beforeEach(() => {
		liveState.clearCache();
		liveState.nodes = [];
		liveState.vms = [];
		vi.restoreAllMocks();
	});

	it('caches fetch results', async () => {
		const fetcher = vi.fn().mockResolvedValue({ data: 42 });
		const r1 = await liveState.cachedFetch('test-key', fetcher);
		const r2 = await liveState.cachedFetch('test-key', fetcher);
		expect(r1).toEqual({ data: 42 });
		expect(r2).toEqual({ data: 42 });
		expect(fetcher).toHaveBeenCalledTimes(1);
	});

	it('invalidates cache by pattern', async () => {
		const fetcher = vi.fn().mockResolvedValue({ data: 1 });
		await liveState.cachedFetch('vms:list', fetcher);
		liveState.invalidateCachePattern('vms:');
		await liveState.cachedFetch('vms:list', fetcher);
		expect(fetcher).toHaveBeenCalledTimes(2);
	});

	it('computes pinnedVMs from running VMs', () => {
		liveState.vms = [
			{ id: '1', actual_state: 'running', name: 'A' } as any,
			{ id: '2', actual_state: 'stopped', name: 'B' } as any,
			{ id: '3', actual_state: 'running', name: 'C' } as any,
			{ id: '4', actual_state: 'running', name: 'D' } as any,
		];
		expect(liveState.pinnedVms.length).toBe(3);
		expect(liveState.pinnedVms[0].name).toBe('A');
	});

	it('returns stale data on fetch error if cache exists', async () => {
		vi.useFakeTimers();
		const fetcher = vi.fn().mockResolvedValueOnce({ data: 'fresh' }).mockRejectedValueOnce(new Error('network'));
		const r1 = await liveState.cachedFetch('key', fetcher);
		expect(r1).toEqual({ data: 'fresh' });

		// Advance time beyond TTL
		vi.advanceTimersByTime(31_000);

		const r2 = await liveState.cachedFetch('key', fetcher);
		expect(r2).toEqual({ data: 'fresh' });
		vi.useRealTimers();
	});
});
