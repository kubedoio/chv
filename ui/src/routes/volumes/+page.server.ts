import type { PageServerLoad } from './$types';
import { buildVolumesLoad, type VolumesListModel } from '$lib/webui/volumes-load';

export type { VolumesListModel };

export const load: PageServerLoad = async ({ url, cookies }) => {
	const token = cookies.get('chv_session') ?? undefined;
	const volumes = await buildVolumesLoad({ searchParams: url.searchParams, token });
	return { volumes };
};
