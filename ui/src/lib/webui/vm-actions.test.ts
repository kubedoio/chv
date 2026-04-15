import { describe, expect, it, vi } from 'vitest';

import type { Operation } from '$lib/api/types';
import { runVmLifecycleAction } from '$lib/webui/vm-actions';

const operations: Operation[] = [
	{
		id: 'older-start',
		resource_type: 'vm',
		resource_id: 'vm-1',
		operation_type: 'start',
		state: 'succeeded',
		created_at: '2026-04-15T09:00:00.000Z'
	},
	{
		id: 'new-start',
		resource_type: 'vm',
		resource_id: 'vm-1',
		operation_type: 'start',
		state: 'queued',
		created_at: '2026-04-15T10:00:01.000Z'
	}
];

describe('runVmLifecycleAction', () => {
	it('returns the freshest matching task reference after a lifecycle mutation', async () => {
		const perform = vi.fn().mockResolvedValue(undefined);
		const listOperations = vi.fn().mockResolvedValue(operations);

		const result = await runVmLifecycleAction({
			vmId: 'vm-1',
			vmName: 'api-01',
			action: 'start',
			perform,
			listOperations,
			now: new Date('2026-04-15T10:00:00.000Z')
		});

		expect(perform).toHaveBeenCalledTimes(1);
		expect(result).toMatchObject({
			accepted: true,
			taskId: 'new-start',
			taskLabel: 'Accepted'
		});
	});

	it('falls back to an accepted mutation notice when the task record is not visible yet', async () => {
		const result = await runVmLifecycleAction({
			vmId: 'vm-1',
			vmName: 'api-01',
			action: 'restart',
			perform: vi.fn().mockResolvedValue(undefined),
			listOperations: vi.fn().mockResolvedValue([]),
			now: new Date('2026-04-15T10:00:00.000Z')
		});

		expect(result).toMatchObject({
			accepted: true,
			taskId: null,
			taskLabel: 'Accepted'
		});
		expect(result.summary).toContain('Restart');
	});
});
