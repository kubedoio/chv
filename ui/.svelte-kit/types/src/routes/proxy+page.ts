// @ts-nocheck
import type { PageLoad } from './$types';
import { loadOverviewPageData } from '$lib/webui/load';

export const load = async ({ depends, fetch }: Parameters<PageLoad>[0]) => {
	depends('webui:overview');
	return loadOverviewPageData(fetch);
};
