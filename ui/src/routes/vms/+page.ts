import type { PageLoad } from './$types';
import { loadVmsPageData } from '$lib/webui/resources-load';

export const load: PageLoad = async ({ depends, fetch, url }) => {
	depends('webui:vms');
	return loadVmsPageData(fetch, url);
};
