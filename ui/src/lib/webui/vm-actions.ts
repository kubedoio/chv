import type { Operation } from '$lib/api/types';
import type { ShellTone } from '$lib/shell/app-shell';
import { getTaskStatusMeta } from '$lib/webui/tasks';

export type VmLifecycleAction = 'start' | 'stop' | 'restart';

export interface VmLifecycleActionResult {
	accepted: boolean;
	action: VmLifecycleAction;
	summary: string;
	taskId: string | null;
	taskLabel: string;
	taskTone: ShellTone;
	taskHref: string | null;
}

interface RunVmLifecycleActionInput {
	vmId: string;
	vmName: string;
	action: VmLifecycleAction;
	perform: () => Promise<unknown>;
	listOperations: () => Promise<Operation[]>;
	now?: Date;
}

export async function runVmLifecycleAction(
	input: RunVmLifecycleActionInput
): Promise<VmLifecycleActionResult> {
	const startedAt = input.now ?? new Date();
	await input.perform();

	const operations = await input.listOperations().catch(() => []);
	const matchingOperation =
		operations
			.filter(
				(operation) =>
					operation.resource_type.toLowerCase() === 'vm' &&
					operation.resource_id === input.vmId &&
					operation.operation_type.toLowerCase() === input.action &&
					Date.parse(operation.created_at) >= startedAt.getTime() - 1000
			)
			.sort((left, right) => Date.parse(right.created_at) - Date.parse(left.created_at))[0] ?? null;

	const taskMeta = getTaskStatusMeta(matchingOperation?.state ?? 'queued');
	const actionLabel = toTitle(input.action);

	return {
		accepted: true,
		action: input.action,
		summary: matchingOperation
			? `${actionLabel} accepted for ${input.vmName}. Track ${matchingOperation.id} for progress.`
			: `${actionLabel} accepted for ${input.vmName}. The task record is still being indexed.`,
		taskId: matchingOperation?.id ?? null,
		taskLabel: taskMeta.label,
		taskTone: taskMeta.tone,
		taskHref: matchingOperation ? `/tasks?query=${matchingOperation.id}` : null
	};
}

function toTitle(value: string): string {
	return value.charAt(0).toUpperCase() + value.slice(1);
}
