import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TaskStreamStore, type TaskUpdate } from './task-stream.svelte';

describe('TaskStreamStore', () => {
	beforeEach(() => {
		vi.restoreAllMocks();
	});

	it('connects to SSE endpoint with resource_kinds filter', () => {
		const esMock = {
			close: vi.fn(),
			onopen: null as any,
			onerror: null as any,
			onmessage: null as any,
		};
		const ES = vi.fn().mockImplementation(() => esMock);
		vi.stubGlobal('EventSource', ES);

		const store = new TaskStreamStore();
		store.connect(['vm', 'node']);

		expect(ES).toHaveBeenCalledWith(
			expect.stringContaining('/v1/tasks/stream?resource_kinds=vm%2Cnode'),
			expect.anything()
		);
		expect(store.status).toBe('connecting');
		store.disconnect();
	});

	it('handles task completion and calls handler', () => {
		const esMock = { close: vi.fn(), onopen: null, onerror: null, onmessage: null } as any;
		vi.stubGlobal('EventSource', vi.fn().mockImplementation(() => esMock));

		const handler = vi.fn();
		const store = new TaskStreamStore();
		store.onTaskCompleted = handler;
		store.connect();

		esMock.onopen?.();
		expect(store.status).toBe('open');

		const task: TaskUpdate = {
			task_id: 'op-1',
			status: 'Completed',
			summary: 'CreateVm',
			resource_kind: 'vm',
			resource_id: 'vm-123',
			event_unix_ms: Date.now(),
		};

		esMock.onmessage?.({ data: JSON.stringify({ items: [task] }) });
		expect(handler).toHaveBeenCalledWith(task);

		store.disconnect();
	});

	it('deduplicates by task_id', () => {
		const esMock = { close: vi.fn(), onopen: null, onerror: null, onmessage: null } as any;
		vi.stubGlobal('EventSource', vi.fn().mockImplementation(() => esMock));

		const handler = vi.fn();
		const store = new TaskStreamStore();
		store.onTaskCompleted = handler;
		store.connect();

		const payload = JSON.stringify({
			items: [{
				task_id: 'op-1',
				status: 'Completed',
				summary: 'CreateVm',
				resource_kind: 'vm',
				resource_id: 'vm-123',
				event_unix_ms: Date.now(),
			}],
		});

		esMock.onmessage?.({ data: payload });
		esMock.onmessage?.({ data: payload });

		expect(handler).toHaveBeenCalledTimes(1);
		store.disconnect();
	});
});
