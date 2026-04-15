import type { PageServerLoad } from './$types';
import { getNode } from '$lib/bff/nodes';
import { listVms } from '$lib/bff/vms';
import { BFFError } from '$lib/bff/client';
import type { NodeSummary, VmListItem, RelatedTask } from '$lib/bff/types';

type NodeDetailModel = {
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
	};
	sections: { id: string; label: string; count?: number }[];
	hostedVms: VmListItem[];
	hasMoreVms: boolean;
	recentTasks: RelatedTask[];
	configuration: Array<{ label: string; value: string }>;
};

function buildConfiguration(summary: NodeSummary): Array<{ label: string; value: string }> {
	return [
		{ label: 'Node ID', value: summary.node_id },
		{ label: 'Name', value: summary.name },
		{ label: 'Cluster', value: summary.cluster },
		{ label: 'State', value: summary.state },
		{ label: 'Health', value: summary.health },
		{ label: 'Version', value: summary.version },
		{ label: 'CPU', value: summary.cpu },
		{ label: 'Memory', value: summary.memory },
		{ label: 'Storage', value: summary.storage },
		{ label: 'Network', value: summary.network }
	];
}

function buildSections(
	hostedVms: VmListItem[],
	recentTasks: RelatedTask[]
): { id: string; label: string; count?: number }[] {
	return [
		{ id: 'summary', label: 'Summary' },
		{ id: 'vms', label: 'VMs', count: hostedVms.length },
		{ id: 'volumes', label: 'Volumes' },
		{ id: 'networks', label: 'Networks' },
		{ id: 'tasks', label: 'Tasks', count: recentTasks.length },
		{ id: 'events', label: 'Events' },
		{ id: 'configuration', label: 'Configuration' }
	];
}

export const load: PageServerLoad = async ({ params, url, cookies }) => {
	const token = cookies.get('chv_session') ?? undefined;
	const currentTab = url.searchParams.get('tab') ?? 'summary';

	try {
		const [nodeRes, vmsRes] = await Promise.all([
			getNode({ node_id: params.id }, token),
			listVms({ page: 1, page_size: 1000, filters: { nodeId: params.id } }, token)
		]);

		const summary = nodeRes.summary;
		const hostedVms = vmsRes.items ?? [];
		const hasMoreVms =
			hostedVms.length === 1000 || (vmsRes.page?.total_items ?? 0) > 1000;
		const recentTasks = summary.recent_tasks ?? [];
		const configuration = buildConfiguration(summary);
		const sections = buildSections(hostedVms, recentTasks);

		const detail: NodeDetailModel = {
			state: 'ready',
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
				network: summary.network
			},
			sections,
			hostedVms,
			hasMoreVms,
			recentTasks,
			configuration
		};

		return { detail };
	} catch (err) {
		// eslint-disable-next-line no-console
		console.error('BFF node detail error:', err);
		const detail: NodeDetailModel = {
			state: 'error',
			currentTab,
			summary: {
				nodeId: params.id,
				name: '',
				cluster: '',
				state: '',
				health: '',
				version: '',
				cpu: '',
				memory: '',
				storage: '',
				network: ''
			},
			sections: [
				{ id: 'summary', label: 'Summary' },
				{ id: 'vms', label: 'VMs' },
				{ id: 'volumes', label: 'Volumes' },
				{ id: 'networks', label: 'Networks' },
				{ id: 'tasks', label: 'Tasks' },
				{ id: 'events', label: 'Events' },
				{ id: 'configuration', label: 'Configuration' }
			],
			hostedVms: [],
			hasMoreVms: false,
			recentTasks: [],
			configuration: []
		};
		return { detail };
	}
};
