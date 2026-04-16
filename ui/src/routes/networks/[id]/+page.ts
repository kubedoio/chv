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
	attached_vms: { vm_id: string; name: string; ip?: string }[];
	created_at: string;
	last_task: string;
	alerts: number;
	state: 'ready' | 'error';
};

export const load: PageLoad = async ({ params }) => {
	const token = getStoredToken() ?? undefined;

	try {
		const res = await getNetwork(params.id, token);
		const detail = res.detail as NetworkDetailModel;
		if (!detail) {
			error(404, 'Network not found');
		}
		return { detail };
	} catch (err) {
		// eslint-disable-next-line no-console
		console.error('BFF getNetwork error:', err);
		error(404, 'Network not found');
	}
};
