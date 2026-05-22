import { browser } from '$app/environment';
import type { Event, Network, NodeWithResources, Operation, StoragePool, VM } from '$lib/api/types';
import { getStoredToken } from '$lib/api/client';
import {
	loadEventsFromBff,
	loadNetworksFromBff,
	loadNodesFromBff,
	loadOperationsFromBff,
	loadVmFromBff,
	loadVmsFromBff
} from '$lib/webui/bff-resources';
import { loadStoragePoolsFromBff } from '$lib/webui/storage-pools';
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
	void fetcher;

	if (!browser) {
		return {
			nodes: buildNodesList({ nodes: [], operations: [], events: [] }),
			meta: deferredMeta()
		};
	}

	const [nodes, operations, events] = await Promise.all([
		loadBffNodes(),
		loadBffOperations(),
		loadBffEvents()
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
	void fetcher;

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
		loadBffNodes(),
		loadBffVmsForNode(nodeId),
		loadBffStoragePoolsForNode(nodeId),
		loadBffNetworksForNode(nodeId),
		loadBffOperations(),
		loadBffEvents()
	]);

	return {
		detail: buildNodeDetail(
			{
				nodes: nodes.value ?? [],
				nodeVms: nodeVms.value ?? [],
				nodeStoragePools: nodeStoragePools.value ?? [],
				nodeNetworks: nodeNetworks.value ?? [],
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
	void fetcher;

	if (!browser) {
		return {
			vms: buildVmsList({ vms: [], nodes: [], vmPlacements: {}, operations: [], events: [] }),
			meta: deferredMeta()
		};
	}

	const [nodes, vms, operations, events] = await Promise.all([
		loadBffNodes(),
		loadBffVms(),
		loadBffOperations(),
		loadBffEvents()
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
	void fetcher;

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
		loadBffVm(vmId),
		loadBffNodes(),
		loadBffStoragePools(),
		loadBffNetworks(),
		loadBffOperations(),
		loadBffEvents()
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

async function loadBffStoragePools() {
	try {
		return { value: await loadStoragePoolsFromBff(getStoredToken() ?? undefined), failed: false };
	} catch {
		return { value: null as StoragePool[] | null, failed: true };
	}
}

async function loadBffNodes() {
	try {
		return { value: await loadNodesFromBff(getStoredToken() ?? undefined), failed: false };
	} catch {
		return { value: null as NodeWithResources[] | null, failed: true };
	}
}

async function loadBffVms() {
	try {
		return { value: await loadVmsFromBff(getStoredToken() ?? undefined), failed: false };
	} catch {
		return { value: null as VM[] | null, failed: true };
	}
}

async function loadBffVm(vmId: string) {
	try {
		return { value: await loadVmFromBff(vmId, getStoredToken() ?? undefined), failed: false };
	} catch {
		return { value: null as VM | null, failed: true };
	}
}

async function loadBffVmsForNode(nodeId: string) {
	const result = await loadBffVms();
	return {
		value: result.value?.filter((vm) => vm.node_id === nodeId) ?? null,
		failed: result.failed
	};
}

async function loadBffStoragePoolsForNode(_nodeId: string) {
	return loadBffStoragePools();
}

async function loadBffNetworks() {
	try {
		return { value: await loadNetworksFromBff(getStoredToken() ?? undefined), failed: false };
	} catch {
		return { value: null as Network[] | null, failed: true };
	}
}

async function loadBffNetworksForNode(_nodeId: string) {
	return loadBffNetworks();
}

async function loadBffOperations() {
	try {
		return { value: await loadOperationsFromBff(getStoredToken() ?? undefined), failed: false };
	} catch {
		return { value: null as Operation[] | null, failed: true };
	}
}

async function loadBffEvents() {
	try {
		return { value: await loadEventsFromBff(getStoredToken() ?? undefined), failed: false };
	} catch {
		return { value: null as Event[] | null, failed: true };
	}
}

function deferredMeta(): ResourceLoadMeta {
	return {
		deferred: true,
		partial: false,
		clientRefreshRecommended: true
	};
}
