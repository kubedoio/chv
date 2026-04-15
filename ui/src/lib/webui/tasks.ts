import type { Event, Operation } from '$lib/api/types';
import type { ShellTone } from '$lib/shell/app-shell';

export type TaskStatusKey =
	| 'queued'
	| 'running'
	| 'succeeded'
	| 'failed'
	| 'cancelled'
	| 'unknown';
export type TaskWindowKey = 'active' | '24h' | '7d' | '30d' | 'all';
export type TaskPageState = 'ready' | 'empty' | 'error';

export interface TaskStatusMeta {
	key: TaskStatusKey;
	label: string;
	detail: string;
	tone: ShellTone;
}

export interface TaskFilters {
	status?: string;
	resourceKind?: string;
	query?: string;
	window?: string;
}

export interface TaskTimelineItemModel {
	taskId: string;
	status: TaskStatusKey;
	label: string;
	detail: string;
	tone: ShellTone;
	operation: string;
	summary: string;
	resourceKind: string;
	resourceId: string;
	resourceLabel: string;
	actor: string;
	startedAt: string;
	startedAtLabel: string;
	finishedAt?: string;
	finishedAtLabel?: string;
	durationLabel: string;
	timelineTitle: string;
	timelineDetail: string;
	failureReason?: string;
	isActive: boolean;
	isTerminal: boolean;
}

export interface TaskFilterMeta {
	statuses: TaskStatusKey[];
	resourceKinds: string[];
	windows: TaskWindowKey[];
}

export interface TaskListModel {
	items: TaskTimelineItemModel[];
	state: TaskPageState;
	page: {
		page: number;
		pageSize: number;
		totalItems: number;
	};
	filters: {
		current: Record<string, string>;
		applied: Record<string, string>;
		options: TaskFilterMeta;
	};
}

export interface TaskSnapshot {
	operations: Operation[];
	events: Event[];
}

interface BuildTaskListOptions {
	page?: number;
	pageSize?: number;
	now?: Date;
	fetchFailed?: boolean;
	primaryDataUnavailable?: boolean;
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
	unknown: {
		key: 'unknown',
		label: 'Unknown',
		detail: 'Status could not be determined',
		tone: 'unknown'
	}
};

const ACTIVE_WINDOWS: TaskWindowKey[] = ['active', '24h', '7d', '30d', 'all'];

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
		default:
			return 'unknown';
	}
}

export function getTaskStatusMeta(state: string | undefined): TaskStatusMeta {
	return TASK_STATUS_META[normalizeTaskStatus(state)];
}

export function buildTaskList(
	snapshot: TaskSnapshot,
	filters: TaskFilters = {},
	options: BuildTaskListOptions = {}
): TaskListModel {
	const now = options.now ?? new Date();
	const page = Math.max(options.page ?? 1, 1);
	const pageSize = Math.max(options.pageSize ?? 50, 1);
	const current = getCurrentFilters(filters);
	const allItems = snapshot.operations
		.map((operation) => mapOperationToTask(operation, snapshot.events, now))
		.sort((left, right) => Date.parse(right.startedAt) - Date.parse(left.startedAt));
	const filteredItems = allItems.filter((item) => matchesTaskFilters(item, filters, now));
	const pageStart = (page - 1) * pageSize;
	const pagedItems = filteredItems.slice(pageStart, pageStart + pageSize);
	const applied = getAppliedFilters(filters);
	const state: TaskPageState =
		pagedItems.length > 0
			? 'ready'
			: (options.fetchFailed || options.primaryDataUnavailable) && allItems.length === 0
				? 'error'
				: 'empty';

	return {
		items: pagedItems,
		state,
		page: {
			page,
			pageSize,
			totalItems: filteredItems.length
		},
		filters: {
			current,
			applied,
			options: {
				statuses: Object.keys(TASK_STATUS_META) as TaskStatusKey[],
				resourceKinds: Array.from(
					new Set(allItems.map((item) => item.resourceKind).filter(Boolean))
				).sort(),
				windows: ACTIVE_WINDOWS
			}
		}
	};
}

function getCurrentFilters(filters: TaskFilters): Record<string, string> {
	return {
		status: filters.status?.trim() || 'all',
		resourceKind: filters.resourceKind?.trim() || 'all',
		query: filters.query?.trim() || '',
		window: filters.window?.trim() || '7d'
	};
}

function mapOperationToTask(
	operation: Operation,
	events: Event[],
	now: Date
): TaskTimelineItemModel {
	const relatedEvents = findRelatedEvents(operation, events);
	const latestEvent = relatedEvents[0];
	const inferredState = operation.state === 'unknown' ? latestEvent?.status : operation.state;
	const statusMeta = getTaskStatusMeta(inferredState);
	const startedAt = operation.created_at;
	const finishedAt = getFinishedTimestamp(statusMeta.key, latestEvent);
	const operationLabel = titleize(operation.operation_type);
	const resourceKind = normalizeResourceKind(operation.resource_type);
	const failureReason =
		statusMeta.key === 'failed'
			? latestEvent?.message ?? latestEvent?.details?.reason ?? `Last ${operation.operation_type} attempt failed`
			: undefined;

	return {
		taskId: operation.id,
		status: statusMeta.key,
		label: statusMeta.label,
		detail: statusMeta.detail,
		tone: statusMeta.tone,
		operation: operationLabel,
		summary: `${operationLabel} ${resourceKind} ${operation.resource_id}`,
		resourceKind,
		resourceId: operation.resource_id,
		resourceLabel: `${resourceKind} ${operation.resource_id}`,
		actor: 'Control plane',
		startedAt,
		startedAtLabel: formatDateTime(startedAt),
		finishedAt,
		finishedAtLabel: finishedAt ? formatDateTime(finishedAt) : undefined,
		durationLabel: formatDuration(startedAt, finishedAt, now),
		timelineTitle: `${statusMeta.label}: ${operationLabel}`,
		timelineDetail: `${resourceKind} ${operation.resource_id}`,
		failureReason,
		isActive: statusMeta.key === 'queued' || statusMeta.key === 'running',
		isTerminal:
			statusMeta.key === 'succeeded' ||
			statusMeta.key === 'failed' ||
			statusMeta.key === 'cancelled'
	};
}

function matchesTaskFilters(
	item: TaskTimelineItemModel,
	filters: TaskFilters,
	now: Date
): boolean {
	if (filters.status && filters.status !== 'all' && item.status !== normalizeTaskStatus(filters.status)) {
		return false;
	}

	if (
		filters.resourceKind &&
		filters.resourceKind !== 'all' &&
		item.resourceKind !== filters.resourceKind.toLowerCase()
	) {
		return false;
	}

	if (filters.window && filters.window !== 'all') {
		const startedAtMs = Date.parse(item.startedAt);

		if (filters.window === 'active' && !item.isActive) {
			return false;
		}

		if (filters.window === '24h' && startedAtMs < now.getTime() - 24 * 60 * 60 * 1000) {
			return false;
		}

		if (filters.window === '7d' && startedAtMs < now.getTime() - 7 * 24 * 60 * 60 * 1000) {
			return false;
		}

		if (filters.window === '30d' && startedAtMs < now.getTime() - 30 * 24 * 60 * 60 * 1000) {
			return false;
		}
	}

	if (filters.query) {
		const query = filters.query.trim().toLowerCase();
		const haystack = [
			item.summary,
			item.operation,
			item.resourceKind,
			item.resourceId,
			item.failureReason,
			item.label
		]
			.filter(Boolean)
			.join(' ')
			.toLowerCase();

		if (!haystack.includes(query)) {
			return false;
		}
	}

	return true;
}

function getAppliedFilters(filters: TaskFilters): Record<string, string> {
	const applied: Record<string, string> = {};

	if (filters.status && filters.status !== 'all') {
		applied.status = normalizeTaskStatus(filters.status);
	}

	if (filters.resourceKind && filters.resourceKind !== 'all') {
		applied.resourceKind = filters.resourceKind.toLowerCase();
	}

	if (filters.query?.trim()) {
		applied.query = filters.query.trim();
	}

	if (filters.window && filters.window !== 'all') {
		applied.window = filters.window;
	}

	return applied;
}

function findRelatedEvents(operation: Operation, events: Event[]): Event[] {
	return events
		.filter(
			(event) =>
				event.resource.toLowerCase() === normalizeResourceKind(operation.resource_type) &&
				event.resource_id === operation.resource_id &&
				event.operation.toLowerCase() === operation.operation_type.toLowerCase()
		)
		.sort((left, right) => Date.parse(right.timestamp) - Date.parse(left.timestamp));
}

function getFinishedTimestamp(status: TaskStatusKey, latestEvent: Event | undefined): string | undefined {
	if (status === 'queued' || status === 'running') {
		return undefined;
	}

	return latestEvent?.timestamp;
}

function normalizeResourceKind(resourceKind: string): string {
	const normalized = resourceKind.trim().toLowerCase();

	switch (normalized) {
		case 'storage':
		case 'storagepool':
		case 'storage_pool':
		case 'storage-pool':
			return 'volume';
		default:
			return normalized;
	}
}

function titleize(value: string): string {
	return value
		.replace(/[_-]+/g, ' ')
		.replace(/\s+/g, ' ')
		.trim()
		.replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function formatDateTime(value: string): string {
	const date = new Date(value);

	return new Intl.DateTimeFormat('en-US', {
		month: 'short',
		day: 'numeric',
		hour: 'numeric',
		minute: '2-digit'
	}).format(date);
}

function formatDuration(startedAt: string, finishedAt: string | undefined, now: Date): string {
	const start = Date.parse(startedAt);
	const end = finishedAt ? Date.parse(finishedAt) : now.getTime();
	const elapsedSeconds = Math.max(Math.round((end - start) / 1000), 0);

	if (elapsedSeconds < 60) {
		return `${elapsedSeconds}s`;
	}

	if (elapsedSeconds < 3600) {
		return `${Math.round(elapsedSeconds / 60)}m`;
	}

	if (elapsedSeconds < 86400) {
		return `${Math.round(elapsedSeconds / 3600)}h`;
	}

	return `${Math.round(elapsedSeconds / 86400)}d`;
}
