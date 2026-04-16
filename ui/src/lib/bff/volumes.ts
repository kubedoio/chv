import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';
import type {
	ListVolumesRequest,
	ListVolumesResponse,
	GetVolumeRequest,
	GetVolumeResponse
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
