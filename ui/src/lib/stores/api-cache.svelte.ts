import { liveState } from './live-state.svelte';

export type CacheKey = string;
export const LIST_TTL = 30_000;
export const DETAIL_TTL = 60_000;

export type CacheEntry<T> = { data: T; timestamp: number; ttl: number };

export async function cachedFetch<T>(key: CacheKey, fetcher: () => Promise<T>, ttlMs?: number): Promise<T> {
	return liveState.cachedFetch(key, fetcher, ttlMs);
}

export function invalidate(key: CacheKey): void {
	// TODO: integrate structured logger instead of console
	// eslint-disable-next-line no-console
	console.warn(
		`[api-cache] invalidate("${key}") is deprecated. Use liveState.invalidateAndRefresh() or mutateWithRefresh() instead. See ADR-004-WebUI.`
	);
	liveState.invalidateCache(key);
}

export function invalidatePattern(prefix: CacheKey): void {
	// TODO: integrate structured logger instead of console
	// eslint-disable-next-line no-console
	console.warn(
		`[api-cache] invalidatePattern("${prefix}") is deprecated. Use liveState.invalidateAndRefresh() or mutateWithRefresh() instead. See ADR-004-WebUI.`
	);
	liveState.invalidateCachePattern(prefix);
}

export function getCacheEntry<T>(_key: CacheKey): CacheEntry<T> | undefined {
	// TODO: integrate structured logger instead of console
	// eslint-disable-next-line no-console
	console.warn(
		`[api-cache] getCacheEntry() is deprecated. Use liveState.cachedFetch() for read access, or mutateWithRefresh() for post-mutation refresh. See ADR-004-WebUI.`
	);
	return undefined; // Deprecated — consumers should migrate to liveState
}

export function clearCache(): void {
	// TODO: integrate structured logger instead of console
	// eslint-disable-next-line no-console
	console.warn(
		`[api-cache] clearCache() is deprecated. Use liveState.invalidateAndRefresh() or mutateWithRefresh() instead. See ADR-004-WebUI.`
	);
	liveState.clearCache();
}
