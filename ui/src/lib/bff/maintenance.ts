import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';

export async function getMaintenance(token?: string): Promise<{
	windows: Record<string, unknown>[];
	nodes: Record<string, unknown>[];
	pending_actions: number;
}> {
	return bffFetch(BFFEndpoints.getMaintenance, {
		method: 'POST',
		body: JSON.stringify({}),
		token
	});
}
