import type { PageServerLoad } from './$types';
import { loadOverview } from '$lib/bff/overview';
import { BFFError } from '$lib/bff/client';

export const load: PageServerLoad = async ({ cookies }) => {
	const token = cookies.get('chv_session') ?? undefined;
	try {
		const overview = await loadOverview(token);
		return {
			overview,
			meta: { error: false }
		};
	} catch (err) {
		const message = err instanceof BFFError ? err.message : 'Overview unavailable';
		return {
			overview: null,
			meta: { error: true, message }
		};
	}
};
