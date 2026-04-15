import { fail } from '@sveltejs/kit';
import { mutateVm } from '$lib/bff/vms';
import { BFFError } from '$lib/bff/client';
import type { MutateVmResponse } from '$lib/bff/types';

const VALID_VM_ACTIONS = ['start', 'stop', 'restart'] as const;
type ValidVmAction = (typeof VALID_VM_ACTIONS)[number];

export async function handleVmMutation(
	formData: FormData,
	token: string | undefined
): Promise<MutateVmResponse & { action: ValidVmAction } | ReturnType<typeof fail>> {
	const vm_id = formData.get('vm_id')?.toString();
	const action = formData.get('action')?.toString();

	if (!vm_id || !action) {
		return fail(400, { message: 'Missing vm_id or action' });
	}

	if (!(VALID_VM_ACTIONS as readonly string[]).includes(action)) {
		return fail(400, { message: 'Invalid action' });
	}

	const validAction = action as ValidVmAction;

	try {
		const result = await mutateVm({ vm_id, action: validAction, force: false }, token);
		return { ...result, action: validAction };
	} catch (err) {
		const message = err instanceof BFFError ? err.message : err instanceof Error ? err.message : 'Mutation failed';
		return fail(500, { message });
	}
}
