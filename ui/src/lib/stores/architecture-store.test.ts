import { beforeEach, describe, expect, it, vi } from 'vitest';

// architecture-store imports mutation.svelte which transitively pulls in
// live-state.svelte (and SvelteKit's $app/navigation). Mirror the mocks
// already used by mutation.test.ts so this suite runs cleanly under jsdom.
vi.mock('$env/dynamic/public', () => ({
	env: {}
}));

vi.mock('$app/navigation', () => ({
	goto: vi.fn(),
	invalidateAll: vi.fn()
}));

vi.mock('$lib/api/client', () => ({
	getStoredToken: vi.fn(() => 'test-token'),
	clearToken: vi.fn()
}));

vi.mock('$lib/bff/architectures', async () => {
	const actual = await vi.importActual<typeof import('$lib/bff/architectures')>(
		'$lib/bff/architectures'
	);
	return {
		...actual,
		listArchitectures: vi.fn(),
		getArchitecture: vi.fn(),
		createArchitecture: vi.fn(),
		updateArchitecture: vi.fn(),
		archiveArchitecture: vi.fn()
	};
});

import {
	listArchitectures,
	getArchitecture,
	createArchitecture,
	updateArchitecture,
	archiveArchitecture,
	StaleVersionError,
	type Architecture,
	type ArchitectureSummary
} from '$lib/bff/architectures';
import { liveState } from './live-state.svelte';
import { architectureStore } from './architecture-store.svelte';
import { toast } from './toast.svelte';

const ARCH: Architecture = {
	id: 'arch-1',
	name: 'phase-0-test',
	description: 'smoke',
	environment: 'development',
	status: 'draft',
	version_number: 1,
	created_at: '2026-06-13T00:00:00Z',
	updated_at: '2026-06-13T00:00:00Z'
};

const SUMMARY: ArchitectureSummary = {
	id: ARCH.id,
	name: ARCH.name,
	description: ARCH.description,
	environment: ARCH.environment,
	status: ARCH.status,
	version_number: ARCH.version_number,
	created_at: ARCH.created_at,
	updated_at: ARCH.updated_at
};

describe('architectureStore', () => {
	beforeEach(() => {
		vi.restoreAllMocks();
		vi.spyOn(toast, 'success').mockImplementation(() => {});
		vi.spyOn(toast, 'error').mockImplementation(() => {});
		// architectureStore is a module-level singleton; reset reactive state
		// between tests so each starts from a known baseline.
		architectureStore.items = [];
		architectureStore.error = null;
		architectureStore.loading = false;
	});

	describe('list', () => {
		it('loads items and returns them', async () => {
			vi.mocked(listArchitectures).mockResolvedValue({
				items: [SUMMARY],
				page: { page: 1, page_size: 50, total_items: 1 }
			});

			const items = await architectureStore.list();

			expect(items).toEqual([SUMMARY]);
			expect(architectureStore.items).toEqual([SUMMARY]);
			expect(architectureStore.error).toBeNull();
			expect(architectureStore.loading).toBe(false);
		});

		it('passes the stored token through to the BFF client', async () => {
			vi.mocked(listArchitectures).mockResolvedValue({
				items: [],
				page: { page: 1, page_size: 50, total_items: 0 }
			});

			await architectureStore.list();

			expect(listArchitectures).toHaveBeenCalledWith({}, 'test-token');
		});

		it('records error message and rethrows on failure', async () => {
			const boom = new Error('list failed');
			vi.mocked(listArchitectures).mockRejectedValue(boom);

			await expect(architectureStore.list()).rejects.toBe(boom);
			expect(architectureStore.error).toBe('list failed');
			expect(architectureStore.loading).toBe(false);
		});
	});

	describe('get', () => {
		it('returns the architecture from the BFF', async () => {
			vi.mocked(getArchitecture).mockResolvedValue({ architecture: ARCH });

			const result = await architectureStore.get('arch-1');

			expect(result).toEqual(ARCH);
			expect(getArchitecture).toHaveBeenCalledWith({ id: 'arch-1' }, 'test-token');
		});

		it('rethrows BFF errors so the loader can branch on them', async () => {
			const boom = new Error('not found');
			vi.mocked(getArchitecture).mockRejectedValue(boom);

			await expect(architectureStore.get('arch-1')).rejects.toBe(boom);
		});
	});

	describe('create', () => {
		it('invokes mutateWithRefresh with architectures: pattern and returns the architecture', async () => {
			const refreshSpy = vi
				.spyOn(liveState, 'invalidateAndRefresh')
				.mockResolvedValue(undefined);
			vi.mocked(createArchitecture).mockResolvedValue({ architecture: ARCH });

			const result = await architectureStore.create({
				name: 'phase-0-test',
				description: 'smoke',
				environment: 'development'
			});

			expect(result).toEqual(ARCH);
			expect(createArchitecture).toHaveBeenCalledWith(
				{ name: 'phase-0-test', description: 'smoke', environment: 'development' },
				'test-token'
			);
			expect(refreshSpy).toHaveBeenCalledTimes(1);
			expect(refreshSpy).toHaveBeenCalledWith(
				expect.objectContaining({ patterns: ['architectures:'] })
			);
		});

		it('forwards caller-supplied opts (sidebar/skipRefresh) to mutateWithRefresh', async () => {
			const refreshSpy = vi
				.spyOn(liveState, 'invalidateAndRefresh')
				.mockResolvedValue(undefined);
			vi.mocked(createArchitecture).mockResolvedValue({ architecture: ARCH });

			await architectureStore.create(
				{ name: 'phase-0-test', environment: 'development' },
				{ sidebar: false, skipRefresh: true }
			);

			// skipRefresh: true must short-circuit the refresh call entirely.
			expect(refreshSpy).not.toHaveBeenCalled();
		});

		it('rethrows when the BFF call rejects', async () => {
			vi.spyOn(liveState, 'invalidateAndRefresh').mockResolvedValue(undefined);
			const boom = new Error('create failed');
			vi.mocked(createArchitecture).mockRejectedValue(boom);

			await expect(
				architectureStore.create({ name: 'x', environment: 'development' })
			).rejects.toBe(boom);
		});
	});

	describe('update', () => {
		it('calls updateArchitecture with id, expected_version and patch', async () => {
			vi.spyOn(liveState, 'invalidateAndRefresh').mockResolvedValue(undefined);
			vi.mocked(updateArchitecture).mockResolvedValue({
				architecture: { ...ARCH, name: 'renamed', version_number: 2 }
			});

			const result = await architectureStore.update('arch-1', 1, { name: 'renamed' });

			expect(updateArchitecture).toHaveBeenCalledWith(
				{ id: 'arch-1', expected_version: 1, patch: { name: 'renamed' } },
				'test-token'
			);
			expect(result.name).toBe('renamed');
			expect(result.version_number).toBe(2);
		});

		it('forwards detailId so per-resource cache key is invalidated', async () => {
			const refreshSpy = vi
				.spyOn(liveState, 'invalidateAndRefresh')
				.mockResolvedValue(undefined);
			vi.mocked(updateArchitecture).mockResolvedValue({ architecture: ARCH });

			await architectureStore.update('arch-1', 1, { description: 'edited' });

			expect(refreshSpy).toHaveBeenCalledWith(
				expect.objectContaining({ patterns: ['architectures:'], detailId: 'arch-1' })
			);
		});

		it('propagates StaleVersionError on 409', async () => {
			vi.spyOn(liveState, 'invalidateAndRefresh').mockResolvedValue(undefined);
			const stale = new StaleVersionError('arch-1', 1, 'stale', 'STALE_VERSION');
			vi.mocked(updateArchitecture).mockRejectedValue(stale);

			await expect(
				architectureStore.update('arch-1', 1, { name: 'renamed' })
			).rejects.toBeInstanceOf(StaleVersionError);
		});
	});

	describe('archive', () => {
		it('calls archiveArchitecture with id and expected_version', async () => {
			vi.spyOn(liveState, 'invalidateAndRefresh').mockResolvedValue(undefined);
			const archived: Architecture = { ...ARCH, status: 'archived', version_number: 2 };
			vi.mocked(archiveArchitecture).mockResolvedValue({ architecture: archived });

			const result = await architectureStore.archive('arch-1', 1);

			expect(archiveArchitecture).toHaveBeenCalledWith(
				{ id: 'arch-1', expected_version: 1 },
				'test-token'
			);
			expect(result.status).toBe('archived');
		});

		it('propagates StaleVersionError on 409', async () => {
			vi.spyOn(liveState, 'invalidateAndRefresh').mockResolvedValue(undefined);
			const stale = new StaleVersionError('arch-1', 1);
			vi.mocked(archiveArchitecture).mockRejectedValue(stale);

			await expect(architectureStore.archive('arch-1', 1)).rejects.toBeInstanceOf(
				StaleVersionError
			);
		});
	});
});
