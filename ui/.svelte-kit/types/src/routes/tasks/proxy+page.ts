// @ts-nocheck
import type { PageLoad } from './$types';
import { loadTasksPageData } from '$lib/webui/load';

export const load = async ({ depends, fetch, url }: Parameters<PageLoad>[0]) => {
	depends('webui:tasks');
	return loadTasksPageData(fetch, url);
};
