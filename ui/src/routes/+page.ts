import type { PageLoad } from './$types';
import { loadOverviewPageData } from '$lib/webui/load';

export const load: PageLoad = async ({ depends, fetch }) => {
	depends('webui:overview');
	return loadOverviewPageData(fetch);
};
