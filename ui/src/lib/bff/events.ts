import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';
import type { InfrastructureEvent } from './types';

export async function listEvents(token?: string): Promise<{
	items: Record<string, unknown>[];
	page: { page: number; page_size: number; total_items: number };
	filters: { applied: Record<string, string> } | null;
}> {
	return bffFetch(BFFEndpoints.listEvents, {
		method: 'POST',
		body: JSON.stringify({}),
		token
	});
}

export async function listVmEvents(vmId: string, token?: string): Promise<{
	items: InfrastructureEvent[];
	page: { page: number; page_size: number; total_items: number };
}> {
	return bffFetch(BFFEndpoints.listVmEvents, {
		method: 'POST',
		body: JSON.stringify({ vm_id: vmId }),
		token
	});
}
