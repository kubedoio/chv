import { render, screen, cleanup } from '@testing-library/svelte';
import { describe, expect, it, afterEach } from 'vitest';
import InstanceStatusBadge from './InstanceStatusBadge.svelte';

describe('InstanceStatusBadge', () => {
	afterEach(() => {
		cleanup();
	});

	it('renders RUNNING with success dot', () => {
		render(InstanceStatusBadge, { props: { status: 'running' } });
		expect(screen.getByText('RUNNING')).toBeTruthy();
		expect(screen.getByLabelText('Status: RUNNING')).toBeTruthy();
	});

	it('renders STOPPED with neutral dot', () => {
		render(InstanceStatusBadge, { props: { status: 'stopped' } });
		expect(screen.getByText('STOPPED')).toBeTruthy();
	});

	it('renders ERROR with danger dot', () => {
		render(InstanceStatusBadge, { props: { status: 'error' } });
		expect(screen.getByText('ERROR')).toBeTruthy();
	});

	it('renders PAUSED with warning dot', () => {
		render(InstanceStatusBadge, { props: { status: 'paused' } });
		expect(screen.getByText('PAUSED')).toBeTruthy();
	});

	it('renders UNKNOWN for unrecognized status', () => {
		render(InstanceStatusBadge, { props: { status: 'unknown' } });
		expect(screen.getByText('UNKNOWN')).toBeTruthy();
	});

	it('hides text when showText is false', () => {
		render(InstanceStatusBadge, { props: { status: 'running', showText: false } });
		expect(screen.queryByText('RUNNING')).toBeNull();
		expect(screen.getByLabelText('Status: RUNNING')).toBeTruthy();
	});
});
