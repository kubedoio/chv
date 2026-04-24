import { render, screen, cleanup, fireEvent } from '@testing-library/svelte';
import { describe, expect, it, afterEach, vi } from 'vitest';
import DeleteInstanceDialog from './DeleteInstanceDialog.svelte';

describe('DeleteInstanceDialog', () => {
	afterEach(() => {
		cleanup();
	});

	function renderDialog(props: { open?: boolean; instanceName?: string; instanceId?: string }) {
		const onConfirm = vi.fn();
		const onCancel = vi.fn();
		const { component } = render(DeleteInstanceDialog, {
			props: {
				open: props.open ?? true,
				instanceName: props.instanceName ?? 'test-instance',
				instanceId: props.instanceId ?? 'inst-1',
				onConfirm,
				onCancel
			}
		});
		return { component, onConfirm, onCancel };
	}

	it('renders title with instance name', () => {
		renderDialog({ instanceName: 'web-server-01' });
		expect(screen.getByText(/Delete instance "web-server-01"/)).toBeTruthy();
	});

	it('lists affected items', () => {
		renderDialog({});
		expect(screen.getByText('Instance configuration')).toBeTruthy();
		expect(screen.getByText('Root disk')).toBeTruthy();
		expect(screen.getByText('Cloud-init disk')).toBeTruthy();
		expect(screen.getByText('Runtime state')).toBeTruthy();
	});

	it('disables delete button when confirmation text is empty', () => {
		renderDialog({});
		const deleteButton = screen.getByRole('button', { name: /Delete Instance/i });
		expect(deleteButton).toBeDisabled();
	});

	it('disables delete button when confirmation text does not match instance name', () => {
		renderDialog({ instanceName: 'web-server-01' });
		const input = screen.getByLabelText(/Type instance name to confirm deletion/i);
		fireEvent.input(input, { target: { value: 'wrong-name' } });
		const deleteButton = screen.getByRole('button', { name: /Delete Instance/i });
		expect(deleteButton).toBeDisabled();
	});

	it('enables delete button when confirmation text exactly matches instance name', () => {
		renderDialog({ instanceName: 'web-server-01' });
		const input = screen.getByLabelText(/Type instance name to confirm deletion/i);
		fireEvent.input(input, { target: { value: 'web-server-01' } });
		const deleteButton = screen.getByRole('button', { name: /Delete Instance/i });
		expect(deleteButton).not.toBeDisabled();
	});

	it('trims accidental spaces before comparing confirmation text', () => {
		renderDialog({ instanceName: 'web-server-01' });
		const input = screen.getByLabelText(/Type instance name to confirm deletion/i);
		fireEvent.input(input, { target: { value: '  web-server-01  ' } });
		const deleteButton = screen.getByRole('button', { name: /Delete Instance/i });
		expect(deleteButton).not.toBeDisabled();
	});

	it('calls onConfirm when delete is clicked with matching text', () => {
		const { onConfirm } = renderDialog({ instanceName: 'web-server-01' });
		const input = screen.getByLabelText(/Type instance name to confirm deletion/i);
		fireEvent.input(input, { target: { value: 'web-server-01' } });
		const deleteButton = screen.getByRole('button', { name: /Delete Instance/i });
		fireEvent.click(deleteButton);
		expect(onConfirm).toHaveBeenCalledTimes(1);
	});

	it('calls onCancel when cancel is clicked', () => {
		const { onCancel } = renderDialog({});
		const cancelButton = screen.getByRole('button', { name: /Cancel/i });
		fireEvent.click(cancelButton);
		expect(onCancel).toHaveBeenCalledTimes(1);
	});
});
