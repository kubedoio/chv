import type { PageLoad } from './$types';
import { getStoredToken } from '#lib/api/client.ts';
import { buildVmsLoad, type VmsListModel } from '#lib/webui/vms-load.ts';

export type { VmsListModel };

export const load: PageLoad = async ({ url }) => {
	const token = getStoredToken() ?? undefined;
	const vms = await buildVmsLoad({ searchParams: url.searchParams, token });
	return { vms };
};
