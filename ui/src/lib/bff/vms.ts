import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';
import type {
	ListVmsRequest,
	ListVmsResponse,
	GetVmRequest,
	GetVmResponse,
	CreateVmRequest,
	CreateVmResponse,
	MutateVmRequest,
	MutateVmResponse,
	GetVmConsoleUrlResponse
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

export async function createVm(req: CreateVmRequest, token?: string): Promise<CreateVmResponse> {
	return bffFetch<CreateVmResponse>(BFFEndpoints.createVm, {
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

export async function deleteVm(
	req: { vm_id: string; requested_by: string },
	token?: string
): Promise<{ vm_id: string; operation_id: string; status: string }> {
	return bffFetch(BFFEndpoints.deleteVm, { method: 'POST', body: JSON.stringify(req), token });
}


export async function getVmConsoleUrl(
	vm_id: string,
	token?: string
): Promise<GetVmConsoleUrlResponse> {
	return bffFetch<GetVmConsoleUrlResponse>(`/v1/vms/${vm_id}/console-url`, {
		method: 'GET',
		token
	});
}
