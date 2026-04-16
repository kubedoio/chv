import type { PageLoad } from './$types';
import { getStoredToken } from '$lib/api/client';
import { listImages } from '$lib/bff/images';

export type ImageListItem = {
	image_id: string;
	name: string;
	os: string;
	version: string;
	status: 'ready' | 'pending' | 'failed' | 'deprecated';
	last_updated: string;
	usage_count: number;
	size: string;
};

export type ImagesListModel = {
	items: ImageListItem[];
	state: 'ready' | 'empty' | 'error';
	filters: { current: Record<string, string>; applied: Record<string, string> };
	page: { page: number; pageSize: number; totalItems: number };
};

function filterImages(items: ImageListItem[], current: Record<string, string>): ImageListItem[] {
	let result = [...items];
	const query = (current.query ?? '').toLowerCase();
	if (query) {
		result = result.filter(
			(i) => i.name.toLowerCase().includes(query) || i.os.toLowerCase().includes(query)
		);
	}
	const status = current.status;
	if (status && status !== 'all') {
		result = result.filter((i) => i.status === status);
	}
	return result;
}

export const load: PageLoad = async ({ url }) => {
	const token = getStoredToken() ?? undefined;
	const page = Math.max(1, parseInt(url.searchParams.get('page') ?? '1', 10) || 1);
	const pageSize = 50;

	const current: Record<string, string> = {};
	const query = url.searchParams.get('query');
	const status = url.searchParams.get('status');

	if (query) current.query = query;
	if (status) current.status = status;

	try {
		const res = await listImages(token);
		const items = (res.items ?? []) as ImageListItem[];
		const filtered = filterImages(items, current);

		const model: ImagesListModel = {
			items: filtered,
			state: filtered.length === 0 ? 'empty' : 'ready',
			filters: { current, applied: res.filters?.applied ?? current },
			page: { page, pageSize, totalItems: res.page.total_items }
		};

		return { images: model };
	} catch (err) {
		// eslint-disable-next-line no-console
		console.error('BFF listImages error:', err);
		const model: ImagesListModel = {
			items: [],
			state: 'error',
			filters: { current, applied: {} },
			page: { page, pageSize, totalItems: 0 }
		};
		return { images: model };
	}
};
