import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render } from '@testing-library/svelte';

// Same environment mocks as architecture-netbox-store.test.ts: the
// panel transitively imports architecture-store.svelte.ts (for
// StaleVersionError), which pulls mutation.svelte → live-state.svelte →
// SvelteKit's $app modules.
vi.mock('$app/env/public', () => ({
	PUBLIC_CHV_API_BASE_URL: ''
}));

vi.mock('$app/navigation', () => ({
	goto: vi.fn(),
	refreshAll: vi.fn()
}));

vi.mock('#lib/api/client.ts', () => ({
	getStoredToken: vi.fn(() => 'test-token'),
	clearToken: vi.fn()
}));

// The panel is a thin view over the netbox store singleton; replace it
// with a plain mutable object so each scenario flips one field before
// rendering (the mock has no reactivity — values are read at render).
vi.mock('#lib/stores/architecture-netbox-store.svelte.ts', () => ({
	architectureNetboxStore: {
		config: null,
		configLoading: false,
		configError: null,
		dryRunPlan: null,
		dryRunLoading: false,
		dryRunError: null,
		runs: [],
		runsLoading: false,
		runsError: null,
		currentRun: null,
		runLoading: false,
		runError: null,
		exporting: false,
		retrying: false,
		exportError: null,
		retryError: null,
		reset: vi.fn(),
		loadConfig: vi.fn(),
		loadRuns: vi.fn(),
		loadRun: vi.fn(),
		runDryRun: vi.fn(),
		exportNow: vi.fn(),
		retryRun: vi.fn(),
		saveConfig: vi.fn(),
		deleteConfig: vi.fn()
	}
}));

import NetboxPanel from './NetboxPanel.svelte';
import type { Architecture, NetboxConfig } from '#lib/bff/architectures.ts';
import { architectureNetboxStore } from '#lib/stores/architecture-netbox-store.svelte.ts';

const ARCHITECTURE: Architecture = {
	id: 'arch-1',
	name: 'app-stack',
	display_name: 'App stack',
	description: null,
	environment: 'staging',
	status: 'applied',
	owner_user_id: null,
	last_validation_status: 'passed',
	last_fleet_check_status: null,
	version_number: 3,
	created_at: '2026-10-08T09:00:00Z',
	updated_at: '2026-10-08T09:00:00Z',
	archived_at: null
};

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

// Test-side handle over the mocked store singleton: the real store
// exposes read-only getters, the mock above is a plain object, so this
// mapped type just strips the readonly-ness for assignment.
type StoreShape = typeof architectureNetboxStore;
const store = architectureNetboxStore as unknown as {
	-readonly [K in keyof StoreShape]: StoreShape[K];
};

function renderPanel() {
	return render(NetboxPanel, {
		props: { architecture: ARCHITECTURE, onStaleVersion: vi.fn() }
	});
}

describe('NetboxPanel', () => {
	afterEach(() => cleanup());

	beforeEach(() => {
		store.config = null;
		store.configLoading = false;
		store.configError = null;
		store.dryRunPlan = null;
		store.dryRunLoading = false;
		store.dryRunError = null;
		store.runs = [];
		store.runsLoading = false;
		store.runsError = null;
		store.currentRun = null;
		store.runLoading = false;
		store.exporting = false;
		store.retrying = false;
		store.exportError = null;
		store.retryError = null;
	});

	it('lazy-loads config and runs on first activation', () => {
		renderPanel();

		expect(store.loadConfig).toHaveBeenCalledWith('arch-1');
		expect(store.loadRuns).toHaveBeenCalledWith('arch-1');
	});

	it('banners an active-run export refusal inline (409 NETBOX_RUN_ACTIVE)', () => {
		store.exportError = {
			code: 'NETBOX_RUN_ACTIVE',
			message: 'A projection run is already queued or running'
		};

		const { getByTestId } = renderPanel();

		const banner = getByTestId('netbox-export-error');
		expect(banner.getAttribute('role')).toBe('alert');
		expect(banner.getAttribute('data-netbox-error-code')).toBe('NETBOX_RUN_ACTIVE');
		expect(banner.textContent).toContain('Export failed.');
		expect(banner.textContent).toContain('already queued or running');
	});

	it('banners the production admin gate on export (403 PRODUCTION_REQUIRES_ADMIN)', () => {
		store.exportError = {
			code: 'PRODUCTION_REQUIRES_ADMIN',
			message: 'Admin role required'
		};

		const { getByTestId } = renderPanel();

		const banner = getByTestId('netbox-export-error');
		expect(banner.textContent).toContain('require an admin');
	});

	it('falls back to the server message for unknown export error codes', () => {
		store.exportError = { code: 'INTERNAL', message: 'unexpected BFF failure' };

		const { getByTestId } = renderPanel();

		expect(getByTestId('netbox-export-error').textContent).toContain('unexpected BFF failure');
	});

	it('banners a non-retryable run refusal inline (409 PROJECTION_RUN_NOT_RETRYABLE)', () => {
		store.retryError = {
			code: 'PROJECTION_RUN_NOT_RETRYABLE',
			message: 'Run is not retryable'
		};

		const { getByTestId } = renderPanel();

		const banner = getByTestId('netbox-retry-error');
		expect(banner.getAttribute('role')).toBe('alert');
		expect(banner.getAttribute('data-netbox-error-code')).toBe('PROJECTION_RUN_NOT_RETRYABLE');
		expect(banner.textContent).toContain('Retry failed.');
		expect(banner.textContent).toContain('no longer be retried');
	});

	it('banners the active-run text for a retry refused by NETBOX_RUN_ACTIVE', () => {
		store.retryError = {
			code: 'NETBOX_RUN_ACTIVE',
			message: 'A projection run is already queued or running'
		};

		const { getByTestId } = renderPanel();

		expect(getByTestId('netbox-retry-error').textContent).toContain('already queued or running');
	});

	it('falls back to the server message for unknown retry error codes', () => {
		store.retryError = { code: 'INTERNAL', message: 'retry exploded' };

		const { getByTestId } = renderPanel();

		expect(getByTestId('netbox-retry-error').textContent).toContain('retry exploded');
	});

	it('banners a NetBox-side unreachable dry-run distinctly (502 NETBOX_UNREACHABLE)', () => {
		store.config = CONFIG;
		store.dryRunError = { code: 'NETBOX_UNREACHABLE', message: 'netbox 502' };

		const { getByTestId } = renderPanel();

		const banner = getByTestId('netbox-dry-run-error-banner');
		expect(banner.getAttribute('role')).toBe('alert');
		expect(banner.getAttribute('data-netbox-error-code')).toBe('NETBOX_UNREACHABLE');
		expect(banner.textContent).toContain('NetBox is unreachable');
		expect(banner.textContent).toContain('check the endpoint and network');
	});

	it('banners a rejected-token dry-run distinctly (502 NETBOX_AUTH_FAILED)', () => {
		store.config = CONFIG;
		store.dryRunError = { code: 'NETBOX_AUTH_FAILED', message: 'netbox 502' };

		const { getByTestId } = renderPanel();

		const banner = getByTestId('netbox-dry-run-error-banner');
		expect(banner.getAttribute('data-netbox-error-code')).toBe('NETBOX_AUTH_FAILED');
		expect(banner.textContent).toContain('rejected the configured token');
	});

	it('falls back to the server message for unknown dry-run error codes', () => {
		store.config = CONFIG;
		store.dryRunError = { code: 'NETBOX_NOT_APPLIED', message: 'no succeeded apply run' };

		const { getByTestId } = renderPanel();

		expect(getByTestId('netbox-dry-run-error-banner').textContent).toContain(
			'no succeeded apply run'
		);
	});
});
