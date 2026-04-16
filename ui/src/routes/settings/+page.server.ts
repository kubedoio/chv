import type { PageServerLoad } from './$types';

export type SettingsModel = {
	version: string;
	build: string;
	environment: string;
	api_endpoint: string;
	session_ttl_hours: number;
	users: { id: string; email: string; role: string }[];
	state: 'ready' | 'error';
};

export const load: PageServerLoad = async () => {
	const model: SettingsModel = {
		version: '1.4.2',
		build: '2026.04.16-8f5a2a',
		environment: 'production',
		api_endpoint: '/api/v1',
		session_ttl_hours: 24,
		users: [
			{ id: 'u-1', email: 'admin@chv.local', role: 'Admin' },
			{ id: 'u-2', email: 'operator@chv.local', role: 'Operator' }
		],
		state: 'ready'
	};

	return { settings: model };
};
