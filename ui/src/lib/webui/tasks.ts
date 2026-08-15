import type { ShellTone } from '$lib/shell/app-shell';

export type TaskStatusKey =
	| 'queued'
	| 'running'
	| 'succeeded'
	| 'failed'
	| 'cancelled'
	| 'awaiting-operator-input'
	| 'unknown';

export interface TaskStatusMeta {
	key: TaskStatusKey;
	label: string;
	detail: string;
	tone: ShellTone;
}

const TASK_STATUS_META: Record<TaskStatusKey, TaskStatusMeta> = {
	queued: {
		key: 'queued',
		label: 'Accepted',
		detail: 'Queued for execution',
		tone: 'warning'
	},
	running: {
		key: 'running',
		label: 'In progress',
		detail: 'Actively applying changes',
		tone: 'degraded'
	},
	succeeded: {
		key: 'succeeded',
		label: 'Completed',
		detail: 'Finished successfully',
		tone: 'healthy'
	},
	failed: {
		key: 'failed',
		label: 'Failed',
		detail: 'Needs operator attention',
		tone: 'failed'
	},
	cancelled: {
		key: 'cancelled',
		label: 'Cancelled',
		detail: 'Stopped before completion',
		tone: 'unknown'
	},
	'awaiting-operator-input': {
		key: 'awaiting-operator-input',
		label: 'Awaiting Input',
		detail: 'Paused for operator decision',
		tone: 'warning'
	},
	unknown: {
		key: 'unknown',
		label: 'Unknown',
		detail: 'Status could not be determined',
		tone: 'unknown'
	}
};

export function normalizeTaskStatus(state: string | undefined): TaskStatusKey {
	const normalized = state?.trim().toLowerCase();

	switch (normalized) {
		case 'queued':
		case 'accepted':
		case 'pending':
			return 'queued';
		case 'running':
		case 'in_progress':
		case 'in-progress':
			return 'running';
		case 'succeeded':
		case 'success':
		case 'completed':
			return 'succeeded';
		case 'failed':
		case 'error':
			return 'failed';
		case 'cancelled':
		case 'canceled':
			return 'cancelled';
		case 'awaitingoperatorinput':
		case 'awaiting-operator-input':
			return 'awaiting-operator-input';
		default:
			return 'unknown';
	}
}

export function getTaskStatusMeta(state: string | undefined): TaskStatusMeta {
	return TASK_STATUS_META[normalizeTaskStatus(state)];
}
