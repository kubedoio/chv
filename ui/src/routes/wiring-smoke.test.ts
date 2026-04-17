import { describe, expect, it } from 'vitest';

import { load as backupJobsRedirect } from './backup-jobs/+page';
import { load as metricsRedirect } from './metrics/+page';
import { load as storageRedirect } from './storage/+page';
import { load as templatesRedirect } from './templates/+page';
import { load as nodeImagesRedirect } from './nodes/[id]/images/+page';
import { load as nodeNetworksRedirect } from './nodes/[id]/networks/+page';
import { load as nodeStorageRedirect } from './nodes/[id]/storage/+page';
import { load as nodeVmsRedirect } from './nodes/[id]/vms/+page';

async function expectRedirect(
	invoke: () => unknown,
	location: string
): Promise<void> {
	try {
		await invoke();
		throw new Error('Expected redirect');
	} catch (error) {
		expect(error).toMatchObject({ status: 307, location });
	}
}

describe('frontend-backend wiring smoke checks', () => {
	it('has route files for primary shell pages and high-traffic deep links', () => {
		const routeModules = Object.keys(
			import.meta.glob('/src/routes/**/+page.{ts,svelte}')
		);
		const expectedRouteFiles = [
			'/src/routes/+page.svelte',
			'/src/routes/clusters/+page.svelte',
			'/src/routes/clusters/[id]/+page.svelte',
			'/src/routes/nodes/+page.svelte',
			'/src/routes/nodes/[id]/+page.svelte',
			'/src/routes/vms/+page.svelte',
			'/src/routes/vms/[id]/+page.svelte',
			'/src/routes/volumes/+page.svelte',
			'/src/routes/volumes/[id]/+page.svelte',
			'/src/routes/networks/+page.svelte',
			'/src/routes/networks/[id]/+page.svelte',
			'/src/routes/images/+page.svelte',
			'/src/routes/tasks/+page.svelte',
			'/src/routes/events/+page.svelte',
			'/src/routes/maintenance/+page.svelte',
			'/src/routes/settings/+page.svelte'
		];

		for (const routeFile of expectedRouteFiles) {
			expect(routeModules).toContain(routeFile);
		}
	});

	it('does not contain literal brace interpolation inside href attributes', () => {
		const svelteSources = import.meta.glob('/src/**/*.svelte', {
			eager: true,
			query: '?raw',
			import: 'default'
		}) as Record<string, string>;
		const filesToScan = Object.entries(svelteSources).filter(([key]) =>
			key.startsWith('/src/routes/') ||
			key === '/src/lib/components/system/FilterPanel.svelte' ||
			key === '/src/lib/components/SkipLink.svelte'
		);
		const literalHrefInterpolation = /href=["'][^"'`]*\{[^"'`]*["']/;

		for (const [, content] of filesToScan) {
			expect(content).not.toMatch(literalHrefInterpolation);
		}
	});

	it('redirects legacy top-level routes to BFF-backed pages', async () => {
		await expectRedirect(() => storageRedirect({} as never), '/volumes');
		await expectRedirect(() => templatesRedirect({} as never), '/images');
		await expectRedirect(() => backupJobsRedirect({} as never), '/tasks');
		await expectRedirect(() => metricsRedirect({} as never), '/');
	});

	it('redirects legacy node subpages to BFF-backed node detail views', async () => {
		await expectRedirect(() => nodeVmsRedirect({ params: { id: 'node-1' } } as never), '/nodes/node-1?tab=vms');
		await expectRedirect(() => nodeNetworksRedirect({ params: { id: 'node-1' } } as never), '/nodes/node-1');
		await expectRedirect(() => nodeImagesRedirect({ params: { id: 'node-1' } } as never), '/nodes/node-1');
		await expectRedirect(() => nodeStorageRedirect({ params: { id: 'node-1' } } as never), '/nodes/node-1');
	});
});
