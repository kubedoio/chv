import type { PageLoad } from './$types';
import { getStoredToken } from '$lib/api/client';
import { error } from '@sveltejs/kit';
import { getNetwork } from '$lib/bff/networks';

export type NetworkDetailModel = {
	network_id: string;
	name: string;
	scope: string;
	health: string;
	exposure: 'private' | 'nat' | 'public';
	policy: string;
	cidr: string;
	gateway: string;
	dhcp_enabled: boolean;
	ipam_mode: string;
	is_default: boolean;
	attached_vms: Array<{
		vm_id: string;
		display_name: string;
		runtime_status: string;
		ip_address?: string;
		mac_address?: string;
	}>;
	created_at: string;
	last_task: string;
	alerts: number;
	state: 'ready' | 'error';
	title?: string;
	description?: string;
};

export const load: PageLoad = async ({ params }) => {
	const token = getStoredToken() ?? undefined;

	try {
		const res = await getNetwork(params.id, token);
		const detail = res.detail as NetworkDetailModel | null;
		if (!detail) {
			error(404, 'Network not found');
		}
		return { detail };
	} catch (err) {
		// eslint-disable-next-line no-console
		console.error('BFF getNetwork error:', err);
		const detail: NetworkDetailModel = {
			network_id: params.id,
			name: '',
			scope: '',
			health: '',
			exposure: 'private',
			policy: '',
			cidr: '',
			gateway: '',
			dhcp_enabled: false,
			ipam_mode: '',
			is_default: false,
			attached_vms: [],
			created_at: '',
			last_task: '',
			alerts: 0,
			state: 'error',
			title: 'Failed to load network',
			description: err instanceof Error ? err.message : 'Unknown error'
		};
		return { detail };
	}
};
