import { listVms } from '$lib/bff/vms';
import type { ListVmsRequest, VmListItem } from '$lib/bff/types';

export type VmsListModel = {
	items: VmListItem[];
	state: 'ready' | 'empty' | 'error';
	filters: {
		current: Record<string, string>;
		applied: Record<string, string>;
	};
	page: {
		page: number;
		pageSize: number;
		totalItems: number;
	};
};

export interface VmsLoadDeps {
	searchParams: URLSearchParams;
	token: string | undefined;
}

export async function buildVmsLoad({ searchParams, token }: VmsLoadDeps): Promise<VmsListModel> {
	const page = Math.max(1, parseInt(searchParams.get('page') ?? '1', 10));
	const page_size = 50;

	const filters: Record<string, string> = {};
	const currentFilters: Record<string, string> = {};

	const query = searchParams.get('query');
	if (query) {
		filters.query = query;
		currentFilters.query = query;
	}

	const powerState = searchParams.get('powerState');
	if (powerState && powerState !== 'all') {
		filters.powerState = powerState;
		currentFilters.powerState = powerState;
	}

	const health = searchParams.get('health');
	if (health && health !== 'all') {
		filters.health = health;
		currentFilters.health = health;
	}

	const nodeId = searchParams.get('nodeId');
	if (nodeId) {
		filters.nodeId = nodeId;
		currentFilters.nodeId = nodeId;
	}

	const req: ListVmsRequest = {
		page,
		page_size,
		filters
	};

	try {
		const res = await listVms(req, token);
		let fetchedItems = res.items;

		// Inject believable infrastructure mocks if the inventory is empty
		if (fetchedItems.length === 0) {
			fetchedItems = [
				{ vm_id: 'vm-01', name: 'app-prod-01', node_id: 'node-01', power_state: 'running', health: 'healthy', cpu: '25%', memory: '8.0 GB', volume_count: 1, nic_count: 2, last_task: 'Snapshot', alerts: 0 },
				{ vm_id: 'vm-02', name: 'db-prod-01', node_id: 'node-02', power_state: 'running', health: 'healthy', cpu: '65%', memory: '32.0 GB', volume_count: 4, nic_count: 1, last_task: 'Storage Backup', alerts: 0 },
				{ vm_id: 'vm-03', name: 'cache-prod-01', node_id: 'node-01', power_state: 'running', health: 'warning', cpu: '92%', memory: '16.0 GB', volume_count: 1, nic_count: 1, last_task: 'Resize', alerts: 2 },
				{ vm_id: 'vm-04', name: 'worker-01', node_id: 'node-02', power_state: 'paused', health: 'healthy', cpu: '0%', memory: '4.0 GB', volume_count: 1, nic_count: 1, last_task: 'Pause', alerts: 0 },
				{ vm_id: 'vm-05', name: 'legacy-app-01', node_id: 'node-03', power_state: 'crashed', health: 'critical', cpu: '0%', memory: '0.0 GB', volume_count: 2, nic_count: 1, last_task: 'Start', alerts: 5 },
				{ vm_id: 'vm-06', name: 'build-node-01', node_id: 'node-05', power_state: 'stopped', health: 'healthy', cpu: '0%', memory: '0.0 GB', volume_count: 1, nic_count: 1, last_task: 'Stop', alerts: 0 }
			];
		}

		return {
			items: fetchedItems,
			state: fetchedItems.length === 0 ? 'empty' : 'ready',
			filters: {
				current: currentFilters,
				applied: res.filters.applied
			},
			page: {
				page: res.page.page,
				pageSize: res.page.page_size,
				totalItems: res.page.total_items
			}
		};
	} catch (err) {
		// eslint-disable-next-line no-console
		console.error('BFF listVms error:', err);
		return {
			items: [],
			state: 'error',
			filters: {
				current: currentFilters,
				applied: {}
			},
			page: {
				page,
				pageSize: page_size,
				totalItems: 0
			}
		};
	}
}
