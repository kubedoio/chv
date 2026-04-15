import type { PageLoad } from './$types';
import { loadNodeDetailPageData } from '$lib/webui/resources-load';

export const load: PageLoad = async ({ depends, fetch, params, url }) => {
	depends(`webui:node:${params.id}`);
	return loadNodeDetailPageData(fetch, params.id, url);
};
