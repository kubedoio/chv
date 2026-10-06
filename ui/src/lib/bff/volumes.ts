import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';
import type {
	ListVolumesRequest,
	ListVolumesResponse,
	GetVolumeRequest,
	GetVolumeResponse,
	CreateVolumeRequest,
	CreateVolumeResponse,
	MutateVolumeRequest,
	MutateVolumeResponse,
	DeleteVolumeRequest,
	DeleteVolumeResponse
} from './types';

export async function listVolumes(req: ListVolumesRequest, token?: string): Promise<ListVolumesResponse> {
	return bffFetch<ListVolumesResponse>(BFFEndpoints.listVolumes, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

export async function getVolume(req: GetVolumeRequest, token?: string): Promise<GetVolumeResponse> {
	return bffFetch<GetVolumeResponse>(BFFEndpoints.getVolume, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

export async function createVolume(req: CreateVolumeRequest, token?: string): Promise<CreateVolumeResponse> {
	return bffFetch<CreateVolumeResponse>(BFFEndpoints.createVolume, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

export async function mutateVolume(req: MutateVolumeRequest, token?: string): Promise<MutateVolumeResponse> {
	return bffFetch<MutateVolumeResponse>(BFFEndpoints.mutateVolume, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

// #522 DP12 (PR 4): the volume-delete route PR 2 landed (#535). The
// one-key body is built by `buildDeleteVolumePayload`
// (`$lib/webui/volume-delete`) so the contract is pinned by tests.
export async function deleteVolume(req: DeleteVolumeRequest, token?: string): Promise<DeleteVolumeResponse> {
	return bffFetch<DeleteVolumeResponse>(BFFEndpoints.deleteVolume, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}
