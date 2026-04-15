import type { PageLoad } from './$types';
import { loadVmDetailPageData } from '$lib/webui/resources-load';

export const load: PageLoad = async ({ depends, fetch, params, url }) => {
	depends(`webui:vm:${params.id}`);
	return loadVmDetailPageData(fetch, params.id, url);
};
