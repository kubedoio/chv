import type { RecentTask, RelatedTask } from '$lib/bff/types';
import type { TaskTimelineItemModel } from './tasks';
import { normalizeTaskStatus, getTaskStatusMeta } from './tasks';
import { formatDateTimeLabel, formatDurationLabel, titleize } from './formatters';

export function normalizeResourceKind(kind: string): string {
	const s = kind.trim().toLowerCase();
	if (['storage', 'storagepool', 'storage_pool', 'storage-pool'].includes(s)) return 'volume';
	return s;
}

export function mapRecentTask(task: RecentTask, now = Date.now()): TaskTimelineItemModel {
	const status = normalizeTaskStatus(task.status);
	const meta = getTaskStatusMeta(status);
	const resourceKind = normalizeResourceKind(task.resource_kind);
	const operation = titleize(task.operation || resourceKind);
	const startedAt = new Date(task.started_unix_ms).toISOString();
	return {
		taskId: task.task_id,
		status,
		label: meta.label,
		detail: meta.detail,
		tone: meta.tone,
		operation,
		summary: task.summary,
		resourceKind,
		resourceId: task.resource_id,
		resourceLabel: `${resourceKind} ${task.resource_id}`,
		actor: 'Control plane',
		startedAt,
		startedAtLabel: formatDateTimeLabel(task.started_unix_ms),
		finishedAt: undefined,
		finishedAtLabel: undefined,
		durationLabel: formatDurationLabel(task.started_unix_ms, now),
		timelineTitle: `${meta.label}: ${operation}`,
		timelineDetail: `${resourceKind} ${task.resource_id}`,
		failureReason: status === 'failed' ? task.summary : undefined,
		isActive: status === 'queued' || status === 'running',
		isTerminal: status === 'succeeded' || status === 'failed' || status === 'cancelled'
	};
}

export function mapRelatedTask(
	task: RelatedTask,
	resourceId: string,
	resourceKind: string,
	now = Date.now()
): TaskTimelineItemModel {
	const status = normalizeTaskStatus(task.status);
	const meta = getTaskStatusMeta(status);
	const operation = titleize(task.operation || resourceKind);
	const startedAt = new Date(task.started_unix_ms).toISOString();
	return {
		taskId: task.task_id,
		status,
		label: meta.label,
		detail: meta.detail,
		tone: meta.tone,
		operation,
		summary: task.summary,
		resourceKind,
		resourceId,
		resourceLabel: `${resourceKind} ${resourceId}`,
		actor: 'Control plane',
		startedAt,
		startedAtLabel: formatDateTimeLabel(task.started_unix_ms),
		finishedAt: undefined,
		finishedAtLabel: undefined,
		durationLabel: formatDurationLabel(task.started_unix_ms, now),
		timelineTitle: `${meta.label}: ${operation}`,
		timelineDetail: `${resourceKind} ${resourceId}`,
		failureReason: status === 'failed' ? task.summary : undefined,
		isActive: status === 'queued' || status === 'running',
		isTerminal: status === 'succeeded' || status === 'failed' || status === 'cancelled'
	};
}
