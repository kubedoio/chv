import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';
import type {
	ListNodesRequest,
	ListNodesResponse,
	GetNodeRequest,
	GetNodeResponse,
	MutateNodeRequest,
	MutateNodeResponse
} from './types';

export async function listNodes(
	req: ListNodesRequest,
	token?: string
): Promise<ListNodesResponse> {
	return bffFetch<ListNodesResponse>(BFFEndpoints.listNodes, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

export async function getNode(req: GetNodeRequest, token?: string): Promise<GetNodeResponse> {
	return bffFetch<GetNodeResponse>(BFFEndpoints.getNode, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

export async function mutateNode(
	req: MutateNodeRequest,
	token?: string
): Promise<MutateNodeResponse> {
	return bffFetch<MutateNodeResponse>(BFFEndpoints.mutateNode, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}
