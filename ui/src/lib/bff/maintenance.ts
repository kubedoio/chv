import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';
import type { GetMaintenanceResponse } from './types';

export async function getMaintenance(token?: string): Promise<GetMaintenanceResponse> {
	return bffFetch(BFFEndpoints.getMaintenance, {
		method: 'POST',
		body: JSON.stringify({}),
		token
	});
}
