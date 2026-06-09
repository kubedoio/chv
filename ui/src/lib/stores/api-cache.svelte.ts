import { liveState } from './live-state.svelte';

export type CacheKey = string;
export const LIST_TTL = 30_000;
export const DETAIL_TTL = 60_000;

export async function cachedFetch<T>(key: CacheKey, fetcher: () => Promise<T>, ttlMs?: number): Promise<T> {
	return liveState.cachedFetch(key, fetcher, ttlMs);
}

export function invalidate(key: CacheKey): void {
	liveState.invalidateCache(key);
}

export function invalidatePattern(prefix: CacheKey): void {
	liveState.invalidateCachePattern(prefix);
}

export function getCacheEntry<T>(_key: CacheKey): undefined {
	return undefined; // Deprecated — consumers should migrate to liveState
}

export function clearCache(): void {
	liveState.clearCache();
}
