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
		archiveArchitecture: vi.fn(),
		validateArchitecture: vi.fn(),
		validateYaml: vi.fn(),
		generateYaml: vi.fn(),
		importYaml: vi.fn()
	};
});

import {
	listArchitectures,
	getArchitecture,
	createArchitecture,
	updateArchitecture,
	archiveArchitecture,
	validateArchitecture,
	validateYaml as validateYamlBff,
	generateYaml as generateYamlBff,
	importYaml as importYamlBff,
	StaleVersionError,
	type Architecture,
	type ArchitectureDetail,
	type ArchitectureSummary,
	type ValidationResult
} from '$lib/bff/architectures';
import { liveState } from './live-state.svelte';
import { architectureStore } from './architecture-store.svelte';
import { toast } from './toast.svelte';

const SUMMARY: ArchitectureSummary = {
	id: 'arch-1',
	name: 'phase-0-test',
	display_name: 'Phase 0 Test',
	description: 'smoke',
	environment: 'development',
	status: 'draft',
	owner_user_id: null,
	last_validation_status: null,
	last_fleet_check_status: null,
	version_number: 1,
	created_at: '2026-06-13T00:00:00Z',
	updated_at: '2026-06-13T00:00:00Z',
	archived_at: null
};

const ARCH: Architecture = SUMMARY;

const DETAIL: ArchitectureDetail = {
	architecture: SUMMARY,
	design_graph_json: null,
	latest_yaml: null
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
		it('loads items from `architectures` (new wire shape) and returns them', async () => {
			vi.mocked(listArchitectures).mockResolvedValue({ architectures: [SUMMARY] });

			const items = await architectureStore.list();

			expect(items).toEqual([SUMMARY]);
			expect(architectureStore.items).toEqual([SUMMARY]);
			expect(architectureStore.error).toBeNull();
			expect(architectureStore.loading).toBe(false);
		});

		it('passes the stored token through to the BFF client', async () => {
			vi.mocked(listArchitectures).mockResolvedValue({ architectures: [] });

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
		it('returns the architecture detail from the BFF', async () => {
			vi.mocked(getArchitecture).mockResolvedValue(DETAIL);

			const result = await architectureStore.get('arch-1');

			expect(result).toEqual(DETAIL);
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
		it('calls updateArchitecture with a FLAT body — id, expected_version and the editable fields', async () => {
			vi.spyOn(liveState, 'invalidateAndRefresh').mockResolvedValue(undefined);
			vi.mocked(updateArchitecture).mockResolvedValue({
				architecture: { ...ARCH, display_name: 'renamed', version_number: 2 }
			});

			const result = await architectureStore.update('arch-1', 1, {
				display_name: 'renamed',
				description: 'edited',
				environment: 'staging'
			});

			expect(updateArchitecture).toHaveBeenCalledWith(
				{
					id: 'arch-1',
					expected_version: 1,
					display_name: 'renamed',
					description: 'edited',
					environment: 'staging'
				},
				'test-token'
			);
			expect(result.display_name).toBe('renamed');
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

		it('accepts an empty fields object (partial-patch is valid even with no changes)', async () => {
			vi.spyOn(liveState, 'invalidateAndRefresh').mockResolvedValue(undefined);
			vi.mocked(updateArchitecture).mockResolvedValue({ architecture: ARCH });

			await architectureStore.update('arch-1', 1, {});

			expect(updateArchitecture).toHaveBeenCalledWith(
				{ id: 'arch-1', expected_version: 1 },
				'test-token'
			);
		});

		it('propagates StaleVersionError on 409', async () => {
			vi.spyOn(liveState, 'invalidateAndRefresh').mockResolvedValue(undefined);
			const stale = new StaleVersionError('arch-1', 1, 'stale', 'STALE_VERSION');
			vi.mocked(updateArchitecture).mockRejectedValue(stale);

			await expect(
				architectureStore.update('arch-1', 1, { display_name: 'renamed' })
			).rejects.toBeInstanceOf(StaleVersionError);
		});
	});

	describe('archive', () => {
		it('calls archiveArchitecture with id and expected_version and returns the archived architecture', async () => {
			vi.spyOn(liveState, 'invalidateAndRefresh').mockResolvedValue(undefined);
			const archived: Architecture = {
				...ARCH,
				status: 'archived',
				version_number: 2,
				archived_at: '2026-06-13T01:00:00Z'
			};
			vi.mocked(archiveArchitecture).mockResolvedValue({ architecture: archived });

			const result = await architectureStore.archive('arch-1', 1);

			expect(archiveArchitecture).toHaveBeenCalledWith(
				{ id: 'arch-1', expected_version: 1 },
				'test-token'
			);
			expect(result).toEqual(archived);
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

	// ─── Phase 1: validation + YAML store methods ──────────────────────────

	const VALID_RESULT: ValidationResult = {
		status: 'valid',
		summary: { errors: 0, warnings: 0, info: 0 },
		findings: []
	};

	const INVALID_RESULT: ValidationResult = {
		status: 'invalid',
		summary: { errors: 1, warnings: 0, info: 0 },
		findings: [
			{
				severity: 'error',
				code: 'INVALID_CIDR',
				message: 'CIDR is not parseable',
				path: 'networks[0].cidr',
				resource_ref: 'networks/lan',
				blocking: true,
				suggestion: 'Use 10.0.0.0/24'
			}
		]
	};

	describe('validate', () => {
		it('calls validateArchitecture and refreshes the architectures: pattern', async () => {
			const refreshSpy = vi
				.spyOn(liveState, 'invalidateAndRefresh')
				.mockResolvedValue(undefined);
			vi.mocked(validateArchitecture).mockResolvedValue(INVALID_RESULT);

			const res = await architectureStore.validate('arch-1');

			expect(res).toEqual(INVALID_RESULT);
			expect(validateArchitecture).toHaveBeenCalledWith({ id: 'arch-1' }, 'test-token');
			expect(refreshSpy).toHaveBeenCalledWith(
				expect.objectContaining({ patterns: ['architectures:'], detailId: 'arch-1' })
			);
		});

		it('rethrows BFF errors so callers can branch on them', async () => {
			vi.spyOn(liveState, 'invalidateAndRefresh').mockResolvedValue(undefined);
			const boom = new Error('validate failed');
			vi.mocked(validateArchitecture).mockRejectedValue(boom);

			await expect(architectureStore.validate('arch-1')).rejects.toBe(boom);
		});

		it('forwards skipRefresh so silent re-validation does not invalidate the list', async () => {
			const refreshSpy = vi
				.spyOn(liveState, 'invalidateAndRefresh')
				.mockResolvedValue(undefined);
			vi.mocked(validateArchitecture).mockResolvedValue(VALID_RESULT);

			await architectureStore.validate('arch-1', { skipRefresh: true });

			expect(refreshSpy).not.toHaveBeenCalled();
		});
	});

	describe('validateYaml', () => {
		it('calls the BFF wrapper directly and does NOT trigger a refresh', async () => {
			const refreshSpy = vi
				.spyOn(liveState, 'invalidateAndRefresh')
				.mockResolvedValue(undefined);
			vi.mocked(validateYamlBff).mockResolvedValue(VALID_RESULT);

			const res = await architectureStore.validateYaml('kind: Topology\n');

			expect(res).toEqual(VALID_RESULT);
			expect(validateYamlBff).toHaveBeenCalledWith(
				{ yaml: 'kind: Topology\n' },
				'test-token'
			);
			expect(refreshSpy).not.toHaveBeenCalled();
		});

		it('rethrows BFF errors verbatim', async () => {
			const boom = new Error('boom');
			vi.mocked(validateYamlBff).mockRejectedValue(boom);

			await expect(architectureStore.validateYaml('bad')).rejects.toBe(boom);
		});
	});

	describe('generateYaml', () => {
		it('returns the yaml string and does NOT trigger a refresh', async () => {
			const refreshSpy = vi
				.spyOn(liveState, 'invalidateAndRefresh')
				.mockResolvedValue(undefined);
			vi.mocked(generateYamlBff).mockResolvedValue({ yaml: 'kind: Topology\n' });

			const yaml = await architectureStore.generateYaml('arch-1');

			expect(yaml).toBe('kind: Topology\n');
			expect(generateYamlBff).toHaveBeenCalledWith({ id: 'arch-1' }, 'test-token');
			expect(refreshSpy).not.toHaveBeenCalled();
		});
	});

	describe('importYaml', () => {
		it('runs through mutateWithRefresh and unwraps the ValidationResult', async () => {
			const refreshSpy = vi
				.spyOn(liveState, 'invalidateAndRefresh')
				.mockResolvedValue(undefined);
			vi.mocked(importYamlBff).mockResolvedValue({ result: VALID_RESULT });

			const res = await architectureStore.importYaml('arch-1', 'kind: Topology\n');

			expect(res).toEqual(VALID_RESULT);
			expect(importYamlBff).toHaveBeenCalledWith(
				{ id: 'arch-1', yaml: 'kind: Topology\n' },
				'test-token'
			);
			expect(refreshSpy).toHaveBeenCalledWith(
				expect.objectContaining({ patterns: ['architectures:'], detailId: 'arch-1' })
			);
		});

		it('rethrows when the BFF call rejects so the dialog can stay open', async () => {
			vi.spyOn(liveState, 'invalidateAndRefresh').mockResolvedValue(undefined);
			const boom = new Error('import failed');
			vi.mocked(importYamlBff).mockRejectedValue(boom);

			await expect(
				architectureStore.importYaml('arch-1', 'oops')
			).rejects.toBe(boom);
		});
	});
});
