// @ts-nocheck
import type { PageLoad } from './$types';
import { loadNodesPageData } from '$lib/webui/resources-load';

export const load = async ({ depends, fetch, url }: Parameters<PageLoad>[0]) => {
	depends('webui:nodes');
	return loadNodesPageData(fetch, url);
};
