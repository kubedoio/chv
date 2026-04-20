import type { PageLoad } from './$types';
import { getStoredToken } from '$lib/api/client';
import { listUsers, type UserItem } from '$lib/bff/users';

export type UsersPageModel = {
	users: UserItem[];
	state: 'ready' | 'error';
};

export const load: PageLoad = async () => {
	const token = getStoredToken() ?? undefined;

	try {
		const res = await listUsers(token);
		return {
			model: {
				users: res.items ?? [],
				state: 'ready' as const
			}
		};
	} catch (err) {
		console.error('Failed to load users:', err);
		return {
			model: {
				users: [],
				state: 'error' as const
			}
		};
	}
};
