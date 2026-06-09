import { browser } from '$app/environment';

function getStoredToken(): string | null {
	if (typeof localStorage === 'undefined') return null;
	return localStorage.getItem('chv-api-token');
}

export interface TaskUpdate {
	task_id: string;
	status: string;
	summary: string;
	resource_kind: string;
	resource_id: string;
	event_unix_ms: number;
}

export class TaskStreamStore {
	private abortCtrl: AbortController | null = null;
	private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
	private seen = new Set<string>();
	private pollTimer: ReturnType<typeof setInterval> | null = null;
	status = $state<'idle' | 'connecting' | 'open' | 'error'>('idle');
	onTaskCompleted: ((task: TaskUpdate) => void) | null = null;

	private reconnectDelay = 1000;
	private readonly maxReconnectDelay = 30000;

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
				const terminal = ['Completed', 'Failed', 'Cancelled'];
				for (const item of data.items || []) {
					if (!terminal.includes(item.status)) continue;
					if (this.seen.has(item.task_id)) continue;
					this.seen.add(item.task_id);
					this.onTaskCompleted?.(item);
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

	private scheduleReconnect(resourceKinds?: string[]) {
		if (this.reconnectTimer) return;
		this.reconnectTimer = setTimeout(() => {
			this.reconnectTimer = null;
			this.connect(resourceKinds);
		}, this.reconnectDelay);
		this.reconnectDelay = Math.min(this.reconnectDelay * 2, this.maxReconnectDelay);
	}

	async connect(resourceKinds?: string[]) {
		if (!browser) return;
		this.disconnect();
		const token = getStoredToken();
		if (!token) {
			this.startPollingFallback();
			return;
		}

		const url = new URL('/v1/tasks/stream', window.location.origin);
		if (resourceKinds && resourceKinds.length > 0) {
			url.searchParams.set('resource_kinds', resourceKinds.join(','));
		}

		this.abortCtrl = new AbortController();
		this.status = 'connecting';

		try {
			const res = await fetch(url.toString(), {
				headers: {
					Accept: 'text/event-stream',
					Authorization: `Bearer ${token}`,
				},
				signal: this.abortCtrl.signal,
			});

			if (!res.ok) {
				if (res.status === 401) {
					this.status = 'error';
					// Token invalid/expired — don't retry, let user re-login
					return;
				}
				throw new Error(`HTTP ${res.status}`);
			}

			if (!res.body) {
				throw new Error('No response body');
			}

			this.status = 'open';
			this.reconnectDelay = 1000; // Reset on successful connection

			const reader = res.body.getReader();
			const decoder = new TextDecoder();
			let buffer = '';

			while (true) {
				const { done, value } = await reader.read();
				if (done) break;

				buffer += decoder.decode(value, { stream: true });
				const lines = buffer.split('\n');
				buffer = lines.pop() ?? '';

				let dataLine = '';
				for (const line of lines) {
					if (line.startsWith('data:')) {
						dataLine = line.slice(5).trim();
					} else if (line === '' && dataLine) {
						try {
							const payload = JSON.parse(dataLine);
							const items: TaskUpdate[] = payload.items ?? [];
							const terminal = ['Completed', 'Failed', 'Cancelled'];
							for (const task of items) {
								if (!terminal.includes(task.status)) continue;
								if (this.seen.has(task.task_id)) continue;
								this.seen.add(task.task_id);
								this.onTaskCompleted?.(task);
							}
						} catch {
							// Ignore malformed messages
						}
						dataLine = '';
					}
				}
			}

			// Stream ended normally — reconnect
			this.status = 'idle';
			this.scheduleReconnect(resourceKinds);
		} catch (err) {
			if ((err as Error).name === 'AbortError') {
				this.status = 'idle';
				return;
			}
			this.status = 'error';
			this.scheduleReconnect(resourceKinds);
		}
	}

	disconnect() {
		this.stopPollingFallback();
		if (this.reconnectTimer) {
			clearTimeout(this.reconnectTimer);
			this.reconnectTimer = null;
		}
		if (this.abortCtrl) {
			this.abortCtrl.abort();
			this.abortCtrl = null;
		}
		this.status = 'idle';
	}
}

export const taskStream = new TaskStreamStore();
