import type { PageLoad } from './$types';
import { getStoredToken } from '$lib/api/client';
import { getNode } from '$lib/bff/nodes';
import type { GetNodeResponse } from '$lib/bff/types';

export type NodeDetailModel = {
	state: 'ready' | 'empty' | 'error';
	currentTab: string;
	summary: {
		nodeId: string;
		name: string;
		cluster: string;
		state: string;
		health: string;
		version: string;
		cpu: string;
		memory: string;
		storage: string;
		network: string;
		maintenance: boolean;
		scheduling: boolean;
	};
	sections: { id: string; label: string; count?: number }[];
	hostedVms: {
		vm_id: string;
		name: string;
		power_state: string;
		health: string;
		cpu: string;
		memory: string;
	}[];
	recentTasks: { task_id: string; status: string; summary: string; operation: string; started_unix_ms: number }[];
	configuration: Array<{ label: string; value: string }>;
};

function mapDetail(res: GetNodeResponse | null, currentTab: string, fallbackId: string): NodeDetailModel {
	if (!res || !res.summary) {
		return {
			state: 'error',
			currentTab,
			summary: {
				nodeId: fallbackId,
				name: fallbackId,
				cluster: '',
				state: '',
				health: '',
				version: '',
				cpu: '',
				memory: '',
				storage: '',
				network: '',
				maintenance: false,
				scheduling: false
			},
			sections: [
				{ id: 'summary', label: 'Summary' },
				{ id: 'vms', label: 'VMs', count: 0 },
				{ id: 'tasks', label: 'Tasks', count: 0 },
				{ id: 'configuration', label: 'Configuration' }
			],
			hostedVms: [],
			recentTasks: [],
			configuration: [{ label: 'Node ID', value: fallbackId }]
		};
	}

	const summary = res.summary;
	return {
		state: res.state ?? 'ready',
		currentTab,
		summary: {
			nodeId: summary.node_id,
			name: summary.name,
			cluster: summary.cluster,
			state: summary.state,
			health: summary.health,
			version: summary.version,
			cpu: summary.cpu,
			memory: summary.memory,
			storage: summary.storage,
			network: summary.network,
			maintenance: (summary as unknown as { maintenance?: boolean }).maintenance ?? false,
			scheduling: (summary as unknown as { scheduling?: boolean }).scheduling ?? false
		},
		sections: res.sections ?? [
			{ id: 'summary', label: 'Summary' },
			{ id: 'vms', label: 'VMs', count: res.hostedVms?.length ?? 0 },
			{ id: 'tasks', label: 'Tasks', count: res.recentTasks?.length ?? 0 },
			{ id: 'configuration', label: 'Configuration' }
		],
		hostedVms: res.hostedVms ?? [],
		recentTasks: res.recentTasks ?? [],
		configuration: res.configuration ?? [
			{ label: 'Node ID', value: summary.node_id },
			{ label: 'Version', value: summary.version },
			{ label: 'CPU', value: summary.cpu },
			{ label: 'Memory', value: summary.memory },
			{ label: 'Storage backend', value: 'zfs' }
		]
	};
}

export const load: PageLoad = async ({ params, url }) => {
	const token = getStoredToken() ?? undefined;
	const currentTab = url.searchParams.get('tab') ?? 'summary';
	try {
		const res = await getNode({ node_id: params.id }, token);
		const detail = mapDetail(res, currentTab, params.id);
		return { detail, requestedNodeId: params.id };
	} catch {
		const detail = mapDetail(null, currentTab, params.id);
		return { detail, requestedNodeId: params.id };
	}
};
