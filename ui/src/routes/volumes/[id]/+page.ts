import type { PageLoad } from './$types';
import { getStoredToken } from '$lib/api/client';
import { getVolume } from '$lib/bff/volumes';
import type { VolumeSummary, RelatedTask } from '$lib/bff/types';

export type VolumeDetailModel = {
	state: 'ready' | 'empty' | 'error';
	currentTab: string;
	summary: {
		volume_id: string;
		name: string;
		node_id: string;
		size: string;
		capacity_bytes?: number;
		status: string;
		health: string;
		attached_vm_id: string;
		attached_vm_name: string;
		device_name?: string;
		read_only?: boolean;
		volume_kind?: string;
		storage_class?: string;
	};
	sections: { id: string; label: string; count?: number }[];
	recent_tasks: RelatedTask[];
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
				volume_id: '',
				name: '',
				node_id: '',
				size: '',
				status: '',
				health: '',
				attached_vm_id: '',
				attached_vm_name: '',
				device_name: '',
				read_only: false,
				volume_kind: '',
				storage_class: ''
			},
			sections: buildSections(null),
			recent_tasks: [],
			configuration: []
		};
	}

	return {
		state: 'ready',
		currentTab,
		summary: {
			volume_id: summary.volume_id,
			name: summary.name,
			node_id: summary.node_id,
			size: summary.size,
			capacity_bytes: summary.capacity_bytes,
			status: summary.status,
			health: summary.health,
			attached_vm_id: summary.attached_vm_id,
			attached_vm_name: summary.attached_vm_name,
			device_name: summary.device_name,
			read_only: summary.read_only,
			volume_kind: summary.volume_kind,
			storage_class: summary.storage_class
		},
		sections: buildSections(summary),
		recent_tasks: summary.recent_tasks ?? [],
		configuration: buildConfiguration(summary)
	};
}

export const load: PageLoad = async ({ params, url }) => {
	const token = getStoredToken() ?? undefined;
	const currentTab = url.searchParams.get('tab') ?? 'summary';
	try {
		const res = await getVolume({ volume_id: params.id }, token);
		const detail = buildDetailModel(res.summary ?? null, currentTab);
		return { detail, requestedVolumeId: params.id };
	} catch {
		const detail: VolumeDetailModel = {
			state: 'error',
			currentTab,
			summary: {
				volume_id: params.id,
				name: '',
				node_id: '',
				size: '',
				status: '',
				health: '',
				attached_vm_id: '',
				attached_vm_name: ''
			},
			sections: buildSections(null),
			recent_tasks: [],
			configuration: []
		};
		return { detail, requestedVolumeId: params.id };
	}
};
