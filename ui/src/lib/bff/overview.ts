import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';
import type { OverviewResponse } from './types';

export async function loadOverview(token?: string): Promise<OverviewResponse> {
	return bffFetch<OverviewResponse>(BFFEndpoints.overview, {
		method: 'POST',
		body: JSON.stringify({}),
		token
	});
}
