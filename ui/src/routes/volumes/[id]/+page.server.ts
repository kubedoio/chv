import type { Actions } from './$types';
import { handleVolumeMutation } from '$lib/webui/volume-server-actions';

export const actions: Actions = {
	default: async ({ request, cookies }) => {
		const token = cookies.get('chv_session') ?? undefined;
		const formData = await request.formData();
		return handleVolumeMutation(formData, token);
	}
};
