import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render } from '@testing-library/svelte';
import FleetCheckPanel from './FleetCheckPanel.svelte';
import type { Finding, FleetCheckResult } from '$lib/bff/architectures';

function makeFinding(overrides: Partial<Finding> = {}): Finding {
	return {
		severity: 'error',
		code: 'INSUFFICIENT_MEMORY',
		message: 'host lacks 32GB RAM',
		path: 'instances[0]',
		resource_ref: 'instance/web',
		blocking: true,
		suggestion: 'reduce instance memory or pick a larger host',
		...overrides
	};
}

function makeResult(overrides: Partial<FleetCheckResult> = {}): FleetCheckResult {
	return {
		status: 'valid',
		inventory_snapshot_id: 'snap-1',
		checked_at: new Date().toISOString(),
		findings: [],
		...overrides
	};
}

describe('FleetCheckPanel', () => {
	afterEach(() => cleanup());

	it('renders the idle state when result is null and not loading', () => {
		const { container, getByTestId, queryByTestId } = render(FleetCheckPanel, {
			props: { result: null, loading: false, onRefresh: vi.fn() }
		});

		expect(getByTestId('fleet-empty').textContent?.toLowerCase()).toContain(
			'no fleet check has been run yet'
		);
		expect(container.textContent).toMatch(/refresh inventory/i);
		// Idle has no banner and no status pill.
		expect(queryByTestId('fleet-deploy-blocked-banner')).toBeNull();
		expect(queryByTestId('fleet-status-pill')).toBeNull();
	});

	it('shows the loading label on the refresh button while loading', () => {
		const { getByTestId } = render(FleetCheckPanel, {
			props: { result: null, loading: true, onRefresh: vi.fn() }
		});

		expect(getByTestId('fleet-refresh-button').textContent?.toLowerCase()).toContain(
			'capturing inventory'
		);
	});

	it('renders the deploy-blocked banner when result has any error finding', () => {
		const result = makeResult({
			status: 'invalid',
			findings: [makeFinding()]
		});
		const { getByTestId } = render(FleetCheckPanel, {
			props: { result, loading: false, onRefresh: vi.fn() }
		});

		const banner = getByTestId('fleet-deploy-blocked-banner');
		expect(banner.getAttribute('role')).toBe('alert');
		expect(banner.textContent?.toLowerCase()).toContain('1 fleet error');
		expect(getByTestId('fleet-status-pill').textContent?.toLowerCase()).toContain('invalid');
	});

	it('omits the blocked banner for a clean (valid) result and shows the status pill', () => {
		const result = makeResult({ status: 'valid', findings: [] });
		const { getByTestId, queryByTestId } = render(FleetCheckPanel, {
			props: { result, loading: false, onRefresh: vi.fn() }
		});

		expect(queryByTestId('fleet-deploy-blocked-banner')).toBeNull();
		expect(getByTestId('fleet-status-pill').textContent?.toLowerCase()).toContain('valid');
		expect(getByTestId('fleet-checked-at').textContent?.toLowerCase()).toMatch(/last checked/);
	});

	it('renders pluralised banner copy for multiple errors', () => {
		const result = makeResult({
			status: 'invalid',
			findings: [
				makeFinding({ path: 'instances[0]' }),
				makeFinding({ path: 'instances[1]', code: 'IMAGE_NOT_FOUND' })
			]
		});
		const { getByTestId } = render(FleetCheckPanel, {
			props: { result, loading: false, onRefresh: vi.fn() }
		});

		expect(getByTestId('fleet-deploy-blocked-banner').textContent?.toLowerCase()).toContain(
			'2 fleet errors'
		);
	});
});
