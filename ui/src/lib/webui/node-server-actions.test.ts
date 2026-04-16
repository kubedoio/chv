import { describe, expect, it, vi } from 'vitest';
import { handleNodeMutation } from './node-server-actions';
import { BFFError } from '$lib/bff/client';

vi.mock('$lib/bff/nodes', () => ({
	mutateNode: vi.fn()
}));

import { mutateNode } from '$lib/bff/nodes';

describe('handleNodeMutation', () => {
	it('returns success with action for valid pause_scheduling mutation', async () => {
		const mockedMutateNode = vi.mocked(mutateNode);
		mockedMutateNode.mockResolvedValue({
			accepted: true,
			task_id: 'task-1',
			node_id: 'node-1',
			summary: 'Paused scheduling'
		});

		const formData = new FormData();
		formData.set('node_id', 'node-1');
		formData.set('action', 'pause_scheduling');

		const result = await handleNodeMutation(formData, 'token-123');

		expect(mockedMutateNode).toHaveBeenCalledWith(
			{ node_id: 'node-1', action: 'pause_scheduling' },
			'token-123'
		);
		expect(result).toMatchObject({
			accepted: true,
			action: 'pause_scheduling',
			task_id: 'task-1'
		});
	});

	it('returns 400 fail when node_id is missing', async () => {
		const formData = new FormData();
		formData.set('action', 'drain');

		const result = await handleNodeMutation(formData, 'token-123');

		expect(result).toMatchObject({
			status: 400,
			data: { message: 'Missing node_id or action' }
		});
	});

	it('returns 400 fail for invalid action', async () => {
		const formData = new FormData();
		formData.set('node_id', 'node-1');
		formData.set('action', 'destroy');

		const result = await handleNodeMutation(formData, 'token-123');

		expect(result).toMatchObject({
			status: 400,
			data: { message: 'Invalid action' }
		});
	});

	it('returns 500 fail when mutateNode throws a BFFError', async () => {
		const mockedMutateNode = vi.mocked(mutateNode);
		mockedMutateNode.mockRejectedValue(new BFFError('Node not found', 404, 'NOT_FOUND'));

		const formData = new FormData();
		formData.set('node_id', 'node-1');
		formData.set('action', 'enter_maintenance');

		const result = await handleNodeMutation(formData, 'token-123');

		expect(result).toMatchObject({
			status: 500,
			data: { message: 'Node not found' }
		});
	});
});
