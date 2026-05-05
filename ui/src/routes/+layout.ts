import { redirect } from '@sveltejs/kit';
import type { LayoutLoad } from './$types';

const PUBLIC_PATHS = ['/login', '/install'];

export const load: LayoutLoad = ({ url }) => {
	const isPublic = PUBLIC_PATHS.some((p) => url.pathname.startsWith(p));
	if (isPublic) return {};

	if (typeof window !== 'undefined') {
		const token = localStorage.getItem('chv-api-token');
		if (!token) {
			throw redirect(302, '/login');
		}
	}

	return {};
};
