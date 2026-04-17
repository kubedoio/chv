import { render, screen, cleanup } from '@testing-library/svelte';
import { describe, expect, it, afterEach } from 'vitest';
import SearchModal from './SearchModal.svelte';

describe('SearchModal.svelte', () => {
	afterEach(() => {
		cleanup();
	});

	it('renders correctly when open', () => {
		render(SearchModal, { props: { open: true } });
		// The search modal contains a search input
		const input = screen.getByPlaceholderText(/Search/i);
		expect(input).toBeTruthy();
	});

	it('does not render when closed', () => {
		render(SearchModal, { props: { open: false } });
		const input = screen.queryByPlaceholderText(/Search/i);
		expect(input).toBeNull();
	});
});
