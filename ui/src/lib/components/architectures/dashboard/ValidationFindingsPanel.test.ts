import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render } from '@testing-library/svelte';
import ValidationFindingsPanel from './ValidationFindingsPanel.svelte';
import type { Finding, ValidationResult } from '$lib/bff/architectures';

function makeFinding(overrides: Partial<Finding> = {}): Finding {
	return {
		severity: 'error',
		code: 'INVALID_CIDR',
		message: 'CIDR is not parseable',
		path: 'networks[0].cidr',
		resource_ref: null,
		blocking: true,
		suggestion: null,
		...overrides
	};
}

describe('ValidationFindingsPanel', () => {
	afterEach(() => cleanup());
	it('renders the "Run validation" CTA when result is null', () => {
		const { container, getByRole } = render(ValidationFindingsPanel, {
			props: { result: null, onRevalidate: vi.fn() }
		});

		expect(container.textContent).toMatch(/no validation run yet/i);
		expect(getByRole('button', { name: /run validation/i })).toBeTruthy();
	});

	it('renders the "topology is valid" empty state when there are no findings', () => {
		const result: ValidationResult = {
			status: 'valid',
			summary: { errors: 0, warnings: 0, info: 0 },
			findings: []
		};
		const { getByTestId } = render(ValidationFindingsPanel, {
			props: { result, onRevalidate: vi.fn() }
		});

		expect(getByTestId('validation-empty').textContent?.toLowerCase()).toContain(
			'no findings'
		);
		expect(getByTestId('validation-status-pill').textContent?.toLowerCase()).toContain(
			'valid'
		);
	});

	it('sorts findings by severity (errors first), then code, then path', () => {
		const result: ValidationResult = {
			status: 'invalid',
			summary: { errors: 1, warnings: 1, info: 1 },
			findings: [
				makeFinding({ severity: 'info', code: 'STYLE_GUIDE', path: 'meta.tags' }),
				makeFinding({ severity: 'warning', code: 'DUPLICATE_NAME', path: 'instances[1].name' }),
				makeFinding({ severity: 'error', code: 'INVALID_CIDR', path: 'networks[0].cidr' })
			]
		};

		const { getAllByTestId } = render(ValidationFindingsPanel, {
			props: { result, onRevalidate: vi.fn() }
		});

		const codes = getAllByTestId('finding-code').map((el) => el.textContent);
		expect(codes).toEqual(['INVALID_CIDR', 'DUPLICATE_NAME', 'STYLE_GUIDE']);
	});

	it('uses the status to drive the status pill class', () => {
		const make = (status: 'valid' | 'warning' | 'invalid'): ValidationResult => ({
			status,
			summary: { errors: 0, warnings: 0, info: 0 },
			findings: []
		});
		const { container, rerender } = render(ValidationFindingsPanel, {
			props: { result: make('valid'), onRevalidate: vi.fn() }
		});
		expect(container.querySelector('.status-valid')).not.toBeNull();

		rerender({ result: make('warning'), onRevalidate: vi.fn() });
		expect(container.querySelector('.status-warning')).not.toBeNull();

		rerender({ result: make('invalid'), onRevalidate: vi.fn() });
		expect(container.querySelector('.status-invalid')).not.toBeNull();
	});

	it('renders the summary counts from the result', () => {
		const result: ValidationResult = {
			status: 'invalid',
			summary: { errors: 3, warnings: 2, info: 1 },
			findings: [makeFinding()]
		};
		const { getByTestId } = render(ValidationFindingsPanel, {
			props: { result, onRevalidate: vi.fn() }
		});

		expect(getByTestId('count-errors').textContent).toContain('3');
		expect(getByTestId('count-warnings').textContent).toContain('2');
		expect(getByTestId('count-info').textContent).toContain('1');
	});

	it('calls onRevalidate when the button is clicked', async () => {
		const onRevalidate = vi.fn();
		const { getByRole } = render(ValidationFindingsPanel, {
			props: { result: null, onRevalidate }
		});

		await fireEvent.click(getByRole('button', { name: /run validation/i }));
		expect(onRevalidate).toHaveBeenCalledTimes(1);
	});

	it('reflects the loading prop on the button', () => {
		const { getByRole } = render(ValidationFindingsPanel, {
			props: { result: null, loading: true, onRevalidate: vi.fn() }
		});

		const btn = getByRole('button', { name: /run validation/i });
		expect(btn.getAttribute('aria-busy')).toBe('true');
	});
});
