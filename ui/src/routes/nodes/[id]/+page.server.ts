import type { Actions } from './$types';
import { handleNodeMutation } from '$lib/webui/node-server-actions';

export const actions: Actions = {
	default: async ({ request, cookies }) => {
		const token = cookies.get('chv_session') ?? undefined;
		const formData = await request.formData();
		return handleNodeMutation(formData, token);
	}
};
