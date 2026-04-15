// @ts-nocheck
import type { PageLoad } from './$types';
import { loadVmDetailPageData } from '$lib/webui/resources-load';

export const load = async ({ depends, fetch, params, url }: Parameters<PageLoad>[0]) => {
	depends(`webui:vm:${params.id}`);
	return loadVmDetailPageData(fetch, params.id, url);
};
