import { render, screen, cleanup, fireEvent } from '@testing-library/svelte';
import { describe, expect, it, afterEach, vi } from 'vitest';
import PowerOffInstanceDialog from './PowerOffInstanceDialog.svelte';

describe('PowerOffInstanceDialog', () => {
	afterEach(() => {
		cleanup();
	});

	function renderDialog(props: { open?: boolean; instanceName?: string }) {
		const onConfirm = vi.fn();
		const onCancel = vi.fn();
		render(PowerOffInstanceDialog, {
			props: {
				open: props.open ?? true,
				instanceName: props.instanceName ?? 'test-instance',
				onConfirm,
				onCancel
			}
		});
		return { onConfirm, onCancel };
	}

	it('renders title with instance name', () => {
		renderDialog({ instanceName: 'db-primary' });
		expect(screen.getByText(/Power off instance "db-primary"/)).toBeTruthy();
	});

	it('shows data-loss warning', () => {
		renderDialog({});
		expect(screen.getByText(/Immediate hard stop/)).toBeTruthy();
		expect(screen.getByText(/may cause data loss/)).toBeTruthy();
		expect(screen.getByText(/Use Shutdown for a graceful stop/)).toBeTruthy();
	});

	it('calls onConfirm when Power Off is clicked', () => {
		const { onConfirm } = renderDialog({});
		const confirmButton = screen.getByRole('button', { name: /Power Off/i });
		fireEvent.click(confirmButton);
		expect(onConfirm).toHaveBeenCalledTimes(1);
	});

	it('calls onCancel when cancel is clicked', () => {
		const { onCancel } = renderDialog({});
		const cancelButton = screen.getByRole('button', { name: /Cancel/i });
		fireEvent.click(cancelButton);
		expect(onCancel).toHaveBeenCalledTimes(1);
	});
});
