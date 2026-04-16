import { fail } from '@sveltejs/kit';
import { mutateVolume } from '$lib/bff/volumes';
import { BFFError } from '$lib/bff/client';
import type { MutateVolumeResponse } from '$lib/bff/types';

const VALID_VOLUME_ACTIONS = ['attach', 'detach', 'resize'] as const;
type ValidVolumeAction = (typeof VALID_VOLUME_ACTIONS)[number];

export async function handleVolumeMutation(
	formData: FormData,
	token: string | undefined
): Promise<MutateVolumeResponse & { action: ValidVolumeAction } | ReturnType<typeof fail>> {
	const volume_id = formData.get('volume_id')?.toString();
	const action = formData.get('action')?.toString();

	if (!volume_id || !action) {
		return fail(400, { message: 'Missing volume_id or action' });
	}

	if (!(VALID_VOLUME_ACTIONS as readonly string[]).includes(action)) {
		return fail(400, { message: 'Invalid action' });
	}

	const validAction = action as ValidVolumeAction;
	const force = formData.get('force')?.toString() === 'true';
	const resizeBytes = formData.get('resize_bytes')?.toString();

	try {
		const result = await mutateVolume({
			volume_id,
			action: validAction,
			force,
			resize_bytes: resizeBytes ? parseInt(resizeBytes, 10) : undefined
		}, token);
		return { ...result, action: validAction };
	} catch (err) {
		const message = err instanceof BFFError ? err.message : err instanceof Error ? err.message : 'Mutation failed';
		return fail(500, { message });
	}
}
