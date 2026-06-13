import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$env/dynamic/public', () => ({
	env: {}
}));

vi.mock('$app/navigation', () => ({
	invalidateAll: vi.fn()
}));

import { invalidateAll } from '$app/navigation';
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

	describe('invalidateAndRefresh', () => {
		beforeEach(() => {
			// invalidateAll is the imported reference to the mocked $app/navigation export.
			(invalidateAll as ReturnType<typeof vi.fn>).mockClear();
			(invalidateAll as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
		});

		it('invalidates each pattern via invalidateCachePattern', async () => {
			const patternSpy = vi
				.spyOn(liveState, 'invalidateCachePattern');
			vi.spyOn(liveState, 'fetchInventory').mockResolvedValue(undefined);

			await liveState.invalidateAndRefresh({ patterns: ['vms:', 'nodes:'] });

			expect(patternSpy).toHaveBeenCalledWith('vms:');
			expect(patternSpy).toHaveBeenCalledWith('nodes:');
			expect(patternSpy).toHaveBeenCalledTimes(2);
		});

		it('invalidates the per-resource cache key when detailId is provided alongside patterns', async () => {
			vi.spyOn(liveState, 'fetchInventory').mockResolvedValue(undefined);

			// Pre-populate the per-resource cache entry that the detail view would read.
			const detailFetcher = vi.fn().mockResolvedValue({ id: 'abc-123', name: 'web-1' });
			await liveState.cachedFetch('vms:abc-123', detailFetcher);
			expect(detailFetcher).toHaveBeenCalledTimes(1);

			await liveState.invalidateAndRefresh({
				patterns: ['vms:'],
				detailId: 'abc-123'
			});

			// After invalidation, the next read must re-fetch — the per-resource
			// key was dropped, not just the list-level pattern entries.
			await liveState.cachedFetch('vms:abc-123', detailFetcher);
			expect(detailFetcher).toHaveBeenCalledTimes(2);
		});

		it('calls fetchInventory when sidebar is true', async () => {
			const inventorySpy = vi
				.spyOn(liveState, 'fetchInventory')
				.mockResolvedValue(undefined);

			await liveState.invalidateAndRefresh({ sidebar: true });

			expect(inventorySpy).toHaveBeenCalledTimes(1);
		});

		it('does not call fetchInventory when sidebar is false (or omitted)', async () => {
			const inventorySpy = vi
				.spyOn(liveState, 'fetchInventory')
				.mockResolvedValue(undefined);

			await liveState.invalidateAndRefresh({ sidebar: false });
			await liveState.invalidateAndRefresh({});

			expect(inventorySpy).not.toHaveBeenCalled();
		});

		it('always calls invalidateAll() from $app/navigation', async () => {
			vi.spyOn(liveState, 'fetchInventory').mockResolvedValue(undefined);

			await liveState.invalidateAndRefresh({ patterns: ['vms:'] });

			expect(invalidateAll).toHaveBeenCalledTimes(1);
		});

		it('runs invalidation immediately and again after delayMs when delayMs > 0', async () => {
			vi.useFakeTimers();
			try {
				const patternSpy = vi.spyOn(liveState, 'invalidateCachePattern');
				vi.spyOn(liveState, 'fetchInventory').mockResolvedValue(undefined);

				await liveState.invalidateAndRefresh({
					patterns: ['vms:'],
					delayMs: 500
				});

				// Immediate pass: one call.
				expect(patternSpy).toHaveBeenCalledTimes(1);

				// Before the delay elapses, no further calls.
				vi.advanceTimersByTime(499);
				expect(patternSpy).toHaveBeenCalledTimes(1);

				// After the delay elapses, the deferred pass runs.
				vi.advanceTimersByTime(1);
				expect(patternSpy).toHaveBeenCalledTimes(2);
				expect(patternSpy).toHaveBeenLastCalledWith('vms:');
			} finally {
				vi.useRealTimers();
			}
		});

		it('is a no-op under SSR (browser=false)', async () => {
			// Re-import the module with $app/environment.browser stubbed to false
			// to exercise the SSR guard at the top of invalidateAndRefresh.
			vi.resetModules();
			vi.doMock('$app/environment', () => ({ browser: false }));
			vi.doMock('$env/dynamic/public', () => ({ env: {} }));
			const navMock = { invalidateAll: vi.fn() };
			vi.doMock('$app/navigation', () => navMock);

			try {
				const mod = await import('./live-state.svelte');
				const ssrLiveState = mod.liveState;

				const patternSpy = vi.spyOn(ssrLiveState, 'invalidateCachePattern');
				const inventorySpy = vi.spyOn(ssrLiveState, 'fetchInventory');

				await ssrLiveState.invalidateAndRefresh({
					patterns: ['vms:'],
					sidebar: true,
					detailId: 'abc-123',
					delayMs: 100
				});

				expect(patternSpy).not.toHaveBeenCalled();
				expect(inventorySpy).not.toHaveBeenCalled();
				expect(navMock.invalidateAll).not.toHaveBeenCalled();
			} finally {
				vi.doUnmock('$app/environment');
				vi.doUnmock('$app/navigation');
				vi.doUnmock('$env/dynamic/public');
				vi.resetModules();
			}
		});
	});
});
