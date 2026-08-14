import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';
import type {
	BackupHistory,
	ListBackupJobsResponse,
	ListBackupHistoryResponse,
	CreateBackupJobRequest,
	CreateBackupJobResponse,
	UpdateBackupJobRequest,
	ExecuteBackupJobResponse
} from './types';

export async function listBackupJobs(token?: string): Promise<ListBackupJobsResponse> {
	return bffFetch<ListBackupJobsResponse>(BFFEndpoints.listBackupJobs, {
		method: 'GET',
		token
	});
}

export async function createBackupJob(
	req: CreateBackupJobRequest,
	token?: string
): Promise<CreateBackupJobResponse> {
	return bffFetch<CreateBackupJobResponse>(BFFEndpoints.createBackupJob, {
		method: 'POST',
		body: JSON.stringify(req),
		token
	});
}

export async function updateBackupJob(
	jobId: string,
	req: UpdateBackupJobRequest,
	token?: string
): Promise<{ updated: boolean }> {
	return bffFetch<{ updated: boolean }>(`${BFFEndpoints.listBackupJobs}/${jobId}`, {
		method: 'PATCH',
		body: JSON.stringify(req),
		token
	});
}

export async function deleteBackupJob(
	jobId: string,
	token?: string
): Promise<{ deleted: boolean }> {
	return bffFetch<{ deleted: boolean }>(`${BFFEndpoints.listBackupJobs}/${jobId}`, {
		method: 'DELETE',
		token
	});
}

export async function executeBackupJob(
	jobId: string,
	token?: string
): Promise<ExecuteBackupJobResponse> {
	return bffFetch<ExecuteBackupJobResponse>(
		`${BFFEndpoints.listBackupJobs}/${jobId}/execute`,
		{
			method: 'POST',
			token
		}
	);
}

export async function listBackupHistory(
	page = 1,
	pageSize = 50,
	token?: string
): Promise<ListBackupHistoryResponse> {
	return bffFetch<ListBackupHistoryResponse>(BFFEndpoints.listBackupHistory, {
		method: 'POST',
		body: JSON.stringify({ page, page_size: pageSize }),
		token
	});
}
