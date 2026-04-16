import type { PageServerLoad } from './$types';

export type NetworkListItem = {
	network_id: string;
	name: string;
	scope: string;
	health: string;
	attached_vms: number;
	exposure: 'private' | 'nat' | 'public';
	policy: string;
	last_task: string;
	alerts: number;
};

export type NetworksListModel = {
	items: NetworkListItem[];
	state: 'ready' | 'empty' | 'error';
	filters: { current: Record<string, string>; applied: Record<string, string> };
	page: { page: number; pageSize: number; totalItems: number };
};

const mockNetworks: NetworkListItem[] = [
	{
		network_id: 'net-1',
		name: 'prod-backend',
		scope: 'cluster/eu-west-core',
		health: 'healthy',
		attached_vms: 124,
		exposure: 'private',
		policy: 'deny-all-ingress',
		last_task: 'policy update',
		alerts: 0
	},
	{
		network_id: 'net-2',
		name: 'edge-public',
		scope: 'cluster/eu-west-edge',
		health: 'degraded',
		attached_vms: 18,
		exposure: 'public',
		policy: 'allow-443-80',
		last_task: 'route sync',
		alerts: 2
	},
	{
		network_id: 'net-3',
		name: 'internal-mgmt',
		scope: 'fleet',
		health: 'healthy',
		attached_vms: 56,
		exposure: 'private',
		policy: 'restricted-ssh',
		last_task: 'subnet resize',
		alerts: 0
	},
	{
		network_id: 'net-4',
		name: 'dmz-nat',
		scope: 'cluster/us-east-core',
		health: 'warning',
		attached_vms: 42,
		exposure: 'nat',
		policy: 'port-forwarded',
		last_task: 'nat rule add',
		alerts: 1
	},
	{
		network_id: 'net-5',
		name: 'dev-overlay',
		scope: 'cluster/us-west-dev',
		health: 'healthy',
		attached_vms: 8,
		exposure: 'private',
		policy: 'open-internal',
		last_task: 'create network',
		alerts: 0
	}
];

function filterNetworks(items: NetworkListItem[], current: Record<string, string>): NetworkListItem[] {
	let result = [...items];
	const query = (current.query ?? '').toLowerCase();
	if (query) {
		result = result.filter((n) => n.name.toLowerCase().includes(query) || n.scope.toLowerCase().includes(query));
	}
	const health = current.health;
	if (health && health !== 'all') {
		result = result.filter((n) => n.health.toLowerCase() === health.toLowerCase());
	}
	const exposure = current.exposure;
	if (exposure && exposure !== 'all') {
		result = result.filter((n) => n.exposure === exposure);
	}
	return result;
}

export const load: PageServerLoad = async ({ url }) => {
	const page = Math.max(1, parseInt(url.searchParams.get('page') ?? '1', 10) || 1);
	const pageSize = 50;

	const current: Record<string, string> = {};
	const query = url.searchParams.get('query');
	const health = url.searchParams.get('health');
	const exposure = url.searchParams.get('exposure');

	if (query) current.query = query;
	if (health) current.health = health;
	if (exposure) current.exposure = exposure;

	const filtered = filterNetworks(mockNetworks, current);

	const model: NetworksListModel = {
		items: filtered,
		state: filtered.length === 0 ? 'empty' : 'ready',
		filters: { current, applied: current },
		page: { page, pageSize, totalItems: filtered.length }
	};

	return { networks: model };
};
