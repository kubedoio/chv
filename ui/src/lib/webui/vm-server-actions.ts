import { fail } from '@sveltejs/kit';
import { mutateVm, deleteVm } from '$lib/bff/vms';
import { BFFError } from '$lib/bff/client';
import type { MutateVmResponse } from '$lib/bff/types';

const VALID_VM_ACTIONS = ['start', 'stop', 'restart', 'delete', 'shutdown', 'poweroff'] as const;
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
		if (validAction === 'delete') {
			const result = await deleteVm({ vm_id, requested_by: 'webui' }, token);
			return {
				accepted: true,
				task_id: result.operation_id,
				vm_id: result.vm_id,
				summary: `Delete instance accepted`,
				action: validAction
			};
		}

		// Map shutdown/poweroff to the underlying stop action with force flag
		const apiAction = validAction === 'shutdown' || validAction === 'poweroff' ? 'stop' : validAction;
		const isForce = validAction === 'poweroff';

		const result = await mutateVm({ vm_id, action: apiAction, force: isForce }, token);
		return { ...result, action: validAction };
	} catch (err) {
		const message = err instanceof BFFError
			? err.message
			: err instanceof Error
				? err.message.includes('<html') || err.message.includes('Unexpected token')
					? 'Backend service unavailable. Check that the control plane is running.'
					: err.message
				: 'Mutation failed';
		const status = err instanceof BFFError && err.status >= 400 ? err.status : 500;
		return fail(status, { message });
	}
}
