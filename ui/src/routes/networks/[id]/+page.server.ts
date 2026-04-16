import type { PageServerLoad } from './$types';
import { error } from '@sveltejs/kit';

export type NetworkDetailModel = {
	network_id: string;
	name: string;
	scope: string;
	health: string;
	exposure: 'private' | 'nat' | 'public';
	policy: string;
	cidr: string;
	gateway: string;
	attached_vms: { vm_id: string; name: string; ip?: string }[];
	created_at: string;
	last_task: string;
	alerts: number;
	state: 'ready' | 'error';
};

const mockNetworks: Record<string, NetworkDetailModel> = {
	'net-1': {
		network_id: 'net-1',
		name: 'prod-backend',
		scope: 'cluster/eu-west-core',
		health: 'healthy',
		exposure: 'private',
		policy: 'deny-all-ingress',
		cidr: '10.0.1.0/24',
		gateway: '10.0.1.1',
		attached_vms: [
			{ vm_id: 'vm-1101', name: 'db-primary-01', ip: '10.0.1.10' },
			{ vm_id: 'vm-1102', name: 'db-replica-01', ip: '10.0.1.11' }
		],
		created_at: '2024-06-01T08:00:00Z',
		last_task: 'policy update',
		alerts: 0,
		state: 'ready'
	},
	'net-2': {
		network_id: 'net-2',
		name: 'edge-public',
		scope: 'cluster/eu-west-edge',
		health: 'degraded',
		exposure: 'public',
		policy: 'allow-443-80',
		cidr: '203.0.113.0/26',
		gateway: '203.0.113.1',
		attached_vms: [{ vm_id: 'vm-8842', name: 'prod-api-42', ip: '203.0.113.5' }],
		created_at: '2025-01-15T10:00:00Z',
		last_task: 'route sync',
		alerts: 2,
		state: 'ready'
	},
	'net-3': {
		network_id: 'net-3',
		name: 'internal-mgmt',
		scope: 'fleet',
		health: 'healthy',
		exposure: 'private',
		policy: 'restricted-ssh',
		cidr: '192.168.255.0/24',
		gateway: '192.168.255.1',
		attached_vms: [{ vm_id: 'vm-3301', name: 'bastion-01', ip: '192.168.255.10' }],
		created_at: '2023-11-20T09:00:00Z',
		last_task: 'subnet resize',
		alerts: 0,
		state: 'ready'
	},
	'net-4': {
		network_id: 'net-4',
		name: 'dmz-nat',
		scope: 'cluster/us-east-core',
		health: 'warning',
		exposure: 'nat',
		policy: 'port-forwarded',
		cidr: '10.64.0.0/22',
		gateway: '10.64.0.1',
		attached_vms: [{ vm_id: 'vm-4401', name: 'web-proxy-01', ip: '10.64.1.5' }],
		created_at: '2024-09-10T11:00:00Z',
		last_task: 'nat rule add',
		alerts: 1,
		state: 'ready'
	},
	'net-5': {
		network_id: 'net-5',
		name: 'dev-overlay',
		scope: 'cluster/us-west-dev',
		health: 'healthy',
		exposure: 'private',
		policy: 'open-internal',
		cidr: '172.20.0.0/16',
		gateway: '172.20.0.1',
		attached_vms: [],
		created_at: '2025-03-01T08:00:00Z',
		last_task: 'create network',
		alerts: 0,
		state: 'ready'
	}
};

export const load: PageServerLoad = async ({ params }) => {
	const network = mockNetworks[params.id];
	if (!network) {
		error(404, 'Network not found');
	}
	return { detail: network };
};
