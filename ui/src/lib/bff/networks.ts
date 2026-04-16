import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';

export async function listNetworks(token?: string): Promise<{
	items: Record<string, unknown>[];
	page: { page: number; page_size: number; total_items: number };
	filters: { applied: Record<string, string> } | null;
}> {
	return bffFetch(BFFEndpoints.listNetworks, {
		method: 'POST',
		body: JSON.stringify({}),
		token
	});
}

export async function getNetwork(network_id: string, token?: string): Promise<{
	detail: Record<string, unknown>;
}> {
	return bffFetch(BFFEndpoints.getNetwork, {
		method: 'POST',
		body: JSON.stringify({ network_id }),
		token
	});
}
