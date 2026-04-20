import type { PageLoad } from './$types';
import { getStoredToken } from '$lib/api/client';
import { getMaintenance } from '$lib/bff/maintenance';

export const load: PageLoad = async () => {
	const token = getStoredToken() ?? undefined;
	try {
		const res = await getMaintenance(token);
		return { maintenance: res, error: false };
	} catch (err) {
		console.error('BFF getMaintenance error:', err);
		return {
			error: true,
			maintenance: {
				windows: [],
				nodes: [],
				pending_actions: 0
			}
		};
	}
};
