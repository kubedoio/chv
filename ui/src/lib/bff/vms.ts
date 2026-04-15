import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';
import type {
	ListVmsRequest,
	ListVmsResponse,
	GetVmRequest,
	GetVmResponse,
	MutateVmRequest,
	MutateVmResponse
} from './types';

export async function listVms(req: ListVmsRequest, token?: string): Promise<ListVmsResponse> {
	return bffFetch<ListVmsResponse>(BFFEndpoints.listVms, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

export async function getVm(req: GetVmRequest, token?: string): Promise<GetVmResponse> {
	return bffFetch<GetVmResponse>(BFFEndpoints.getVm, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

export async function mutateVm(
	req: MutateVmRequest,
	token?: string
): Promise<MutateVmResponse> {
	return bffFetch<MutateVmResponse>(BFFEndpoints.mutateVm, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}
