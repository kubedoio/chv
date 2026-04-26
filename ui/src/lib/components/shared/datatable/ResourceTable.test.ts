import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/svelte';
import ResourceTable from '../datatable/ResourceTable.svelte';

describe('ResourceTable', () => {
	const columns = [
		{ key: 'name', label: 'Name' },
		{ key: 'status', label: 'Status' }
	];

	const rows = [
		{ name: 'Alpha', status: 'Running' },
		{ name: 'Beta', status: 'Stopped' }
	];

	it('renders columns and rows', () => {
		const { getByText } = render(ResourceTable, { props: { columns, rows } });
		expect(getByText('Name')).toBeTruthy();
		expect(getByText('Status')).toBeTruthy();
		expect(getByText('Alpha')).toBeTruthy();
		expect(getByText('Beta')).toBeTruthy();
		expect(getByText('Running')).toBeTruthy();
		expect(getByText('Stopped')).toBeTruthy();
	});

	it('renders first column as links when rowHref is provided', () => {
		const { container } = render(ResourceTable, {
			props: {
				columns,
				rows,
				rowHref: (row) => `/detail/${row.name}`
			}
		});
		const links = container.querySelectorAll('a');
		expect(links.length).toBe(2);
		expect(links[0].getAttribute('href')).toBe('/detail/Alpha');
		expect(links[1].getAttribute('href')).toBe('/detail/Beta');
	});

	it('handles empty state with default title', () => {
		const { getByText } = render(ResourceTable, { props: { columns, rows: [] } });
		expect(getByText('No data')).toBeTruthy();
	});

	it('handles empty state with custom title and description', () => {
		const { getByText } = render(ResourceTable, {
			props: {
				columns,
				rows: [],
				emptyTitle: 'Nothing here',
				emptyDescription: 'Add a row to get started.'
			}
		});
		expect(getByText('Nothing here')).toBeTruthy();
		expect(getByText('Add a row to get started.')).toBeTruthy();
	});
});
