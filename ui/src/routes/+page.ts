import { browser } from '$app/environment';
import { getStoredToken } from '$lib/api/client';
import { loadOverview } from '$lib/bff/overview';
import { cachedFetch, LIST_TTL } from '$lib/stores/api-cache.svelte';
import { createOverview, toOverviewModel } from '$lib/helpers/dashboard';
import type { PageLoad } from './$types';

export const load: PageLoad = async () => {
	// In static builds, avoid server-pass data fetches that can become HTML fallback responses.
	if (!browser) {
		return { overview: createOverview('loading') };
	}

	const token = getStoredToken();

	try {
		const payload = await cachedFetch(
			'overview',
			() => loadOverview(token ?? undefined),
			LIST_TTL
		);
		return { overview: toOverviewModel(payload) };
	} catch {
		return { overview: createOverview('error') };
	}
};
