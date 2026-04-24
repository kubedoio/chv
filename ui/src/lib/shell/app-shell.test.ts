import { describe, expect, it } from 'vitest';

import {
	getPageDefinition,
	getPrimaryStateDefinition,
	getTopLevelPageDefinitions,
	navigationItems
} from '$lib/shell/app-shell';

describe('app shell definitions', () => {
	it('matches the accepted top-level IA order', () => {
		expect(navigationItems.map((item) => item.href)).toEqual([
			'/',
			'/clusters',
			'/nodes',
			'/vms',
			'/volumes',
			'/networks',
			'/images',
			'/tasks',
			'/events',
			'/backup-jobs',
			'/settings'
		]);
	});

	it('provides placeholder definitions for every starter-bundle top-level page', () => {
		const pages = getTopLevelPageDefinitions();

		expect(pages.map((page) => page.href)).toEqual(navigationItems.map((item) => item.href));
		expect(pages.every((page) => page.states.loading && page.states.empty && page.states.error)).toBe(
			true
		);
	});

	it('can resolve current-page metadata from a concrete route', () => {
		expect(getPageDefinition('/vms').title).toBe('Instances');
		expect(getPageDefinition('/events').navLabel).toBe('Events');
		expect(getPageDefinition('/unknown').title).toBe('Overview');
	});

	it('maps legacy top-level routes to the closest shell section instead of Overview', () => {
		expect(getPageDefinition('/storage').title).toBe('Storage Pools');
		expect(getPageDefinition('/operations').title).toBe('Tasks');
		expect(getPageDefinition('/templates').title).toBe('Images');
		expect(getPageDefinition('/backup-jobs').title).toBe('Backups');
		expect(getPageDefinition('/maintenance').title).toBe('Backups');
		expect(getPageDefinition('/quotas').title).toBe('Settings');
		expect(getPageDefinition('/metrics').title).toBe('Overview');
	});

	it('selects one primary placeholder state per page', () => {
		const overview = getPageDefinition('/');
		const clusters = getPageDefinition('/clusters');
		const networks = getPageDefinition('/networks');

		expect(getPrimaryStateDefinition(overview)).toEqual(overview.states.loading);
		expect(getPrimaryStateDefinition(clusters)).toEqual(clusters.states.empty);
		expect(getPrimaryStateDefinition(networks)).toEqual(networks.states.error);
	});
});
