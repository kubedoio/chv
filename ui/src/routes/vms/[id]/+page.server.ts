import type { PageServerLoad, Actions } from './$types';
import { getVm } from '$lib/bff/vms';
import { BFFError } from '$lib/bff/client';
import { handleVmMutation } from '$lib/webui/vm-server-actions';
import type { VmSummary, RelatedTask } from '$lib/bff/types';

export type VmDetailModel = {
	state: 'ready' | 'empty' | 'error';
	currentTab: string;
	summary: {
		vmId: string;
		name: string;
		nodeId: string;
		powerState: string;
		health: string;
		cpu: string;
		memory: string;
	};
	sections: { id: string; label: string; count?: number }[];
	recentTasks: RelatedTask[];
	configuration: Array<{ label: string; value: string }>;
};

function buildSections(summary: VmSummary | null): { id: string; label: string; count?: number }[] {
	const taskCount = summary?.recent_tasks?.length ?? 0;
	return [
		{ id: 'summary', label: 'Summary' },
		{ id: 'console', label: 'Console' },
		{ id: 'volumes', label: 'Volumes' },
		{ id: 'networks', label: 'Networks' },
		{ id: 'tasks', label: 'Tasks', count: taskCount },
		{ id: 'events', label: 'Events' },
		{ id: 'configuration', label: 'Configuration' }
	];
}

function buildConfiguration(summary: VmSummary): Array<{ label: string; value: string }> {
	return [
		{ label: 'VM ID', value: summary.vm_id },
		{ label: 'Name', value: summary.name },
		{ label: 'Node ID', value: summary.node_id },
		{ label: 'Power State', value: summary.power_state },
		{ label: 'Health', value: summary.health },
		{ label: 'CPU', value: summary.cpu },
		{ label: 'Memory', value: summary.memory }
	];
}

function buildDetailModel(summary: VmSummary | null, currentTab: string): VmDetailModel {
	if (!summary) {
		return {
			state: 'empty',
			currentTab,
			summary: {
				vmId: '',
				name: '',
				nodeId: '',
				powerState: '',
				health: '',
				cpu: '',
				memory: ''
			},
			sections: buildSections(null),
			recentTasks: [],
			configuration: []
		};
	}

	return {
		state: 'ready',
		currentTab,
		summary: {
			vmId: summary.vm_id,
			name: summary.name,
			nodeId: summary.node_id,
			powerState: summary.power_state,
			health: summary.health,
			cpu: summary.cpu,
			memory: summary.memory
		},
		sections: buildSections(summary),
		recentTasks: summary.recent_tasks ?? [],
		configuration: buildConfiguration(summary)
	};
}

export const load: PageServerLoad = async ({ params, url, cookies }) => {
	const token = cookies.get('chv_session') ?? undefined;
	const currentTab = url.searchParams.get('tab') ?? 'summary';
	try {
		const res = await getVm({ vm_id: params.id }, token);
		const detail = buildDetailModel(res.summary ?? null, currentTab);
		return { detail, requestedVmId: params.id };
	} catch (err) {
		const detail: VmDetailModel = {
			state: 'error',
			currentTab,
			summary: {
				vmId: params.id,
				name: '',
				nodeId: '',
				powerState: '',
				health: '',
				cpu: '',
				memory: ''
			},
			sections: buildSections(null),
			recentTasks: [],
			configuration: []
		};
		return { detail, requestedVmId: params.id };
	}
};

export const actions: Actions = {
	default: async ({ request, cookies }) => {
		const token = cookies.get('chv_session') ?? undefined;
		const formData = await request.formData();
		return handleVmMutation(formData, token);
	}
};
