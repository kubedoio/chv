import { browser } from '$app/environment';
import { getStoredToken } from '$lib/api/client';

export interface TaskUpdate {
	task_id: string;
	status: string;
	summary: string;
	resource_kind: string;
	resource_id: string;
	event_unix_ms: number;
}

export class TaskStreamStore {
	private es: EventSource | null = null;
	private seen = new Set<string>();
	private pollTimer: ReturnType<typeof setInterval> | null = null;
	status = $state<'idle' | 'connecting' | 'open' | 'error'>('idle');
	onTaskCompleted: ((task: TaskUpdate) => void) | null = null;

	startPollingFallback(intervalMs = 10_000) {
		this.stopPollingFallback();
		this.pollTimer = setInterval(async () => {
			try {
				const token = getStoredToken();
				if (!token) return;
				const res = await fetch('/v1/tasks', {
					method: 'POST',
					headers: {
						'Content-Type': 'application/json',
						Authorization: `Bearer ${token}`,
					},
					body: JSON.stringify({ page: 1, page_size: 50 }),
				});
				if (!res.ok) return;
				const data = await res.json();
				for (const item of data.items || []) {
					if (!this.seen.has(item.task_id)) {
						this.seen.add(item.task_id);
						if (['Completed', 'Failed', 'Cancelled'].includes(item.status)) {
							this.onTaskCompleted?.(item);
						}
					}
				}
			} catch {
				// Silently ignore polling errors
			}
		}, intervalMs);
	}

	stopPollingFallback() {
		if (this.pollTimer) {
			clearInterval(this.pollTimer);
			this.pollTimer = null;
		}
	}

	connect(resourceKinds?: string[]) {
		if (!browser) return;
		this.disconnect();
		if (typeof EventSource === 'undefined') {
			this.startPollingFallback();
			return;
		}

		const token = getStoredToken() ?? '';
		const url = new URL('/v1/tasks/stream', window.location.origin);
		if (resourceKinds && resourceKinds.length > 0) {
			url.searchParams.set('resource_kinds', resourceKinds.join(','));
		}

		this.es = new EventSource(url.toString(), {
			headers: token ? { Authorization: `Bearer ${token}` } : undefined,
		} as EventSourceInit);
		this.status = 'connecting';

		this.es.onopen = () => {
			this.status = 'open';
		};

		this.es.onerror = () => {
			this.status = 'error';
		};

		this.es.onmessage = (event) => {
			try {
				const payload = JSON.parse(event.data);
				const items: TaskUpdate[] = payload.items ?? [];
				for (const task of items) {
					if (task.status !== 'Completed') continue;
					if (this.seen.has(task.task_id)) continue;
					this.seen.add(task.task_id);
					this.onTaskCompleted?.(task);
				}
			} catch {
				// Ignore malformed messages
			}
		};
	}

	disconnect() {
		this.stopPollingFallback();
		if (this.es) {
			this.es.close();
			this.es = null;
		}
		this.status = 'idle';
	}
}

export const taskStream = new TaskStreamStore();
