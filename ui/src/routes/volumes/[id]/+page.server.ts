import type { PageServerLoad, Actions } from './$types';
import { getVolume } from '$lib/bff/volumes';
import { handleVolumeMutation } from '$lib/webui/volume-server-actions';
import type { VolumeSummary, RelatedTask } from '$lib/bff/types';

export type VolumeDetailModel = {
	state: 'ready' | 'empty' | 'error';
	currentTab: string;
	summary: {
		volumeId: string;
		name: string;
		nodeId: string;
		size: string;
		status: string;
		health: string;
		attachedVmId: string;
		attachedVmName: string;
	};
	sections: { id: string; label: string; count?: number }[];
	recentTasks: RelatedTask[];
	configuration: Array<{ label: string; value: string }>;
};

function buildSections(summary: VolumeSummary | null): { id: string; label: string; count?: number }[] {
	const taskCount = summary?.recent_tasks?.length ?? 0;
	return [
		{ id: 'summary', label: 'Summary' },
		{ id: 'tasks', label: 'Tasks', count: taskCount },
		{ id: 'configuration', label: 'Configuration' }
	];
}

function buildConfiguration(summary: VolumeSummary): Array<{ label: string; value: string }> {
	return [
		{ label: 'Volume ID', value: summary.volume_id },
		{ label: 'Name', value: summary.name },
		{ label: 'Node ID', value: summary.node_id },
		{ label: 'Size', value: summary.size },
		{ label: 'Status', value: summary.status },
		{ label: 'Health', value: summary.health },
		{ label: 'Volume Kind', value: summary.volume_kind },
		{ label: 'Storage Class', value: summary.storage_class },
		{ label: 'Device Name', value: summary.device_name },
		{ label: 'Read Only', value: summary.read_only ? 'Yes' : 'No' },
		{ label: 'Attached VM', value: summary.attached_vm_name || summary.attached_vm_id || '-' }
	];
}

function buildDetailModel(summary: VolumeSummary | null, currentTab: string): VolumeDetailModel {
	if (!summary) {
		return {
			state: 'empty',
			currentTab,
			summary: {
				volumeId: '',
				name: '',
				nodeId: '',
				size: '',
				status: '',
				health: '',
				attachedVmId: '',
				attachedVmName: ''
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
			volumeId: summary.volume_id,
			name: summary.name,
			nodeId: summary.node_id,
			size: summary.size,
			status: summary.status,
			health: summary.health,
			attachedVmId: summary.attached_vm_id,
			attachedVmName: summary.attached_vm_name
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
		const res = await getVolume({ volume_id: params.id }, token);
		const detail = buildDetailModel(res.summary ?? null, currentTab);
		return { detail, requestedVolumeId: params.id };
	} catch (err) {
		const detail: VolumeDetailModel = {
			state: 'error',
			currentTab,
			summary: {
				volumeId: params.id,
				name: '',
				nodeId: '',
				size: '',
				status: '',
				health: '',
				attachedVmId: '',
				attachedVmName: ''
			},
			sections: buildSections(null),
			recentTasks: [],
			configuration: []
		};
		return { detail, requestedVolumeId: params.id };
	}
};

export const actions: Actions = {
	default: async ({ request, cookies }) => {
		const token = cookies.get('chv_session') ?? undefined;
		const formData = await request.formData();
		return handleVolumeMutation(formData, token);
	}
};
