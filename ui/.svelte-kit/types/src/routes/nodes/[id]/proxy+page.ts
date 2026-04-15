// @ts-nocheck
import type { PageLoad } from './$types';
import { loadNodeDetailPageData } from '$lib/webui/resources-load';

export const load = async ({ depends, fetch, params, url }: Parameters<PageLoad>[0]) => {
	depends(`webui:node:${params.id}`);
	return loadNodeDetailPageData(fetch, params.id, url);
};
