import { getStoredToken } from '$lib/api/client';
import type { NodeWithResources, VM } from '$lib/api/types';
import { listNodes } from '$lib/bff/nodes';
import { listVms } from '$lib/bff/vms';
import type { NodeListItem, VmListItem } from '$lib/bff/types';

function normalizeNodeStatus(state: string): NodeWithResources['status'] {
	const s = state.toLowerCase();
	if (s.includes('ready') || s.includes('online') || s.includes('active')) return 'online';
	if (s.includes('error') || s.includes('fail')) return 'error';
	if (s.includes('maint')) return 'maintenance';
	return 'offline';
}

function normalizeVmState(state: string): string {
	return state.toLowerCase();
}

function mapNode(item: NodeListItem): NodeWithResources {
	return {
		id: item.node_id,
		name: item.name,
		hostname: item.name,
		ip_address: '',
		status: normalizeNodeStatus(item.state),
		is_local: false,
		resources: {
			vms: 0,
			images: 0,
			storage_pools: 0,
			networks: 0
		},
		capabilities: '',
		last_seen_at: '',
		created_at: '',
		updated_at: ''
	};
}

function mapVm(item: VmListItem): VM {
	const state = normalizeVmState(item.power_state);
	return {
		id: item.vm_id,
		name: item.name,
		node_id: item.node_id,
		image_id: '',
		storage_pool_id: '',
		network_id: '',
		desired_state: state,
		actual_state: state,
		vcpu: 0,
		memory_mb: 0,
		disk_path: '',
		seed_iso_path: '',
		workspace_path: '',
		ip_address: '',
		mac_address: '',
		console_type: 'serial'
	};
}

class InventoryStore {
	nodes = $state<NodeWithResources[]>([]);
	vms = $state<VM[]>([]);
	isLoading = $state(true);

	async fetch() {
		const token = getStoredToken();
		if (!token) {
			this.isLoading = false;
			return;
		}

		try {
			const [nodesRes, vmsRes] = await Promise.all([
				listNodes({ page: 1, page_size: 100, filters: {} }, token),
				listVms({ page: 1, page_size: 100, filters: {} }, token)
			]);
			this.nodes = (nodesRes.items || []).map(mapNode);
			this.vms = (vmsRes.items || []).map(mapVm);
		} catch (err) {
			// TODO: integrate structured logger instead of console
			// eslint-disable-next-line no-console
			console.error('Failed to load local inventory:', err);
			this.nodes = [];
			this.vms = [];
		} finally {
			this.isLoading = false;
		}
	}
}

export const inventory = new InventoryStore();
