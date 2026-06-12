import { beforeEach, describe, expect, it, vi } from 'vitest';

// mutation.svelte.ts pulls in live-state.svelte.ts which transitively imports
// SvelteKit modules. Mock both so this suite runs under jsdom without a
// SvelteKit runtime, mirroring live-state.test.ts.
vi.mock('$env/dynamic/public', () => ({
	env: {}
}));

vi.mock('$app/navigation', () => ({
	goto: vi.fn(),
	invalidateAll: vi.fn()
}));

import { liveState } from './live-state.svelte';
import { mutateWithRefresh } from './mutation.svelte';
import { toast } from './toast.svelte';

describe('mutateWithRefresh', () => {
	beforeEach(() => {
		vi.restoreAllMocks();
	});

	it('forwards patterns, sidebar, detailId, delayMs to liveState.invalidateAndRefresh on success', async () => {
		const refreshSpy = vi
			.spyOn(liveState, 'invalidateAndRefresh')
			.mockResolvedValue(undefined);
		const mutator = vi.fn().mockResolvedValue({ id: 'vm-1' });

		const result = await mutateWithRefresh(mutator, {
			patterns: ['vms:', 'nodes:'],
			sidebar: false,
			detailId: 'abc-123',
			delayMs: 750
		});

		expect(result).toEqual({ id: 'vm-1' });
		expect(mutator).toHaveBeenCalledTimes(1);
		expect(refreshSpy).toHaveBeenCalledTimes(1);
		expect(refreshSpy).toHaveBeenCalledWith({
			patterns: ['vms:', 'nodes:'],
			sidebar: false,
			detailId: 'abc-123',
			delayMs: 750
		});
	});

	it('defaults sidebar to true when not provided', async () => {
		const refreshSpy = vi
			.spyOn(liveState, 'invalidateAndRefresh')
			.mockResolvedValue(undefined);
		const mutator = vi.fn().mockResolvedValue('ok');

		await mutateWithRefresh(mutator, { patterns: ['vms:'] });

		expect(refreshSpy).toHaveBeenCalledWith(
			expect.objectContaining({ sidebar: true })
		);
	});

	it('shows success toast when successMessage is a string', async () => {
		vi.spyOn(liveState, 'invalidateAndRefresh').mockResolvedValue(undefined);
		const successSpy = vi.spyOn(toast, 'success').mockImplementation(() => {});
		const mutator = vi.fn().mockResolvedValue({ id: 'vm-1' });

		await mutateWithRefresh(mutator, { successMessage: 'VM created' });

		expect(successSpy).toHaveBeenCalledTimes(1);
		expect(successSpy).toHaveBeenCalledWith('VM created');
	});

	it('invokes successMessage callback with the mutator result and toasts the returned string', async () => {
		vi.spyOn(liveState, 'invalidateAndRefresh').mockResolvedValue(undefined);
		const successSpy = vi.spyOn(toast, 'success').mockImplementation(() => {});
		const mutator = vi.fn().mockResolvedValue({ id: 'vm-42', name: 'web-1' });
		const successMessage = vi.fn(
			(r: { id: string; name: string }) => `Created ${r.name} (${r.id})`
		);

		await mutateWithRefresh(mutator, { successMessage });

		expect(successMessage).toHaveBeenCalledTimes(1);
		expect(successMessage).toHaveBeenCalledWith({ id: 'vm-42', name: 'web-1' });
		expect(successSpy).toHaveBeenCalledWith('Created web-1 (vm-42)');
	});

	it('does not show a toast when successMessage is omitted', async () => {
		vi.spyOn(liveState, 'invalidateAndRefresh').mockResolvedValue(undefined);
		const successSpy = vi.spyOn(toast, 'success').mockImplementation(() => {});
		const errorSpy = vi.spyOn(toast, 'error').mockImplementation(() => {});
		const mutator = vi.fn().mockResolvedValue('ok');

		await mutateWithRefresh(mutator);

		expect(successSpy).not.toHaveBeenCalled();
		expect(errorSpy).not.toHaveBeenCalled();
	});

	it('does not call invalidateAndRefresh when skipRefresh is true', async () => {
		const refreshSpy = vi
			.spyOn(liveState, 'invalidateAndRefresh')
			.mockResolvedValue(undefined);
		const mutator = vi.fn().mockResolvedValue('ok');

		const result = await mutateWithRefresh(mutator, { skipRefresh: true });

		expect(result).toBe('ok');
		expect(refreshSpy).not.toHaveBeenCalled();
	});

	it('shows error toast and re-throws when the mutator rejects', async () => {
		const refreshSpy = vi
			.spyOn(liveState, 'invalidateAndRefresh')
			.mockResolvedValue(undefined);
		const errorSpy = vi.spyOn(toast, 'error').mockImplementation(() => {});
		const boom = new Error('boom');
		const mutator = vi.fn().mockRejectedValue(boom);

		await expect(mutateWithRefresh(mutator)).rejects.toBe(boom);

		expect(errorSpy).toHaveBeenCalledTimes(1);
		expect(errorSpy).toHaveBeenCalledWith('boom');
		// Refresh must not run on the failure path.
		expect(refreshSpy).not.toHaveBeenCalled();
	});

	it('uses errorMessage option as the toast text when the mutator rejects', async () => {
		const errorSpy = vi.spyOn(toast, 'error').mockImplementation(() => {});
		const mutator = vi.fn().mockRejectedValue(new Error('low-level detail'));

		await expect(
			mutateWithRefresh(mutator, { errorMessage: 'Could not start VM' })
		).rejects.toThrow('low-level detail');

		expect(errorSpy).toHaveBeenCalledWith('Could not start VM');
	});

	it('returns the mutator result and warns when invalidateAndRefresh throws (refresh failure is non-fatal)', async () => {
		const refreshErr = new Error('refresh exploded');
		vi.spyOn(liveState, 'invalidateAndRefresh').mockRejectedValue(refreshErr);
		const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
		const errorSpy = vi.spyOn(toast, 'error').mockImplementation(() => {});
		const mutator = vi.fn().mockResolvedValue({ id: 'vm-1' });

		const result = await mutateWithRefresh(mutator);

		expect(result).toEqual({ id: 'vm-1' });
		// Refresh failure must NOT surface as a user-facing error.
		expect(errorSpy).not.toHaveBeenCalled();
		expect(warnSpy).toHaveBeenCalledTimes(1);
		expect(warnSpy.mock.calls[0]?.[0]).toContain('[mutateWithRefresh]');
		expect(warnSpy.mock.calls[0]?.[1]).toBe(refreshErr);
	});
});
