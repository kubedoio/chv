import { bffFetch } from './client';

export interface UserItem {
	user_id: string;
	username: string;
	display_name: string | null;
	role: string;
	email: string | null;
	created_at: string;
	last_login_at: string | null;
}

export interface ListUsersResponse {
	items: UserItem[];
	count: number;
}

export async function listUsers(token?: string): Promise<ListUsersResponse> {
	return bffFetch('/v1/users', {
		method: 'POST',
		body: JSON.stringify({}),
		token
	});
}

export async function createUser(
	data: {
		username: string;
		password: string;
		role: string;
		display_name?: string;
		email?: string;
	},
	token?: string
): Promise<{ user_id: string; username: string; role: string }> {
	return bffFetch('/v1/users/create', {
		method: 'POST',
		body: JSON.stringify(data),
		token
	});
}

export async function updateUser(
	data: {
		user_id: string;
		role?: string;
		display_name?: string;
		email?: string;
		password?: string;
	},
	token?: string
): Promise<{ user_id: string; username: string; role: string }> {
	return bffFetch('/v1/users/update', {
		method: 'POST',
		body: JSON.stringify(data),
		token
	});
}

export async function deleteUser(user_id: string, token?: string): Promise<{ deleted: boolean; user_id: string }> {
	return bffFetch('/v1/users/delete', {
		method: 'POST',
		body: JSON.stringify({ user_id }),
		token
	});
}
