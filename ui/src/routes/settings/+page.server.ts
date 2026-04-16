import type { PageServerLoad } from './$types';
import { getSettings } from '$lib/bff/settings';

export type SettingsModel = {
	version: string;
	build: string;
	environment: string;
	api_endpoint: string;
	session_ttl_hours: number;
	users: { id: string; email: string; role: string }[];
	state: 'ready' | 'error';
};

export const load: PageServerLoad = async ({ cookies }) => {
	const token = cookies.get('chv_session') ?? undefined;

	try {
		const res = await getSettings(token);
		const model: SettingsModel = {
			version: res.version,
			build: res.build,
			environment: res.environment,
			api_endpoint: res.api_endpoint,
			session_ttl_hours: res.session_ttl_hours,
			users: [
				{ id: 'u-1', email: 'admin@chv.local', role: 'Admin' },
				{ id: 'u-2', email: 'operator@chv.local', role: 'Operator' }
			],
			state: 'ready'
		};
		return { settings: model };
	} catch (err) {
		// eslint-disable-next-line no-console
		console.error('BFF getSettings error:', err);
		const model: SettingsModel = {
			version: 'unknown',
			build: 'unknown',
			environment: 'unknown',
			api_endpoint: '/api/v1',
			session_ttl_hours: 24,
			users: [],
			state: 'error'
		};
		return { settings: model };
	}
};
