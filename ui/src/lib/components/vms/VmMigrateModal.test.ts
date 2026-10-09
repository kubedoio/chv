import { render, screen, cleanup, fireEvent, waitFor } from '@testing-library/svelte';
import { describe, expect, it, afterEach, vi } from 'vitest';
import VmMigrateModal from './VmMigrateModal.svelte';

// The modal fetches eligible nodes on open — stub the BFF hop.
vi.mock('#lib/bff/nodes.ts', () => ({
	listNodes: vi.fn().mockResolvedValue({
		items: [
			{
				node_id: 'node-b',
				name: 'node-b',
				cluster: 'c1',
				state: 'TenantReady',
				health: 'healthy',
				cpu: '2/8',
				memory: '4/32 GiB',
				storage: '10/100 GiB',
				network: 'ok',
				version: '0.3.0'
			}
		],
		page: { page: 1, page_size: 100, total_items: 1 },
		filters: { applied: {} }
	})
}));

vi.mock('#lib/api/client.ts', () => ({
	getStoredToken: vi.fn().mockReturnValue('test-token')
}));

describe('VmMigrateModal', () => {
	afterEach(() => {
		cleanup();
	});

	function renderModal() {
		const onmigrate = vi.fn();
		const onclose = vi.fn();
		const { rerender } = render(VmMigrateModal, {
			props: {
				open: true,
				vmId: 'vm-test',
				currentNodeId: 'node-a',
				submitting: false,
				onmigrate,
				onclose
			}
		});
		return { onmigrate, onclose, rerender };
	}

	async function waitForNodes() {
		await waitFor(() => {
			expect(screen.getByRole('option', { name: /node-b/i })).toBeTruthy();
		});
	}

	it('defaults to live migration: no pause-first, VM described as running', async () => {
		renderModal();
		await waitForNodes();
		const checkbox = screen.getByLabelText(/pause-first \(stop-the-world\)/i);
		expect((checkbox as HTMLInputElement).checked).toBe(false);
		expect(screen.getByText(/will remain running during the migration/i)).toBeTruthy();
		expect(screen.queryByText(/Downtime equals the full disk/i)).toBeNull();
	});

	it('shows the honest downtime warning when pause-first is checked', async () => {
		renderModal();
		await waitForNodes();
		fireEvent.click(screen.getByLabelText(/pause-first \(stop-the-world\)/i));
		await waitFor(() => {
			expect(screen.getByText(/Downtime equals the full disk and memory transfer/i)).toBeTruthy();
		});
		expect(screen.getByText(/paused before any data is copied/i)).toBeTruthy();
		expect(screen.queryByText(/will remain running during the migration/i)).toBeNull();
	});

	it('migrates live by default: onmigrate receives pauseFirst=false', async () => {
		const { onmigrate } = renderModal();
		await waitForNodes();
		fireEvent.change(screen.getByLabelText(/target node/i), { target: { value: 'node-b' } });
		fireEvent.click(screen.getByRole('button', { name: /^Migrate$/i }));
		expect(onmigrate).toHaveBeenCalledWith('node-b', false);
	});

	it('threads the pause-first opt-in: onmigrate receives pauseFirst=true', async () => {
		const { onmigrate } = renderModal();
		await waitForNodes();
		fireEvent.change(screen.getByLabelText(/target node/i), { target: { value: 'node-b' } });
		fireEvent.click(screen.getByLabelText(/pause-first \(stop-the-world\)/i));
		fireEvent.click(screen.getByRole('button', { name: /^Migrate$/i }));
		expect(onmigrate).toHaveBeenCalledWith('node-b', true);
	});

	it('resets the pause-first toggle when the modal reopens', async () => {
		const { rerender } = renderModal();
		await waitForNodes();
		fireEvent.click(screen.getByLabelText(/pause-first \(stop-the-world\)/i));
		expect((screen.getByLabelText(/pause-first \(stop-the-world\)/i) as HTMLInputElement).checked)
			.toBe(true);
		// Close (open=false unmounts the form body), then reopen: the
		// opt-in must not survive as stale state — the same reset the
		// target-node select gets.
		await rerender({ open: false });
		await rerender({ open: true });
		await waitForNodes();
		expect((screen.getByLabelText(/pause-first \(stop-the-world\)/i) as HTMLInputElement).checked)
			.toBe(false);
		expect(screen.getByText(/will remain running during the migration/i)).toBeTruthy();
	});
});
