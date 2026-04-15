import type { PageServerLoad } from './$types';
import { buildVmsLoad, type VmsListModel } from '$lib/webui/vms-load';
import { handleVmMutation } from '$lib/webui/vm-server-actions';

export type { VmsListModel };

export const load: PageServerLoad = async ({ url, cookies }) => {
	const token = cookies.get('chv_session') ?? undefined;
	const vms = await buildVmsLoad({ searchParams: url.searchParams, token });
	return { vms };
};

export const actions = {
	default: async ({ request, cookies }) => {
		const token = cookies.get('chv_session') ?? undefined;
		const formData = await request.formData();
		return handleVmMutation(formData, token);
	}
};
