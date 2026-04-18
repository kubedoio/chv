import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';
import type { CreateNetworkInput, UpdateNetworkInput } from './types';

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

export async function createNetwork(data: CreateNetworkInput, token?: string): Promise<{
	network_id: string;
}> {
	return bffFetch(BFFEndpoints.createNetwork, {
		method: 'POST',
		body: JSON.stringify(data),
		token
	});
}

export async function updateNetwork(data: UpdateNetworkInput, token?: string): Promise<{
	network_id: string;
}> {
	return bffFetch(BFFEndpoints.updateNetwork, {
		method: 'POST',
		body: JSON.stringify(data),
		token
	});
}

export async function deleteNetwork(networkId: string, token?: string): Promise<void> {
	return bffFetch(BFFEndpoints.deleteNetwork, {
		method: 'POST',
		body: JSON.stringify({ network_id: networkId }),
		token
	});
}
