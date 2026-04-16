import type { PageServerLoad } from './$types';
import { error } from '@sveltejs/kit';

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
	hostedVms: { vm_id: string; name: string; power_state: string; health: string; cpu: string; memory: string }[];
	recentTasks: { task_id: string; status: string; summary: string; operation: string; started_unix_ms: number }[];
	configuration: Array<{ label: string; value: string }>;
};

const mockNodes: Record<string, NodeDetailModel> = {
	'n-ber-1-c01': {
		state: 'ready',
		currentTab: 'summary',
		summary: {
			nodeId: 'n-ber-1-c01',
			name: 'ber-1-c01',
			cluster: 'eu-west-core',
			state: 'host_ready',
			health: 'healthy',
			version: '1.4.2',
			cpu: '62%',
			memory: '58%',
			storage: 'healthy',
			network: 'healthy',
			maintenance: false,
			scheduling: true
		},
		sections: [
			{ id: 'summary', label: 'Summary' },
			{ id: 'vms', label: 'VMs', count: 3 },
			{ id: 'tasks', label: 'Tasks', count: 1 },
			{ id: 'configuration', label: 'Configuration' }
		],
		hostedVms: [
			{ vm_id: 'vm-1101', name: 'db-primary-01', power_state: 'running', health: 'healthy', cpu: '4 vCPU', memory: '16 GB' },
			{ vm_id: 'vm-1102', name: 'db-replica-01', power_state: 'running', health: 'healthy', cpu: '4 vCPU', memory: '16 GB' },
			{ vm_id: 'vm-3301', name: 'bastion-01', power_state: 'stopped', health: 'healthy', cpu: '2 vCPU', memory: '4 GB' }
		],
		recentTasks: [
			{ task_id: 't-3001', status: 'succeeded', summary: 'Node health check passed', operation: 'health_check', started_unix_ms: Date.now() - 1000 * 60 * 60 }
		],
		configuration: [
			{ label: 'Node ID', value: 'n-ber-1-c01' },
			{ label: 'Cluster', value: 'eu-west-core' },
			{ label: 'Version', value: '1.4.2' },
			{ label: 'CPU', value: '64 cores / 256 threads' },
			{ label: 'Memory', value: '1 TB' },
			{ label: 'Storage backend', value: 'zfs-ssd' }
		]
	},
	'n-ber-1-c03': {
		state: 'ready',
		currentTab: 'summary',
		summary: {
			nodeId: 'n-ber-1-c03',
			name: 'ber-1-c03',
			cluster: 'eu-west-core',
			state: 'host_ready',
			health: 'degraded',
			version: '1.4.2',
			cpu: '45%',
			memory: '52%',
			storage: 'healthy',
			network: 'degraded',
			maintenance: false,
			scheduling: true
		},
		sections: [
			{ id: 'summary', label: 'Summary' },
			{ id: 'vms', label: 'VMs', count: 1 },
			{ id: 'tasks', label: 'Tasks', count: 0 },
			{ id: 'configuration', label: 'Configuration' }
		],
		hostedVms: [
			{ vm_id: 'vm-1103', name: 'cache-01', power_state: 'running', health: 'healthy', cpu: '2 vCPU', memory: '8 GB' }
		],
		recentTasks: [],
		configuration: [
			{ label: 'Node ID', value: 'n-ber-1-c03' },
			{ label: 'Cluster', value: 'eu-west-core' },
			{ label: 'Version', value: '1.4.2' },
			{ label: 'CPU', value: '64 cores / 256 threads' },
			{ label: 'Memory', value: '1 TB' },
			{ label: 'Storage backend', value: 'zfs-ssd' }
		]
	},
	'n-ams-1-n02': {
		state: 'ready',
		currentTab: 'summary',
		summary: {
			nodeId: 'n-ams-1-n02',
			name: 'ams-1-n02',
			cluster: 'eu-west-edge',
			state: 'draining',
			health: 'warning',
			version: '1.4.1',
			cpu: '34%',
			memory: '41%',
			storage: 'healthy',
			network: 'healthy',
			maintenance: true,
			scheduling: false
		},
		sections: [
			{ id: 'summary', label: 'Summary' },
			{ id: 'vms', label: 'VMs', count: 2 },
			{ id: 'tasks', label: 'Tasks', count: 3 },
			{ id: 'configuration', label: 'Configuration' }
		],
		hostedVms: [
			{ vm_id: 'vm-8842', name: 'prod-api-42', power_state: 'running', health: 'degraded', cpu: '2 vCPU', memory: '4 GB' },
			{ vm_id: 'vm-8843', name: 'prod-api-43', power_state: 'running', health: 'healthy', cpu: '2 vCPU', memory: '4 GB' }
		],
		recentTasks: [
			{ task_id: 't-2002', status: 'running', summary: 'Drain node ams-1-n02', operation: 'drain', started_unix_ms: Date.now() - 1000 * 60 * 18 },
			{ task_id: 't-2003', status: 'succeeded', summary: 'Pause scheduling on ams-1-n02', operation: 'pause_scheduling', started_unix_ms: Date.now() - 1000 * 60 * 25 }
		],
		configuration: [
			{ label: 'Node ID', value: 'n-ams-1-n02' },
			{ label: 'Cluster', value: 'eu-west-edge' },
			{ label: 'Version', value: '1.4.1' },
			{ label: 'CPU', value: '32 cores / 64 threads' },
			{ label: 'Memory', value: '512 GB' },
			{ label: 'Storage backend', value: 'nvme-local' }
		]
	},
	'n-sjc-1-n01': {
		state: 'ready',
		currentTab: 'summary',
		summary: {
			nodeId: 'n-sjc-1-n01',
			name: 'sjc-1-n01',
			cluster: 'us-west-dev',
			state: 'host_ready',
			health: 'degraded',
			version: '1.3.9',
			cpu: '91%',
			memory: '87%',
			storage: 'degraded',
			network: 'warning',
			maintenance: false,
			scheduling: true
		},
		sections: [
			{ id: 'summary', label: 'Summary' },
			{ id: 'vms', label: 'VMs', count: 1 },
			{ id: 'tasks', label: 'Tasks', count: 4 },
			{ id: 'configuration', label: 'Configuration' }
		],
		hostedVms: [
			{ vm_id: 'vm-9901', name: 'dev-test-01', power_state: 'running', health: 'healthy', cpu: '2 vCPU', memory: '8 GB' }
		],
		recentTasks: [
			{ task_id: 't-4001', status: 'failed', summary: 'Storage scrub', operation: 'scrub', started_unix_ms: Date.now() - 1000 * 60 * 120 }
		],
		configuration: [
			{ label: 'Node ID', value: 'n-sjc-1-n01' },
			{ label: 'Cluster', value: 'us-west-dev' },
			{ label: 'Version', value: '1.3.9' },
			{ label: 'CPU', value: '16 cores / 32 threads' },
			{ label: 'Memory', value: '128 GB' },
			{ label: 'Storage backend', value: 'zfs-ssd' }
		]
	}
};

function getNodeDetail(id: string, tab: string): NodeDetailModel {
	const base = mockNodes[id] ?? {
		state: 'ready',
		currentTab: tab,
		summary: {
			nodeId: id,
			name: id,
			cluster: 'unknown',
			state: 'unknown',
			health: 'unknown',
			version: 'unknown',
			cpu: '—',
			memory: '—',
			storage: 'unknown',
			network: 'unknown',
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
		configuration: [{ label: 'Node ID', value: id }]
	};
	return { ...base, currentTab: tab };
}

export const load: PageServerLoad = async ({ params, url }) => {
	const currentTab = url.searchParams.get('tab') ?? 'summary';
	const detail = getNodeDetail(params.id, currentTab);
	return { detail };
};
