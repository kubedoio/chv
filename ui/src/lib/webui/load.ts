import { browser } from '$app/environment';
import type { Event, NodeWithResources, Operation, StoragePool, VM } from '$lib/api/types';
import { getStoredToken } from '$lib/api/client';
import { buildOverviewModel, type OverviewModel } from '$lib/webui/overview';
import { buildTaskList, type TaskFilters, type TaskListModel } from '$lib/webui/tasks';

interface SnapshotLoadMeta {
	attempted: number;
	failed: number;
	partial: boolean;
	fetchFailed: boolean;
	clientRefreshRecommended: boolean;
	deferred: boolean;
	failures: {
		nodes: boolean;
		vms: boolean;
		storagePools: boolean;
		operations: boolean;
		events: boolean;
	};
}

interface OverviewPageData {
	overview: OverviewModel;
	meta: SnapshotLoadMeta;
}

interface TasksPageData {
	tasks: TaskListModel;
	meta: SnapshotLoadMeta;
}

type Fetcher = typeof fetch;

export async function loadOverviewPageData(fetcher: Fetcher): Promise<OverviewPageData> {
	const snapshotResult = await loadDashboardSnapshot(fetcher);

	return {
		overview: buildOverviewModel(snapshotResult.snapshot, {
			fetchFailed: snapshotResult.meta.fetchFailed
		}),
		meta: snapshotResult.meta
	};
}

export async function loadTasksPageData(
	fetcher: Fetcher,
	url: URL
): Promise<TasksPageData> {
	const snapshotResult = await loadDashboardSnapshot(fetcher);
	const filters = getTaskFilters(url);
	const page = Number(url.searchParams.get('page') ?? '1') || 1;

	return {
		tasks: buildTaskList(snapshotResult.snapshot, filters, {
			page,
			pageSize: 50,
			fetchFailed: snapshotResult.meta.fetchFailed,
			primaryDataUnavailable: snapshotResult.meta.failures.operations
		}),
		meta: snapshotResult.meta
	};
}

const SNAPSHOT_CACHE_TTL = 30000; // 30 seconds
let cachedSnapshot: any = null;
let lastSnapshotFetch = 0;

async function loadDashboardSnapshot(fetcher: Fetcher) {
	if (!browser) {
		return {
			snapshot: {
				nodes: [],
				vms: [],
				storagePools: [],
				operations: [],
				events: []
			},
			meta: {
				attempted: 0,
				failed: 0,
				partial: false,
				fetchFailed: false,
				clientRefreshRecommended: true,
				deferred: true,
				failures: {
					nodes: false,
					vms: false,
					storagePools: false,
					operations: false,
					events: false
				}
			}
		};
	}

	const now = Date.now();
	if (cachedSnapshot && now - lastSnapshotFetch < SNAPSHOT_CACHE_TTL) {
		// Use cached snapshot, but with fresh meta (cache hit)
		return {
			snapshot: cachedSnapshot.snapshot,
			meta: {
				...cachedSnapshot.meta,
				clientRefreshRecommended: false,
				deferred: false
			}
		};
	}

	const token = browser ? getStoredToken() : null;
	const requests = await Promise.all([
		loadJson<NodeWithResources[]>(fetcher, '/api/v1/nodes', token),
		loadJson<VM[]>(fetcher, '/api/v1/vms', token),
		loadJson<StoragePool[]>(fetcher, '/api/v1/storage-pools', token),
		loadJson<Operation[]>(fetcher, '/api/v1/operations', token),
		loadJson<Event[]>(fetcher, '/api/v1/events', token)
	]);
	const failures = {
		nodes: requests[0] === null,
		vms: requests[1] === null,
		storagePools: requests[2] === null,
		operations: requests[3] === null,
		events: requests[4] === null
	};
	const failed = requests.filter((request) => request === null).length;
	const attempted = requests.length;

	const result = {
		snapshot: {
			nodes: requests[0] ?? [],
			vms: requests[1] ?? [],
			storagePools: requests[2] ?? [],
			operations: requests[3] ?? [],
			events: requests[4] ?? []
		},
		meta: {
			attempted,
			failed,
			partial: failed > 0 && failed < attempted,
			fetchFailed: failed === attempted,
			clientRefreshRecommended: false,
			deferred: false,
			failures
		}
	};

	if (!result.meta.fetchFailed) {
		cachedSnapshot = result;
		lastSnapshotFetch = Date.now();
	}

	return result;
}

async function loadJson<T>(
	fetcher: Fetcher,
	path: string,
	token: string | null
): Promise<T | null> {
	try {
		const headers = new Headers();

		if (token) {
			headers.set('Authorization', `Bearer ${token}`);
		}

		const response = await fetcher(path, {
			headers,
			cache: 'no-store'
		});

		if (!response.ok) {
			return null;
		}

		return (await response.json()) as T;
	} catch {
		return null;
	}
}

function getTaskFilters(url: URL): TaskFilters {
	return {
		status: url.searchParams.get('status') ?? 'all',
		resourceKind: url.searchParams.get('resourceKind') ?? 'all',
		query: url.searchParams.get('query') ?? '',
		window: url.searchParams.get('window') ?? '7d'
	};
}
