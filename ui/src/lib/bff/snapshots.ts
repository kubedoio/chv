import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';
import type {
	ListVmSnapshotsRequest,
	ListVmSnapshotsResponse,
	CreateSnapshotRequest,
	CreateSnapshotResponse,
	DeleteSnapshotRequest,
	DeleteSnapshotResponse,
	RestoreSnapshotRequest,
	RestoreSnapshotResponse
} from './types';

export async function listVmSnapshots(
	req: ListVmSnapshotsRequest,
	token?: string
): Promise<ListVmSnapshotsResponse> {
	return bffFetch<ListVmSnapshotsResponse>(BFFEndpoints.listVmSnapshots, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

export async function createSnapshot(
	req: CreateSnapshotRequest,
	token?: string
): Promise<CreateSnapshotResponse> {
	return bffFetch<CreateSnapshotResponse>(BFFEndpoints.createSnapshot, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

export async function deleteSnapshot(
	req: DeleteSnapshotRequest,
	token?: string
): Promise<DeleteSnapshotResponse> {
	return bffFetch<DeleteSnapshotResponse>(BFFEndpoints.deleteSnapshot, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

export async function restoreSnapshot(
	req: RestoreSnapshotRequest,
	token?: string
): Promise<RestoreSnapshotResponse> {
	return bffFetch<RestoreSnapshotResponse>(BFFEndpoints.restoreSnapshot, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}
