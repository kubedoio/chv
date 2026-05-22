import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';
import type { InfrastructureEvent, ListEventsResponse } from './types';

export async function listEvents(token?: string): Promise<ListEventsResponse> {
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
