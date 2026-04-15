import { describe, expect, it, vi } from 'vitest';
import { load } from '../../routes/tasks/+page.server';

vi.mock('$lib/bff/tasks', () => ({
	listTasks: vi.fn()
}));

import { listTasks } from '$lib/bff/tasks';

describe('tasks page server load', () => {
	it('returns ready state with tasks when listTasks succeeds', async () => {
		const mockedListTasks = vi.mocked(listTasks);
		mockedListTasks.mockResolvedValue({
			items: [
				{
					task_id: 'task-1',
					status: 'succeeded',
					operation: 'create',
					resource_kind: 'vm',
					resource_id: 'vm-1',
					actor: 'admin',
					started_unix_ms: 1700000000000,
					finished_unix_ms: 1700000001000
				}
			],
			page: { page: 1, page_size: 50, total_items: 1 },
			filters: { applied: {} }
		});

		const result = await load({
			url: new URL('http://localhost/tasks'),
			cookies: { get: () => 'token-123' }
		} as Parameters<typeof load>[0]);

		expect(result.tasks.state).toBe('ready');
		expect(result.tasks.items).toHaveLength(1);
		expect(result.tasks.page.totalItems).toBe(1);
	});

	it('returns error state when listTasks throws', async () => {
		const mockedListTasks = vi.mocked(listTasks);
		mockedListTasks.mockRejectedValue(new Error('BFF down'));

		const result = await load({
			url: new URL('http://localhost/tasks'),
			cookies: { get: () => 'token-123' }
		} as Parameters<typeof load>[0]);

		expect(result.tasks.state).toBe('error');
		expect(result.tasks.items).toHaveLength(0);
	});
});
