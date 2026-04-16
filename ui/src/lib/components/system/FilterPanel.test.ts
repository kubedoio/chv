import { describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import FilterPanel from './FilterPanel.svelte';

vi.mock('$app/stores', () => ({
	page: {
		subscribe: (fn: (value: { url: URL }) => void) => {
			fn({ url: new URL('http://localhost/nodes?sort=name') });
			return () => {};
		}
	}
}));

describe('FilterPanel', () => {
	const filters = [
		{ name: 'query', label: 'Search', type: 'search' as const },
		{
			name: 'state',
			label: 'State',
			type: 'select' as const,
			options: [
				{ value: 'all', label: 'All' },
				{ value: 'online', label: 'Online' }
			]
		}
	];

	it('renders a GET form', () => {
		const { container } = render(FilterPanel, { props: { filters, values: {} } });
		const form = container.querySelector('form');
		expect(form).toBeTruthy();
		expect(form?.getAttribute('method')).toBe('GET');
	});

	it('renders search and select inputs', () => {
		const { container } = render(FilterPanel, { props: { filters, values: {} } });
		expect(container.querySelector('input[type="search"]')).toBeTruthy();
		expect(container.querySelector('select')).toBeTruthy();
	});

	it('preserves existing non-filter query params as hidden inputs', () => {
		const { container } = render(FilterPanel, { props: { filters, values: {} } });
		const hidden = container.querySelector('input[type="hidden"]');
		expect(hidden).toBeTruthy();
		expect(hidden?.getAttribute('name')).toBe('sort');
		expect(hidden?.getAttribute('value')).toBe('name');
	});

	it('reflects current values in inputs', () => {
		const { container } = render(FilterPanel, {
			props: { filters, values: { query: 'node-1', state: 'online' } }
		});
		const search = container.querySelector('input[type="search"]') as HTMLInputElement;
		const select = container.querySelector('select') as HTMLSelectElement;
		expect(search.value).toBe('node-1');
		expect(select.value).toBe('online');
	});

	it('renders reset link with preserved non-filter params', () => {
		const { container } = render(FilterPanel, { props: { filters, values: {} } });
		const reset = container.querySelector('.filter-panel__actions a');
		expect(reset?.getAttribute('href')).toBe('?sort=name');
	});
});
