import { describe, expect, it, vi } from 'vitest';
import { handleVmMutation } from '$lib/webui/vm-server-actions';
import * as vmsModule from '$lib/bff/vms';

vi.mock('$lib/bff/vms', () => ({
	mutateVm: vi.fn(),
	deleteVm: vi.fn()
}));

describe('handleVmMutation', () => {
	it('rejects missing vm_id', async () => {
		const formData = new FormData();
		formData.set('action', 'start');
		const result = await handleVmMutation(formData, 'token');
		expect(result).toMatchObject({ status: 400 });
	});

	it('rejects missing action', async () => {
		const formData = new FormData();
		formData.set('vm_id', 'vm-1');
		const result = await handleVmMutation(formData, 'token');
		expect(result).toMatchObject({ status: 400 });
	});

	it('rejects invalid action', async () => {
		const formData = new FormData();
		formData.set('vm_id', 'vm-1');
		formData.set('action', 'explode');
		const result = await handleVmMutation(formData, 'token');
		expect(result).toMatchObject({ status: 400 });
	});

	it('accepts start action and calls mutateVm', async () => {
		const mutateVm = vi.mocked(vmsModule.mutateVm).mockResolvedValue({
			accepted: true,
			task_id: 'task-1',
			vm_id: 'vm-1',
			summary: 'Start accepted'
		});
		const formData = new FormData();
		formData.set('vm_id', 'vm-1');
		formData.set('action', 'start');
		const result = await handleVmMutation(formData, 'token');
		expect(mutateVm).toHaveBeenCalledWith({ vm_id: 'vm-1', action: 'start', force: false }, 'token');
		expect(result).toMatchObject({ accepted: true, action: 'start' });
	});

	it('accepts shutdown action and maps it to stop with force=false', async () => {
		const mutateVm = vi.mocked(vmsModule.mutateVm).mockResolvedValue({
			accepted: true,
			task_id: 'task-2',
			vm_id: 'vm-1',
			summary: 'Stop accepted'
		});
		const formData = new FormData();
		formData.set('vm_id', 'vm-1');
		formData.set('action', 'shutdown');
		const result = await handleVmMutation(formData, 'token');
		expect(mutateVm).toHaveBeenCalledWith({ vm_id: 'vm-1', action: 'stop', force: false }, 'token');
		expect(result).toMatchObject({ accepted: true, action: 'shutdown' });
	});

	it('accepts poweroff action and maps it to stop with force=true', async () => {
		const mutateVm = vi.mocked(vmsModule.mutateVm).mockResolvedValue({
			accepted: true,
			task_id: 'task-3',
			vm_id: 'vm-1',
			summary: 'Force stop accepted'
		});
		const formData = new FormData();
		formData.set('vm_id', 'vm-1');
		formData.set('action', 'poweroff');
		const result = await handleVmMutation(formData, 'token');
		expect(mutateVm).toHaveBeenCalledWith({ vm_id: 'vm-1', action: 'stop', force: true }, 'token');
		expect(result).toMatchObject({ accepted: true, action: 'poweroff' });
	});

	it('accepts delete action and calls deleteVm', async () => {
		const deleteVm = vi.mocked(vmsModule.deleteVm).mockResolvedValue({
			vm_id: 'vm-1',
			operation_id: 'op-1',
			status: 'accepted'
		});
		const formData = new FormData();
		formData.set('vm_id', 'vm-1');
		formData.set('action', 'delete');
		const result = await handleVmMutation(formData, 'token');
		expect(deleteVm).toHaveBeenCalledWith({ vm_id: 'vm-1', requested_by: 'webui' }, 'token');
		expect(result).toMatchObject({ accepted: true, action: 'delete' });
	});
});
