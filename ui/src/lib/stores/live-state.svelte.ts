import { browser } from '$app/environment';
import { invalidateAll } from '$app/navigation';
import { getStoredToken } from '$lib/api/client';
import { inventory } from './inventory.svelte';
import { invalidatePattern } from './api-cache.svelte';

export interface InvalidateOpts {
	patterns?: string[];
	sidebar?: boolean;
	detailId?: string;
	delayMs?: number;
}

class LiveState {
	async invalidateAndRefresh(opts: InvalidateOpts = {}) {
		if (!browser) return;

		if (opts.patterns) {
			for (const p of opts.patterns) {
				invalidatePattern(p as import('./api-cache.svelte').CacheKey);
			}
		}

		if (opts.sidebar) {
			await inventory.fetch();
		}

		await invalidateAll();

		if (opts.delayMs && opts.delayMs > 0) {
			setTimeout(() => {
				if (opts.patterns) {
					for (const p of opts.patterns) {
						invalidatePattern(p as import('./api-cache.svelte').CacheKey);
					}
				}
				invalidateAll();
			}, opts.delayMs);
		}
	}
}

export const liveState = new LiveState();
