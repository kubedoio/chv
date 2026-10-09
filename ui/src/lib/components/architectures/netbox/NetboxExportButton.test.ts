import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, waitFor } from '@testing-library/svelte';
import NetboxExportButton from './NetboxExportButton.svelte';

/**
 * Export action wiring: non-production environments export directly
 * (rejections are swallowed — the store's inline banner plus the
 * mutateWithRefresh toast carry the error), production environments go
 * through the typed-name confirm dialog, which stays open on failure.
 */

describe('NetboxExportButton', () => {
	afterEach(() => cleanup());

	it('exports directly on non-production and swallows rejections (no unhandled rejection)', async () => {
		const onExport = vi.fn().mockRejectedValue(new Error('NETBOX_RUN_ACTIVE'));
		const { getByTestId, queryByTestId } = render(NetboxExportButton, {
			props: {
				architectureName: 'app-stack',
				environment: 'staging',
				exporting: false,
				onExport
			}
		});

		await fireEvent.click(getByTestId('netbox-export-button'));

		await waitFor(() => expect(onExport).toHaveBeenCalledTimes(1));
		// No confirm dialog outside production.
		expect(queryByTestId('netbox-export-confirm-dialog')).toBeNull();
	});

	it('opens the typed-name confirm dialog on production and keeps it open when the export is rejected', async () => {
		const onExport = vi.fn().mockRejectedValue(new Error('NETBOX_RUN_ACTIVE'));
		const { getByTestId } = render(NetboxExportButton, {
			props: {
				architectureName: 'app-stack',
				environment: 'production',
				exporting: false,
				onExport
			}
		});

		await fireEvent.click(getByTestId('netbox-export-button'));
		expect(getByTestId('netbox-export-confirm-dialog')).toBeTruthy();

		await fireEvent.input(getByTestId('netbox-typed-name-input'), {
			target: { value: 'app-stack' }
		});
		await fireEvent.click(getByTestId('netbox-export-confirm-button'));

		await waitFor(() => expect(onExport).toHaveBeenCalledTimes(1));
		// Rejected export: the dialog stays open so the operator can
		// retry without re-typing the name.
		expect(getByTestId('netbox-export-confirm-dialog')).toBeTruthy();
	});

	it('closes the confirm dialog after a successful production export', async () => {
		const onExport = vi.fn().mockResolvedValue(undefined);
		const { getByTestId, queryByTestId } = render(NetboxExportButton, {
			props: {
				architectureName: 'app-stack',
				environment: 'production',
				exporting: false,
				onExport
			}
		});

		await fireEvent.click(getByTestId('netbox-export-button'));
		await fireEvent.input(getByTestId('netbox-typed-name-input'), {
			target: { value: 'app-stack' }
		});
		await fireEvent.click(getByTestId('netbox-export-confirm-button'));

		await waitFor(() => expect(onExport).toHaveBeenCalledTimes(1));
		await waitFor(() =>
			expect(queryByTestId('netbox-export-confirm-dialog')).toBeNull()
		);
	});
});
