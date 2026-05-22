import { browser } from '$app/environment';
import type { Event, NodeWithResources, Operation, StoragePool, VM } from '$lib/api/types';
import { getStoredToken } from '$lib/api/client';
import {
	loadEventsFromBff,
	loadNodesFromBff,
	loadOperationsFromBff,
	loadVmsFromBff
} from '$lib/webui/bff-resources';
import { buildOverviewModel, type OverviewModel } from '$lib/webui/overview';
import { loadStoragePoolsFromBff } from '$lib/webui/storage-pools';
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
	void fetcher;

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
		loadBffNodes(token),
		loadBffVms(token),
		loadBffStoragePools(token),
		loadBffOperations(token),
		loadBffEvents(token)
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

async function loadBffStoragePools(token: string | null): Promise<StoragePool[] | null> {
	try {
		return await loadStoragePoolsFromBff(token ?? undefined);
	} catch {
		return null;
	}
}

async function loadBffNodes(token: string | null): Promise<NodeWithResources[] | null> {
	try {
		return await loadNodesFromBff(token ?? undefined);
	} catch {
		return null;
	}
}

async function loadBffVms(token: string | null): Promise<VM[] | null> {
	try {
		return await loadVmsFromBff(token ?? undefined);
	} catch {
		return null;
	}
}

async function loadBffOperations(token: string | null): Promise<Operation[] | null> {
	try {
		return await loadOperationsFromBff(token ?? undefined);
	} catch {
		return null;
	}
}

async function loadBffEvents(token: string | null): Promise<Event[] | null> {
	try {
		return await loadEventsFromBff(token ?? undefined);
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
