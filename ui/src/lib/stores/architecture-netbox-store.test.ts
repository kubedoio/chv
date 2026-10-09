import { beforeEach, describe, expect, it, vi } from 'vitest';

// The store imports mutation.svelte which transitively pulls in
// live-state.svelte (and SvelteKit's $app/navigation). Mirror the mocks
// already used by architecture-store.test.ts so this suite runs cleanly
// under jsdom.
vi.mock('$app/env/public', () => ({
	PUBLIC_CHV_API_BASE_URL: ''
}));

vi.mock('$app/navigation', () => ({
	goto: vi.fn(),
	refreshAll: vi.fn()
}));

vi.mock('#lib/api/client.ts', () => ({
	getStoredToken: vi.fn(() => 'test-token'),
	clearToken: vi.fn()
}));

vi.mock('#lib/bff/architectures.ts', async () => {
	const actual = await vi.importActual<typeof import('#lib/bff/architectures.ts')>(
		'#lib/bff/architectures.ts'
	);
	return {
		...actual,
		getNetboxConfig: vi.fn(),
		upsertNetboxConfig: vi.fn(),
		deleteNetboxConfig: vi.fn(),
		netboxDryRun: vi.fn(),
		exportNetbox: vi.fn(),
		listNetboxRuns: vi.fn(),
		getNetboxRun: vi.fn(),
		retryNetboxRun: vi.fn()
	};
});

import {
	getNetboxConfig,
	upsertNetboxConfig,
	deleteNetboxConfig,
	netboxDryRun,
	exportNetbox,
	listNetboxRuns,
	getNetboxRun,
	retryNetboxRun,
	StaleVersionError,
	type NetboxConfig,
	type NetboxRunDetail,
	type NetboxRunSummary
} from '#lib/bff/architectures.ts';
import { BFFError } from '#lib/bff/client.ts';
import { liveState } from './live-state.svelte';
import { architectureNetboxStore } from './architecture-netbox-store.svelte';
import { toast } from './toast.svelte';

const CONFIG: NetboxConfig = {
	architecture_id: 'arch-1',
	endpoint: 'https://netbox.example.internal',
	token_secret_ref: 'netbox-arch-1',
	token_set: true,
	retention_policy: 'mark_stale',
	enable_post_apply: true,
	custom_field_prefix: 'chv_',
	site_name: 'dc1',
	updated_at: '2026-10-08T09:00:00Z'
};

const PLAN = {
	mapping_version: 'v1',
	architecture_id: 'arch-1',
	architecture_version: 3,
	retention: 'mark_stale' as const,
	summary: { create: 1, update: 0, no_op: 0, conflict: 0, stale: 0 },
	entries: []
};

const RUN: NetboxRunSummary = {
	id: 'netrun-1',
	architecture_id: 'arch-1',
	trigger: 'manual',
	status: 'succeeded',
	mode: 'export',
	summary: { create: 1, update: 0, no_op: 0, conflict: 0, stale: 0 },
	error_message: null,
	attempt_count: 1,
	requested_by: 'user-1',
	started_at: '2026-10-08T09:01:00Z',
	finished_at: '2026-10-08T09:01:05Z',
	created_at: '2026-10-08T09:01:00Z'
};

const RUN_DETAIL: NetboxRunDetail = {
	id: 'netrun-1',
	architecture_id: 'arch-1',
	architecture_version_id: 'ver-3',
	resolved_architecture_version_id: 'ver-3',
	trigger: 'manual',
	status: 'succeeded',
	mode: 'export',
	plan_json: null,
	result_json: null,
	summary: null,
	error_message: null,
	attempt_count: 1,
	requested_by: 'user-1',
	started_at: null,
	finished_at: null,
	next_attempt_at: null,
	created_at: '2026-10-08T09:01:00Z'
};

describe('architectureNetboxStore', () => {
	beforeEach(() => {
		vi.restoreAllMocks();
		vi.spyOn(toast, 'success').mockImplementation(() => {});
		vi.spyOn(toast, 'error').mockImplementation(() => {});
		vi.spyOn(liveState, 'invalidateAndRefresh').mockResolvedValue(undefined);
		architectureNetboxStore.reset();
	});

	describe('loadConfig', () => {
		it('stores the config summary and clears the error on success', async () => {
			vi.mocked(getNetboxConfig).mockResolvedValue(CONFIG);

			const result = await architectureNetboxStore.loadConfig('arch-1');

			expect(result).toEqual(CONFIG);
			expect(architectureNetboxStore.config).toEqual(CONFIG);
			expect(architectureNetboxStore.configError).toBeNull();
			expect(architectureNetboxStore.configLoading).toBe(false);
		});

		it('treats NETBOX_NOT_CONFIGURED (null) as a normal absent-config state', async () => {
			vi.mocked(getNetboxConfig).mockResolvedValue(null);

			const result = await architectureNetboxStore.loadConfig('arch-1');

			expect(result).toBeNull();
			expect(architectureNetboxStore.config).toBeNull();
			expect(architectureNetboxStore.configError).toBeNull();
		});

		it('propagates transport failures into configError without clobbering config', async () => {
			architectureNetboxStore.state.config = CONFIG;
			vi.mocked(getNetboxConfig).mockRejectedValue(new BFFError('Forbidden', 403, 'FORBIDDEN'));

			const result = await architectureNetboxStore.loadConfig('arch-1');

			expect(result).toBeNull();
			expect(architectureNetboxStore.configError).toBe('Forbidden');
			// Prior config kept so the operator does not lose context.
			expect(architectureNetboxStore.config).toEqual(CONFIG);
		});
	});

	describe('saveConfig', () => {
		it('sends a flat upsert request with id + expected_version and stores the response', async () => {
			vi.mocked(upsertNetboxConfig).mockResolvedValue(CONFIG);

			const result = await architectureNetboxStore.saveConfig('arch-1', 4, {
				endpoint: 'https://netbox.example.internal',
				token: 'PAbCd123',
				token_secret_ref: 'netbox-arch-1',
				retention_policy: 'mark_stale',
				enable_post_apply: true,
				site_name: 'dc1'
			});

			expect(result).toEqual(CONFIG);
			expect(upsertNetboxConfig).toHaveBeenCalledWith(
				{
					id: 'arch-1',
					expected_version: 4,
					endpoint: 'https://netbox.example.internal',
					token: 'PAbCd123',
					token_secret_ref: 'netbox-arch-1',
					retention_policy: 'mark_stale',
					enable_post_apply: true,
					site_name: 'dc1'
				},
				'test-token'
			);
			expect(architectureNetboxStore.config).toEqual(CONFIG);
			expect(liveState.invalidateAndRefresh).toHaveBeenCalledWith(
				expect.objectContaining({ patterns: ['architectures:'], detailId: 'arch-1' })
			);
		});

		it('never holds the token in state after save (response is token_set-only)', async () => {
			vi.mocked(upsertNetboxConfig).mockResolvedValue(CONFIG);

			await architectureNetboxStore.saveConfig('arch-1', 4, {
				endpoint: 'https://netbox.example.internal',
				token: 'PAbCd123',
				token_secret_ref: 'netbox-arch-1',
				retention_policy: 'mark_stale',
				enable_post_apply: true,
				site_name: null
			});

			// The only token signal anywhere in the store's state is the
			// server's boolean.
			expect(JSON.stringify(architectureNetboxStore.state)).not.toContain('PAbCd123');
			expect(architectureNetboxStore.config?.token_set).toBe(true);
		});

		it('rethrows StaleVersionError on a 409 conflict and leaves state.config untouched', async () => {
			architectureNetboxStore.state.config = CONFIG;
			vi.mocked(upsertNetboxConfig).mockRejectedValue(
				new StaleVersionError('arch-1', 4, 'Stale architecture version', 'PLAN_EXPIRED')
			);

			await expect(
				architectureNetboxStore.saveConfig('arch-1', 4, {
					endpoint: 'https://netbox.example.internal',
					token_secret_ref: 'netbox-arch-1',
					retention_policy: 'mark_stale',
					enable_post_apply: true
				})
			).rejects.toBeInstanceOf(StaleVersionError);

			// The pre-save config stays; the form component's drafts are
			// local to the component, so the operator's typing survives.
			expect(architectureNetboxStore.config).toEqual(CONFIG);
		});
	});

	describe('deleteConfig', () => {
		it('clears config and the dry-run plan after a successful delete', async () => {
			architectureNetboxStore.state.config = CONFIG;
			architectureNetboxStore.state.dryRunPlan = PLAN;
			vi.mocked(deleteNetboxConfig).mockResolvedValue({ deleted: true });

			await architectureNetboxStore.deleteConfig('arch-1');

			expect(deleteNetboxConfig).toHaveBeenCalledWith('arch-1', 'test-token');
			expect(architectureNetboxStore.config).toBeNull();
			expect(architectureNetboxStore.dryRunPlan).toBeNull();
		});
	});

	describe('runDryRun', () => {
		it('stores the plan on success', async () => {
			vi.mocked(netboxDryRun).mockResolvedValue(PLAN);

			const result = await architectureNetboxStore.runDryRun('arch-1');

			expect(result).toEqual(PLAN);
			expect(architectureNetboxStore.dryRunPlan).toEqual(PLAN);
			expect(architectureNetboxStore.dryRunError).toBeNull();
		});

		it('keeps the previous plan and surfaces the code+message on failure', async () => {
			architectureNetboxStore.state.dryRunPlan = PLAN;
			vi.mocked(netboxDryRun).mockRejectedValue(
				new BFFError('Architecture has no succeeded apply run', 400, 'NETBOX_NOT_APPLIED')
			);

			const result = await architectureNetboxStore.runDryRun('arch-1');

			expect(result).toBeNull();
			// Code + message pair so the panel can branch its banner on
			// the contract's stable code.
			expect(architectureNetboxStore.dryRunError).toEqual({
				code: 'NETBOX_NOT_APPLIED',
				message: 'Architecture has no succeeded apply run'
			});
			expect(architectureNetboxStore.dryRunPlan).toEqual(PLAN);
			expect(architectureNetboxStore.dryRunLoading).toBe(false);
		});
	});

	describe('exportNow', () => {
		it('enqueues the export and refreshes the runs list', async () => {
			vi.mocked(exportNetbox).mockResolvedValue({
				run_id: 'netrun-2',
				architecture_id: 'arch-1',
				status: 'queued'
			});
			vi.mocked(listNetboxRuns).mockResolvedValue([{ ...RUN, id: 'netrun-2', status: 'queued' }]);

			await architectureNetboxStore.exportNow('arch-1');

			expect(exportNetbox).toHaveBeenCalledWith('arch-1', 'test-token');
			expect(listNetboxRuns).toHaveBeenCalledWith('arch-1', undefined, 'test-token');
			expect(architectureNetboxStore.runs[0]?.id).toBe('netrun-2');
			expect(architectureNetboxStore.exporting).toBe(false);
		});

		it('captures the BFFError code+message in exportError before rethrowing (NETBOX_RUN_ACTIVE)', async () => {
			vi.mocked(exportNetbox).mockRejectedValue(
				new BFFError('A projection run is already queued or running', 409, 'NETBOX_RUN_ACTIVE')
			);

			await expect(architectureNetboxStore.exportNow('arch-1')).rejects.toBeInstanceOf(BFFError);

			expect(architectureNetboxStore.exportError).toEqual({
				code: 'NETBOX_RUN_ACTIVE',
				message: 'A projection run is already queued or running'
			});
			expect(architectureNetboxStore.exporting).toBe(false);
		});

		it('clears exportError at the start of each attempt and on success', async () => {
			architectureNetboxStore.state.exportError = { code: 'NETBOX_RUN_ACTIVE', message: 'busy' };
			vi.mocked(exportNetbox).mockResolvedValue({
				run_id: 'netrun-2',
				architecture_id: 'arch-1',
				status: 'queued'
			});
			vi.mocked(listNetboxRuns).mockResolvedValue([RUN]);

			await architectureNetboxStore.exportNow('arch-1');

			expect(architectureNetboxStore.exportError).toBeNull();
		});

		it('drops the post-export runs refresh when the architecture switched mid-export (late-fetch guard)', async () => {
			// The export mutation stays pending until the test resolves
			// it, simulating the operator navigating away mid-await.
			let resolveExport!: (value: { run_id: string; architecture_id: string; status: string }) => void;
			vi.mocked(exportNetbox).mockImplementation(
				() => new Promise((resolve) => (resolveExport = resolve))
			);
			const RUN_A: NetboxRunSummary = { ...RUN, id: 'netrun-a', architecture_id: 'arch-a' };
			const RUN_B: NetboxRunSummary = { ...RUN, id: 'netrun-b', architecture_id: 'arch-b' };
			vi.mocked(listNetboxRuns).mockImplementation(async (id: string) =>
				id === 'arch-b' ? [RUN_B] : [RUN_A]
			);

			const pending = architectureNetboxStore.exportNow('arch-a');
			// Switch to architecture B while arch A's export is in flight.
			architectureNetboxStore.reset();
			await architectureNetboxStore.loadRuns('arch-b');
			expect(architectureNetboxStore.runs).toEqual([RUN_B]);

			resolveExport({ run_id: 'netrun-a', architecture_id: 'arch-a', status: 'queued' });
			await pending;

			// Arch A's post-mutation refresh must not re-pin the id nor
			// overwrite arch B's runs.
			expect(architectureNetboxStore.state.lastArchitectureId).toBe('arch-b');
			expect(architectureNetboxStore.runs).toEqual([RUN_B]);
			expect(listNetboxRuns).not.toHaveBeenCalledWith('arch-a', undefined, 'test-token');
		});
	});

	describe('loadRuns / loadRun', () => {
		it('stores the runs array and the selected run detail', async () => {
			vi.mocked(listNetboxRuns).mockResolvedValue([RUN]);
			vi.mocked(getNetboxRun).mockResolvedValue(RUN_DETAIL);

			const runs = await architectureNetboxStore.loadRuns('arch-1');
			const detail = await architectureNetboxStore.loadRun('arch-1', 'netrun-1');

			expect(runs).toEqual([RUN]);
			expect(architectureNetboxStore.runs).toEqual([RUN]);
			expect(detail).toEqual(RUN_DETAIL);
			expect(architectureNetboxStore.currentRun).toEqual(RUN_DETAIL);
		});

		it('surfaces list errors in runsError without clearing prior runs', async () => {
			architectureNetboxStore.state.runs = [RUN];
			vi.mocked(listNetboxRuns).mockRejectedValue(new BFFError('boom', 500, 'INTERNAL'));

			const runs = await architectureNetboxStore.loadRuns('arch-1');

			expect(runs).toEqual([]);
			expect(architectureNetboxStore.runsError).toBe('boom');
			expect(architectureNetboxStore.runs).toEqual([RUN]);
		});
	});

	describe('retryRun', () => {
		it('requeues and refreshes the runs list', async () => {
			architectureNetboxStore.state.runs = [{ ...RUN, status: 'failed' }];
			vi.mocked(retryNetboxRun).mockResolvedValue({ run_id: 'netrun-1', status: 'queued' });
			vi.mocked(listNetboxRuns).mockResolvedValue([{ ...RUN, status: 'queued' }]);

			await architectureNetboxStore.retryRun('arch-1', 'netrun-1');

			expect(retryNetboxRun).toHaveBeenCalledWith('arch-1', 'netrun-1', 'test-token');
			expect(architectureNetboxStore.runs[0]?.status).toBe('queued');
			expect(architectureNetboxStore.retrying).toBe(false);
		});

		it('captures PROJECTION_RUN_NOT_RETRYABLE in retryError before rethrowing (409 → banner path)', async () => {
			vi.mocked(retryNetboxRun).mockRejectedValue(
				new BFFError('Run is not retryable', 409, 'PROJECTION_RUN_NOT_RETRYABLE')
			);

			await expect(architectureNetboxStore.retryRun('arch-1', 'netrun-1')).rejects.toBeInstanceOf(
				BFFError
			);

			expect(architectureNetboxStore.retryError).toEqual({
				code: 'PROJECTION_RUN_NOT_RETRYABLE',
				message: 'Run is not retryable'
			});
			expect(architectureNetboxStore.retrying).toBe(false);
		});

		it('clears retryError at the start of each attempt and on success', async () => {
			architectureNetboxStore.state.retryError = {
				code: 'PROJECTION_RUN_NOT_RETRYABLE',
				message: 'not retryable'
			};
			vi.mocked(retryNetboxRun).mockResolvedValue({ run_id: 'netrun-1', status: 'queued' });
			vi.mocked(listNetboxRuns).mockResolvedValue([{ ...RUN, status: 'queued' }]);

			await architectureNetboxStore.retryRun('arch-1', 'netrun-1');

			expect(architectureNetboxStore.retryError).toBeNull();
		});

		it('drops the post-retry runs refresh when the architecture switched mid-retry (late-fetch guard)', async () => {
			let resolveRetry!: (value: { run_id: string; status: string }) => void;
			vi.mocked(retryNetboxRun).mockImplementation(
				() => new Promise((resolve) => (resolveRetry = resolve))
			);
			const RUN_A: NetboxRunSummary = { ...RUN, id: 'netrun-a', architecture_id: 'arch-a' };
			const RUN_B: NetboxRunSummary = { ...RUN, id: 'netrun-b', architecture_id: 'arch-b' };
			vi.mocked(listNetboxRuns).mockImplementation(async (id: string) =>
				id === 'arch-b' ? [RUN_B] : [RUN_A]
			);

			const pending = architectureNetboxStore.retryRun('arch-a', 'netrun-a');
			architectureNetboxStore.reset();
			await architectureNetboxStore.loadRuns('arch-b');
			expect(architectureNetboxStore.runs).toEqual([RUN_B]);

			resolveRetry({ run_id: 'netrun-a', status: 'queued' });
			await pending;

			expect(architectureNetboxStore.state.lastArchitectureId).toBe('arch-b');
			expect(architectureNetboxStore.runs).toEqual([RUN_B]);
			expect(listNetboxRuns).not.toHaveBeenCalledWith('arch-a', undefined, 'test-token');
		});
	});

	describe('reset', () => {
		it('returns the store to its initial state', async () => {
			vi.mocked(getNetboxConfig).mockResolvedValue(CONFIG);
			vi.mocked(listNetboxRuns).mockResolvedValue([RUN]);
			await architectureNetboxStore.loadConfig('arch-1');
			await architectureNetboxStore.loadRuns('arch-1');
			architectureNetboxStore.state.exportError = { code: 'NETBOX_RUN_ACTIVE', message: 'busy' };
			architectureNetboxStore.state.retryError = {
				code: 'PROJECTION_RUN_NOT_RETRYABLE',
				message: 'nope'
			};

			architectureNetboxStore.reset();

			expect(architectureNetboxStore.config).toBeNull();
			expect(architectureNetboxStore.runs).toEqual([]);
			expect(architectureNetboxStore.currentRun).toBeNull();
			expect(architectureNetboxStore.configLoading).toBe(false);
			expect(architectureNetboxStore.runsLoading).toBe(false);
			expect(architectureNetboxStore.exportError).toBeNull();
			expect(architectureNetboxStore.retryError).toBeNull();
		});
	});
});
