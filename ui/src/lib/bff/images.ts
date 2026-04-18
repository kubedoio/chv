import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';
import type { ImportImageRequest, ImportImageResponse } from './types';

export async function listImages(token?: string): Promise<{
	items: Record<string, unknown>[];
	page: { page: number; page_size: number; total_items: number };
	filters: { applied: Record<string, string> } | null;
}> {
	return bffFetch(BFFEndpoints.listImages, {
		method: 'POST',
		body: JSON.stringify({}),
		token
	});
}

export async function importImage(
	req: ImportImageRequest,
	token?: string
): Promise<ImportImageResponse> {
	return bffFetch<ImportImageResponse>(BFFEndpoints.importImage, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}
