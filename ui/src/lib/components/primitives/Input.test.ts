import { render, screen, cleanup } from '@testing-library/svelte';
import { describe, expect, it, afterEach } from 'vitest';
import Input from './Input.svelte';

describe('Input.svelte', () => {
	afterEach(() => {
		cleanup();
	});
	it('renders correctly with default props', () => {
		render(Input);
		const input = screen.getByRole('textbox');
		expect(input).toBeTruthy();
	});

	it('applies label when provided', () => {
		render(Input, { props: { label: 'Username', id: 'username-test' } });
		expect(screen.getByLabelText('Username')).toBeTruthy();
	});

	it('binds numerical values', async () => {
		const { component } = render(Input, { props: { type: 'number', value: 10 } });
		const input = screen.getByRole('spinbutton') as HTMLInputElement;
		expect(input.value).toBe('10');
	});

	it('shows error message if provided and aria attributes map correctly', () => {
		render(Input, { props: { error: 'Invalid input' } });
		const errorText = screen.getByRole('alert');
		expect(errorText.textContent?.trim()).toBe('Invalid input');
		
		const input = screen.getByRole('textbox');
		expect(input.getAttribute('aria-invalid')).toBe('true');
	});
});
