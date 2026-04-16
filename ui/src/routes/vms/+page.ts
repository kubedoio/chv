import type { PageLoad } from './$types';
import { getStoredToken } from '$lib/api/client';
import { buildVmsLoad, type VmsListModel } from '$lib/webui/vms-load';

export type { VmsListModel };

export const load: PageLoad = async ({ url }) => {
	const token = getStoredToken() ?? undefined;
	const vms = await buildVmsLoad({ searchParams: url.searchParams, token });
	return { vms };
};
