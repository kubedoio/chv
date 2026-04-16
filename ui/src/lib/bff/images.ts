import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';

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
