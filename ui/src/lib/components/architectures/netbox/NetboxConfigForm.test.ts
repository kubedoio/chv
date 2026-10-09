import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render } from '@testing-library/svelte';
import NetboxConfigForm from './NetboxConfigForm.svelte';
import type { NetboxConfig } from '#lib/bff/architectures.ts';

const CONFIG: NetboxConfig = {
	architecture_id: 'arch-1',
	endpoint: 'https://netbox.example.internal',
	token_secret_ref: 'netbox-arch-1',
	token_set: true,
	retention_policy: 'mark_stale',
	enable_post_apply: true,
	custom_field_prefix: 'chv_',
	site_name: 'dc1',
	updated_at: '2026-10-08T09:00:00Z'
};

function renderForm(overrides: Record<string, unknown> = {}) {
	const onSave = vi.fn().mockResolvedValue(undefined);
	const onDelete = vi.fn();
	const rendered = render(NetboxConfigForm, {
		props: {
			architectureId: 'arch-1',
			config: null,
			expectedVersion: 4,
			saving: false,
			deleting: false,
			onSave,
			onDelete,
			...overrides
		}
	});
	return { ...rendered, onSave, onDelete };
}

async function submitForm(getByTestId: (id: string) => HTMLElement) {
	await fireEvent.submit(getByTestId('netbox-config-form'));
}

describe('NetboxConfigForm', () => {
	afterEach(() => cleanup());

	it('renders the token field as type=password with write-only semantics', () => {
		const { getByTestId } = renderForm();

		const tokenInput = getByTestId('netbox-token-input') as HTMLInputElement;
		expect(tokenInput.type).toBe('password');
		expect(tokenInput.getAttribute('autocomplete')).toBe('new-password');
		// Never seeded, never echoed — write-only.
		expect(tokenInput.value).toBe('');
	});

	it('reflects token_set in the placeholder (keep-existing hint)', () => {
		const { getByTestId } = renderForm({ config: CONFIG });

		const tokenInput = getByTestId('netbox-token-input') as HTMLInputElement;
		expect(tokenInput.placeholder.toLowerCase()).toContain('token set');
		expect(tokenInput.placeholder.toLowerCase()).toContain('leave blank');
	});

	it('omits the token key entirely on empty submission (keep the existing secret)', async () => {
		const { getByTestId, onSave } = renderForm({ config: CONFIG });

		// Fill everything except the token.
		await fireEvent.input(getByTestId('netbox-endpoint-input'), {
			target: { value: 'https://netbox.example.internal' }
		});
		await submitForm(getByTestId);

		expect(onSave).toHaveBeenCalledTimes(1);
		const draft = onSave.mock.calls[0][0];
		expect('token' in draft).toBe(false);
		expect(draft.endpoint).toBe('https://netbox.example.internal');
		expect(draft.retention_policy).toBe('mark_stale');
	});

	it('includes the token exactly once when the field is filled, and clears it after dispatch', async () => {
		const { getByTestId, onSave } = renderForm({ config: CONFIG });

		await fireEvent.input(getByTestId('netbox-token-input'), {
			target: { value: 'PAbCd123' }
		});
		await submitForm(getByTestId);

		expect(onSave).toHaveBeenCalledTimes(1);
		const draft = onSave.mock.calls[0][0];
		expect(draft.token).toBe('PAbCd123');
		// The transient draft does not linger in the field.
		const tokenInput = getByTestId('netbox-token-input') as HTMLInputElement;
		expect(tokenInput.value).toBe('');
	});

	it('seeds drafts from the loaded config (token excepted)', () => {
		const { getByTestId } = renderForm({ config: CONFIG });

		expect((getByTestId('netbox-endpoint-input') as HTMLInputElement).value).toBe(
			'https://netbox.example.internal'
		);
		expect((getByTestId('netbox-secret-ref-input') as HTMLInputElement).value).toBe(
			'netbox-arch-1'
		);
		expect((getByTestId('netbox-retention-input') as HTMLSelectElement).value).toBe('mark_stale');
		expect((getByTestId('netbox-post-apply-input') as HTMLInputElement).checked).toBe(true);
		expect((getByTestId('netbox-site-input') as HTMLInputElement).value).toBe('dc1');
		expect((getByTestId('netbox-token-input') as HTMLInputElement).value).toBe('');
	});

	it('defaults the secret ref to netbox-<architecture id> when creating a config', () => {
		const { getByTestId } = renderForm();

		expect((getByTestId('netbox-secret-ref-input') as HTMLInputElement).value).toBe('netbox-arch-1');
		expect(getByTestId('netbox-config-absent').textContent).toContain('No projection config yet');
	});

	it('shows the delete button when a config exists', () => {
		const { getByTestId } = renderForm({ config: CONFIG });
		expect(getByTestId('netbox-config-delete-button')).toBeTruthy();
	});

	it('omits the delete button when no config exists yet', () => {
		const { queryByTestId } = renderForm();
		expect(queryByTestId('netbox-config-delete-button')).toBeNull();
	});

	it('surfaces the admin requirement note when the delete retention policy is selected', async () => {
		const { getByTestId } = renderForm();

		await fireEvent.change(getByTestId('netbox-retention-input'), {
			target: { value: 'delete' }
		});

		const help = document.getElementById('netbox-retention-help');
		expect(help?.textContent).toContain('requires an Admin');
	});
});
