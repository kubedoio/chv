import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';

export async function getSettings(token?: string): Promise<{
	version: string;
	build: string;
	environment: string;
	api_endpoint: string;
	session_ttl_hours: number;
}> {
	return bffFetch(BFFEndpoints.getSettings, {
		method: 'POST',
		body: JSON.stringify({}),
		token
	});
}
