import type { PageLoad } from './$types';
import { getStoredToken } from '#lib/api/client.ts';
import { buildVolumesLoad, type VolumesListModel } from '#lib/webui/volumes-load.ts';

export type { VolumesListModel };

export const load: PageLoad = async ({ url }) => {
	const token = getStoredToken() ?? undefined;
	const volumes = await buildVolumesLoad({ searchParams: url.searchParams, token });
	return { volumes };
};
