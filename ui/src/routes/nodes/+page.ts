import type { PageLoad } from './$types';
import { loadNodesPageData } from '$lib/webui/resources-load';

export const load: PageLoad = async ({ depends, fetch, url }) => {
	depends('webui:nodes');
	return loadNodesPageData(fetch, url);
};
