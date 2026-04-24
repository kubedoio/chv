import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/svelte';
import Badge from './Badge.svelte';

describe('Badge', () => {
	it('renders the label text', () => {
		const { getByText } = render(Badge, { props: { label: 'Healthy', tone: 'healthy' } });
		expect(getByText('Healthy')).toBeTruthy();
	});

	it.each([
		['healthy', 'bg-[var(--status-healthy-bg)]'],
		['warning', 'bg-[var(--status-warning-bg)]'],
		['degraded', 'bg-[var(--status-degraded-bg)]'],
		['failed', 'bg-[var(--status-failed-bg)]'],
		['unknown', 'bg-[var(--status-unknown-bg)]']
	] as const)('applies the correct CSS class for tone %s', (tone, expectedClass) => {
		const { container } = render(Badge, { props: { label: 'Test', tone } });
		const badge = container.querySelector('span');
		expect(badge?.classList.contains(expectedClass)).toBe(true);
	});
});
