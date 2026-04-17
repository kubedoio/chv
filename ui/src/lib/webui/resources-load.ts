import { browser } from '$app/environment';
import type { Event, Network, NodeWithResources, Operation, StoragePool, VM } from '$lib/api/types';
import { getStoredToken } from '$lib/api/client';
import {
	buildNodeDetail,
	buildNodesList,
	buildVmDetail,
	buildVmsList,
	type NodeDetailModel,
	type NodesListModel,
	type VmDetailModel,
	type VmsListModel
} from '$lib/webui/resources';

interface ResourceLoadMeta {
	deferred: boolean;
	partial: boolean;
	clientRefreshRecommended: boolean;
}

export interface NodesPageData {
	nodes: NodesListModel;
	meta: ResourceLoadMeta;
}

export interface NodeDetailPageData {
	detail: NodeDetailModel;
	meta: ResourceLoadMeta;
}

export interface VmsPageData {
	vms: VmsListModel;
	meta: ResourceLoadMeta;
}

export interface VmDetailPageData {
	detail: VmDetailModel;
	meta: ResourceLoadMeta;
	requestedVmId: string;
}

type Fetcher = typeof fetch;

export async function loadNodesPageData(fetcher: Fetcher, url: URL): Promise<NodesPageData> {
	if (!browser) {
		return {
			nodes: buildNodesList({ nodes: [], operations: [], events: [] }),
			meta: deferredMeta()
		};
	}

	const [nodes, operations, events] = await Promise.all([
		loadJson<NodeWithResources[]>(fetcher, '/api/v1/nodes'),
		loadJson<Operation[]>(fetcher, '/api/v1/operations'),
		loadJson<Event[]>(fetcher, '/api/v1/events')
	]);

	return {
		nodes: buildNodesList(
			{
				nodes: nodes.value ?? [],
				operations: operations.value ?? [],
				events: events.value ?? []
			},
			{
				query: url.searchParams.get('query') ?? '',
				state: url.searchParams.get('state') ?? 'all',
				maintenance: url.searchParams.get('maintenance') ?? 'all'
			},
			{ fetchFailed: [nodes, operations, events].every((item) => item.failed) }
		),
		meta: {
			deferred: false,
			clientRefreshRecommended: false,
			partial: [nodes, operations, events].some((item) => item.failed) && ![nodes, operations, events].every((item) => item.failed)
		}
	};
}

export async function loadNodeDetailPageData(
	fetcher: Fetcher,
	nodeId: string,
	url: URL
): Promise<NodeDetailPageData> {
	if (!browser) {
		return {
			detail: buildNodeDetail(
				{
					nodes: [],
					nodeVms: [],
					nodeStoragePools: [],
					nodeNetworks: [],
					operations: [],
					events: []
				},
				nodeId,
				url.searchParams.get('tab') ?? 'summary'
			),
			meta: deferredMeta()
		};
	}

	const [nodes, nodeVms, nodeStoragePools, nodeNetworks, operations, events] = await Promise.all([
		loadJson<NodeWithResources[]>(fetcher, '/api/v1/nodes'),
		loadJson<{ resources: VM[] }>(fetcher, `/api/v1/nodes/${nodeId}/vms`),
		loadJson<{ resources: StoragePool[] }>(fetcher, `/api/v1/nodes/${nodeId}/storage`),
		loadJson<{ resources: Network[] }>(fetcher, `/api/v1/nodes/${nodeId}/networks`),
		loadJson<Operation[]>(fetcher, '/api/v1/operations'),
		loadJson<Event[]>(fetcher, '/api/v1/events')
	]);

	return {
		detail: buildNodeDetail(
			{
				nodes: nodes.value ?? [],
				nodeVms: nodeVms.value?.resources ?? [],
				nodeStoragePools: nodeStoragePools.value?.resources ?? [],
				nodeNetworks: nodeNetworks.value?.resources ?? [],
				operations: operations.value ?? [],
				events: events.value ?? []
			},
			nodeId,
			url.searchParams.get('tab') ?? 'summary',
			{ fetchFailed: [nodes, nodeVms, nodeStoragePools, nodeNetworks, operations, events].every((item) => item.failed) }
		),
		meta: {
			deferred: false,
			clientRefreshRecommended: false,
			partial:
				[nodes, nodeVms, nodeStoragePools, nodeNetworks, operations, events].some((item) => item.failed) &&
				![nodes, nodeVms, nodeStoragePools, nodeNetworks, operations, events].every((item) => item.failed)
		}
	};
}

export async function loadVmsPageData(fetcher: Fetcher, url: URL): Promise<VmsPageData> {
	if (!browser) {
		return {
			vms: buildVmsList({ vms: [], nodes: [], vmPlacements: {}, operations: [], events: [] }),
			meta: deferredMeta()
		};
	}

	const [nodes, vms, operations, events] = await Promise.all([
		loadJson<NodeWithResources[]>(fetcher, '/api/v1/nodes'),
		loadJson<VM[]>(fetcher, '/api/v1/vms'),
		loadJson<Operation[]>(fetcher, '/api/v1/operations'),
		loadJson<Event[]>(fetcher, '/api/v1/events')
	]);

	return {
		vms: buildVmsList(
			{
				vms: vms.value ?? [],
				nodes: nodes.value ?? [],
				vmPlacements: {},
				operations: operations.value ?? [],
				events: events.value ?? []
			},
			{
				query: url.searchParams.get('query') ?? '',
				powerState: url.searchParams.get('powerState') ?? 'all',
				health: url.searchParams.get('health') ?? 'all',
				nodeId: url.searchParams.get('nodeId') ?? 'all'
			},
			{ fetchFailed: [nodes, vms, operations, events].every((item) => item.failed) }
		),
		meta: {
			deferred: false,
			clientRefreshRecommended: false,
			partial:
				[nodes, vms, operations, events].some((item) => item.failed) &&
				![nodes, vms, operations, events].every((item) => item.failed)
		}
	};
}

export async function loadVmDetailPageData(
	fetcher: Fetcher,
	vmId: string,
	url: URL
): Promise<VmDetailPageData> {
	if (!browser) {
		return {
			detail: buildVmDetail(
				{
					vm: null,
					nodes: [],
					vmPlacements: {},
					storagePools: [],
					networks: [],
					operations: [],
					events: []
				},
				url.searchParams.get('tab') ?? 'summary'
			),
			meta: deferredMeta(),
			requestedVmId: vmId
		};
	}

	const [vm, nodes, storagePools, networks, operations, events] = await Promise.all([
		loadJson<VM>(fetcher, `/api/v1/vms/${vmId}`),
		loadJson<NodeWithResources[]>(fetcher, '/api/v1/nodes'),
		loadJson<StoragePool[]>(fetcher, '/api/v1/storage-pools'),
		loadJson<Network[]>(fetcher, '/api/v1/networks'),
		loadJson<Operation[]>(fetcher, '/api/v1/operations'),
		loadJson<Event[]>(fetcher, '/api/v1/events')
	]);

	return {
		detail: buildVmDetail(
			{
				vm: vm.value ?? null,
				nodes: nodes.value ?? [],
				vmPlacements: {},
				storagePools: storagePools.value ?? [],
				networks: networks.value ?? [],
				operations: operations.value ?? [],
				events: events.value ?? []
			},
			url.searchParams.get('tab') ?? 'summary',
			{ fetchFailed: [vm, nodes, storagePools, networks, operations, events].every((item) => item.failed) }
		),
		meta: {
			deferred: false,
			clientRefreshRecommended: false,
			partial:
				[vm, nodes, storagePools, networks, operations, events].some((item) => item.failed) &&
				![vm, nodes, storagePools, networks, operations, events].every((item) => item.failed)
		},
		requestedVmId: vmId
	};
}

const RESOURCE_CACHE_TTL = 30000;
const requestCache = new Map<string, { value: any; failed: boolean; timestamp: number }>();

async function loadJson<T>(fetcher: Fetcher, path: string, explicitToken?: string | null) {
	const now = Date.now();
	const cached = requestCache.get(path);
	if (cached && now - cached.timestamp < RESOURCE_CACHE_TTL) {
		return { value: cached.value as T, failed: cached.failed };
	}

	const token = explicitToken === undefined ? getStoredToken() : explicitToken;
	try {
		const headers = new Headers();
		if (token) {
			headers.set('Authorization', `Bearer ${token}`);
		}

		const response = await fetcher(path, { headers, cache: 'no-store' });
		if (!response.ok) {
			const failedResult = { value: null as T | null, failed: true };
			// We only want to cache successful requests really, but let's cache failures for a short duration too to prevent hammering
			return failedResult;
		}

		const value = (await response.json()) as T;
		requestCache.set(path, { value, failed: false, timestamp: now });
		return { value, failed: false };
	} catch {
		return { value: null as T | null, failed: true };
	}
}

function deferredMeta(): ResourceLoadMeta {
	return {
		deferred: true,
		partial: false,
		clientRefreshRecommended: true
	};
}
