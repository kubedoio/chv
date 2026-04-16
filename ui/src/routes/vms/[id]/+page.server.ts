import type { Actions } from './$types';
import { handleVmMutation } from '$lib/webui/vm-server-actions';

export const actions: Actions = {
	default: async ({ request, cookies }) => {
		const token = cookies.get('chv_session') ?? undefined;
		const formData = await request.formData();
		return handleVmMutation(formData, token);
	}
};
