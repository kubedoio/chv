import { fail } from '@sveltejs/kit';
import { mutateNode } from '$lib/bff/nodes';
import { BFFError } from '$lib/bff/client';
import type { MutateNodeResponse } from '$lib/bff/types';

const VALID_NODE_ACTIONS = [
	'pause_scheduling',
	'resume_scheduling',
	'drain',
	'enter_maintenance',
	'exit_maintenance'
] as const;
type ValidNodeAction = (typeof VALID_NODE_ACTIONS)[number];

export async function handleNodeMutation(
	formData: FormData,
	token: string | undefined
): Promise<MutateNodeResponse & { action: ValidNodeAction } | ReturnType<typeof fail>> {
	const node_id = formData.get('node_id')?.toString();
	const action = formData.get('action')?.toString();

	if (!node_id || !action) {
		return fail(400, { message: 'Missing node_id or action' });
	}

	if (!(VALID_NODE_ACTIONS as readonly string[]).includes(action)) {
		return fail(400, { message: 'Invalid action' });
	}

	const validAction = action as ValidNodeAction;

	try {
		const result = await mutateNode({ node_id, action: validAction }, token);
		return { ...result, action: validAction };
	} catch (err) {
		const message =
			err instanceof BFFError ? err.message : err instanceof Error ? err.message : 'Mutation failed';
		return fail(500, { message });
	}
}
