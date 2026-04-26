import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/svelte';
import StateBanner from '../shared/StateBanner.svelte';

describe('StateBanner', () => {
	it('renders title and description', () => {
		const { getByText } = render(StateBanner, {
			props: {
				variant: 'empty',
				title: 'No items',
				description: 'There is nothing here yet.'
			}
		});
		expect(getByText('No items')).toBeTruthy();
		expect(getByText('There is nothing here yet.')).toBeTruthy();
	});

	it('renders hint when provided', () => {
		const { getByText } = render(StateBanner, {
			props: {
				variant: 'error',
				title: 'Error',
				description: 'Something went wrong.',
				hint: 'Try again later.'
			}
		});
		expect(getByText('Try again later.')).toBeTruthy();
	});

	it('shows loading skeletons for loading variant', () => {
		const { container } = render(StateBanner, {
			props: {
				variant: 'loading',
				title: 'Loading',
				description: 'Please wait.'
			}
		});
		expect(container.querySelector('.state-banner__skeletons')).toBeTruthy();
	});

	it('does not show skeletons for non-loading variants', () => {
		const { container } = render(StateBanner, {
			props: {
				variant: 'empty',
				title: 'Empty',
				description: 'No data.'
			}
		});
		expect(container.querySelector('.state-banner__skeletons')).toBeFalsy();
	});
});
