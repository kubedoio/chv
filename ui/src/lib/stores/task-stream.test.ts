import { beforeEach, describe, expect, it, vi } from 'vitest';

// task-stream now imports getStoredToken from $lib/api/client, which transitively
// pulls in SvelteKit's $env/dynamic/public and $app/navigation. Mock both so this
// suite can run under jsdom without a SvelteKit runtime.
vi.mock('$env/dynamic/public', () => ({
	env: {}
}));

vi.mock('$app/navigation', () => ({
	goto: vi.fn(),
	invalidateAll: vi.fn()
}));

import { TaskStreamStore, type TaskUpdate } from './task-stream.svelte';

function makeMockReader(chunks: Uint8Array[], hang = false) {
	let idx = 0;
	return {
		read: vi.fn(() => {
			if (hang) {
				return new Promise(() => {}); // Never resolves
			}
			if (idx < chunks.length) {
				return Promise.resolve({ done: false, value: chunks[idx++] });
			}
			return Promise.resolve({ done: true });
		}),
		releaseLock: vi.fn(),
	};
}

function mockFetchWithStream(chunks: Uint8Array[], status = 200, hang = false) {
	const reader = makeMockReader(chunks, hang);
	globalThis.fetch = vi.fn(() =>
		Promise.resolve({
			ok: status >= 200 && status < 300,
			status,
			body: { getReader: () => reader },
		} as unknown as Response)
	);
	return reader;
}

describe('TaskStreamStore', () => {
	beforeEach(() => {
		vi.restoreAllMocks();
		vi.stubGlobal('localStorage', {
			getItem: vi.fn(() => 'test-token'),
			setItem: vi.fn(),
			removeItem: vi.fn(),
		});
	});

	it('connects to SSE endpoint with resource_kinds filter', async () => {
		const reader = mockFetchWithStream([], 200, true);
		const store = new TaskStreamStore();
		store.connect(['vm', 'node']);

		expect(globalThis.fetch).toHaveBeenCalledWith(
			expect.stringContaining('/v1/tasks/stream?resource_kinds=vm%2Cnode'),
			expect.objectContaining({
				headers: expect.objectContaining({
					Authorization: 'Bearer test-token',
				}),
				signal: expect.any(AbortSignal),
			})
		);
		expect(store.status).toBe('connecting');

		// Wait for async connect
		await new Promise((r) => setTimeout(r, 10));
		expect(store.status).toBe('open');
		store.disconnect();
	});

	it('falls back to polling when no token is available', () => {
		vi.stubGlobal('localStorage', {
			getItem: vi.fn(() => null),
			setItem: vi.fn(),
			removeItem: vi.fn(),
		});

		const store = new TaskStreamStore();
		const pollSpy = vi.spyOn(store, 'startPollingFallback');
		store.connect();

		expect(pollSpy).toHaveBeenCalled();
		store.disconnect();
	});

	it('handles task completion and calls handler', async () => {
		const task: TaskUpdate = {
			task_id: 'op-1',
			status: 'Completed',
			summary: 'CreateVm',
			resource_kind: 'vm',
			resource_id: 'vm-123',
			event_unix_ms: Date.now(),
		};
		const payload = JSON.stringify({ items: [task] });
		const encoder = new TextEncoder();
		const reader = mockFetchWithStream([encoder.encode(`data: ${payload}\n\n`)]);

		const handler = vi.fn();
		const store = new TaskStreamStore();
		store.onTaskCompleted = handler;
		store.connect();

		// Wait for async read loop
		await new Promise((r) => setTimeout(r, 50));

		expect(handler).toHaveBeenCalledWith(task);
		store.disconnect();
	});

	it('deduplicates by task_id', async () => {
		const task: TaskUpdate = {
			task_id: 'op-1',
			status: 'Completed',
			summary: 'CreateVm',
			resource_kind: 'vm',
			resource_id: 'vm-123',
			event_unix_ms: Date.now(),
		};
		const payload = JSON.stringify({ items: [task] });
		const encoder = new TextEncoder();
		const reader = mockFetchWithStream([
			encoder.encode(`data: ${payload}\n\n`),
			encoder.encode(`data: ${payload}\n\n`),
		]);

		const handler = vi.fn();
		const store = new TaskStreamStore();
		store.onTaskCompleted = handler;
		store.connect();

		await new Promise((r) => setTimeout(r, 50));

		expect(handler).toHaveBeenCalledTimes(1);
		store.disconnect();
	});

	it('sets error status on 401 and does not reconnect', async () => {
		mockFetchWithStream([], 401);
		const store = new TaskStreamStore();
		const reconnectSpy = vi.spyOn(store as any, 'scheduleReconnect');
		store.connect();

		await new Promise((r) => setTimeout(r, 10));

		expect(store.status).toBe('error');
		expect(reconnectSpy).not.toHaveBeenCalled();
		store.disconnect();
	});

	it('reconnects on non-401 errors with backoff', async () => {
		mockFetchWithStream([], 500);
		const store = new TaskStreamStore();
		const reconnectSpy = vi.spyOn(store as any, 'scheduleReconnect');
		store.connect();

		await new Promise((r) => setTimeout(r, 10));

		expect(store.status).toBe('error');
		expect(reconnectSpy).toHaveBeenCalled();
		store.disconnect();
	});
});
