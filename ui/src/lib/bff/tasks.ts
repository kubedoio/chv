import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';
import type { ListTasksRequest, ListTasksResponse } from './types';

export async function listTasks(
	req: ListTasksRequest,
	token?: string
): Promise<ListTasksResponse> {
	return bffFetch<ListTasksResponse>(BFFEndpoints.listTasks, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}
