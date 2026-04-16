import type { PageServerLoad } from './$types';

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

const mockImages: ImageListItem[] = [
	{
		image_id: 'img-1',
		name: 'ubuntu-22.04-lts',
		os: 'Ubuntu',
		version: '22.04.5',
		status: 'ready',
		last_updated: '2026-04-10',
		usage_count: 482,
		size: '2.1 GB'
	},
	{
		image_id: 'img-2',
		name: 'debian-12-base',
		os: 'Debian',
		version: '12.5',
		status: 'ready',
		last_updated: '2026-04-08',
		usage_count: 215,
		size: '1.8 GB'
	},
	{
		image_id: 'img-3',
		name: 'alpine-3.19-minimal',
		os: 'Alpine',
		version: '3.19.1',
		status: 'ready',
		last_updated: '2026-03-22',
		usage_count: 89,
		size: '420 MB'
	},
	{
		image_id: 'img-4',
		name: 'windows-server-2022',
		os: 'Windows Server',
		version: '2022 23H2',
		status: 'ready',
		last_updated: '2026-04-01',
		usage_count: 56,
		size: '8.4 GB'
	},
	{
		image_id: 'img-5',
		name: 'ubuntu-20.04-lts',
		os: 'Ubuntu',
		version: '20.04.6',
		status: 'deprecated',
		last_updated: '2025-11-15',
		usage_count: 34,
		size: '2.0 GB'
	},
	{
		image_id: 'img-6',
		name: 'fedora-39-workstation',
		os: 'Fedora',
		version: '39.1.5',
		status: 'pending',
		last_updated: '2026-04-15',
		usage_count: 0,
		size: '3.2 GB'
	},
	{
		image_id: 'img-7',
		name: 'rocky-linux-9',
		os: 'Rocky Linux',
		version: '9.3',
		status: 'ready',
		last_updated: '2026-02-28',
		usage_count: 124,
		size: '2.3 GB'
	}
];

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

export const load: PageServerLoad = async ({ url }) => {
	const page = Math.max(1, parseInt(url.searchParams.get('page') ?? '1', 10) || 1);
	const pageSize = 50;

	const current: Record<string, string> = {};
	const query = url.searchParams.get('query');
	const status = url.searchParams.get('status');

	if (query) current.query = query;
	if (status) current.status = status;

	const filtered = filterImages(mockImages, current);

	const model: ImagesListModel = {
		items: filtered,
		state: filtered.length === 0 ? 'empty' : 'ready',
		filters: { current, applied: current },
		page: { page, pageSize, totalItems: filtered.length }
	};

	return { images: model };
};
