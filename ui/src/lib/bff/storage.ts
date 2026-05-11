import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';
import type { ListStoragePoolsResponse } from './types';

export async function listStoragePools(token?: string): Promise<ListStoragePoolsResponse> {
	return bffFetch<ListStoragePoolsResponse>(BFFEndpoints.listStoragePools, {
		method: 'POST',
		body: JSON.stringify({}),
		token
	});
}
