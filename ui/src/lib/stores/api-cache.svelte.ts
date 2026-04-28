import { browser } from '$app/environment';

export type CacheKey = string;
export type CacheEntry<T> = { data: T; timestamp: number; ttl: number };

export const LIST_TTL = 30_000;
export const DETAIL_TTL = 60_000;

const cache = $state<Map<CacheKey, CacheEntry<unknown>>>(new Map());

function isFresh(entry: CacheEntry<unknown>): boolean {
	return Date.now() - entry.timestamp < entry.ttl;
}

export async function cachedFetch<T>(
	key: CacheKey,
	fetcher: () => Promise<T>,
	ttlMs?: number
): Promise<T> {
	if (!browser) {
		return fetcher();
	}

	const entry = cache.get(key) as CacheEntry<T> | undefined;
	if (entry && isFresh(entry)) {
		return entry.data;
	}

	try {
		const data = await fetcher();
		cache.set(key, { data, timestamp: Date.now(), ttl: ttlMs ?? LIST_TTL });
		return data;
	} catch (err) {
		if (entry) {
			// TODO: integrate structured logger instead of console
			// eslint-disable-next-line no-console
			console.warn(`[api-cache] fetch error for key "${key}", returning stale data`, err);
			return entry.data;
		}
		throw err;
	}
}

export function invalidate(key: CacheKey): void {
	if (!browser) return;
	cache.delete(key);
}

export function invalidatePattern(prefix: CacheKey): void {
	if (!browser) return;
	for (const k of cache.keys()) {
		if (k.startsWith(prefix)) {
			cache.delete(k);
		}
	}
}

export function getCacheEntry<T>(key: CacheKey): CacheEntry<T> | undefined {
	if (!browser) return undefined;
	return cache.get(key) as CacheEntry<T> | undefined;
}

export function clearCache(): void {
	if (!browser) return;
	cache.clear();
}
