import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render } from '@testing-library/svelte';
import FindingItem from './FindingItem.svelte';
import type { Finding } from '$lib/bff/architectures';

const BASE_FINDING: Finding = {
	severity: 'error',
	code: 'INVALID_CIDR',
	message: 'CIDR is not parseable',
	path: 'networks[0].cidr',
	resource_ref: 'networks/lan',
	blocking: true,
	suggestion: 'Use 10.0.0.0/24'
};

describe('FindingItem', () => {
	afterEach(() => cleanup());
	it('renders severity, code, message and path', () => {
		const { getByTestId } = render(FindingItem, { props: { finding: BASE_FINDING } });

		expect(getByTestId('finding-severity').textContent).toMatch(/error/i);
		expect(getByTestId('finding-code').textContent).toBe('INVALID_CIDR');
		expect(getByTestId('finding-message').textContent).toBe('CIDR is not parseable');
		expect(getByTestId('finding-path').textContent).toBe('networks[0].cidr');
	});

	it('renders the suggestion when present and omits it when null', () => {
		const { getByTestId, queryByTestId, rerender } = render(FindingItem, {
			props: { finding: BASE_FINDING }
		});

		expect(getByTestId('finding-suggestion').textContent).toContain('Use 10.0.0.0/24');
		expect(getByTestId('finding-suggestion').textContent).toContain('Try:');

		rerender({ finding: { ...BASE_FINDING, suggestion: null } });
		expect(queryByTestId('finding-suggestion')).toBeNull();
	});

	it('renders the resource_ref as a button and emits onSelectResource on click', async () => {
		const onSelect = vi.fn();
		const { getByTestId } = render(FindingItem, {
			props: { finding: BASE_FINDING, onSelectResource: onSelect }
		});

		const btn = getByTestId('finding-resource-ref');
		expect(btn.tagName.toLowerCase()).toBe('button');
		await fireEvent.click(btn);
		expect(onSelect).toHaveBeenCalledWith('networks/lan');
	});

	it('omits the resource pill entirely when resource_ref is null', () => {
		const { queryByTestId } = render(FindingItem, {
			props: { finding: { ...BASE_FINDING, resource_ref: null } }
		});

		expect(queryByTestId('finding-resource-ref')).toBeNull();
	});

	it('shows the blocking pill only when blocking is true', () => {
		const { container, rerender } = render(FindingItem, {
			props: { finding: BASE_FINDING }
		});

		expect(container.textContent?.toLowerCase()).toContain('blocking');

		rerender({ finding: { ...BASE_FINDING, blocking: false } });
		expect(container.querySelector('.blocking-pill')).toBeNull();
	});

	it('applies severity-specific class for warning and info', () => {
		const { container, rerender } = render(FindingItem, {
			props: { finding: { ...BASE_FINDING, severity: 'warning' } }
		});
		expect(container.querySelector('.finding-warning')).not.toBeNull();

		rerender({ finding: { ...BASE_FINDING, severity: 'info' } });
		expect(container.querySelector('.finding-info')).not.toBeNull();
	});
});
