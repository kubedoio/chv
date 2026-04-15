import type { PageLoad } from './$types';
import { loadTasksPageData } from '$lib/webui/load';

export const load: PageLoad = async ({ depends, fetch, url }) => {
	depends('webui:tasks');
	return loadTasksPageData(fetch, url);
};
