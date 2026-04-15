import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/svelte';
import Badge from './Badge.svelte';

describe('Badge', () => {
	it('renders the label text', () => {
		const { getByText } = render(Badge, { props: { label: 'Healthy', tone: 'healthy' } });
		expect(getByText('Healthy')).toBeTruthy();
	});

	it.each([
		['healthy', 'badge--healthy'],
		['warning', 'badge--warning'],
		['degraded', 'badge--degraded'],
		['failed', 'badge--failed'],
		['unknown', 'badge--unknown']
	] as const)('applies the correct CSS class for tone %s', (tone, expectedClass) => {
		const { container } = render(Badge, { props: { label: 'Test', tone } });
		const badge = container.querySelector('.badge');
		expect(badge?.classList.contains(expectedClass)).toBe(true);
	});
});
