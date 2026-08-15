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
		createArchitecture: vi.fn(),
		updateArchitecture: vi.fn(),
		validateArchitecture: vi.fn(),
		validateYaml: vi.fn(),
		generateYaml: vi.fn(),
		importYaml: vi.fn(),
		checkFleet: vi.fn(),
		plan: vi.fn(),
		destroyPlan: vi.fn(),
		discardPlan: vi.fn()
	};
});

import {
	createArchitecture,
	updateArchitecture,
	validateArchitecture,
	validateYaml as validateYamlBff,
	generateYaml as generateYamlBff,
	importYaml as importYamlBff,
	checkFleet as checkFleetBff,
	plan as planBff,
	destroyPlan as destroyPlanBff,
	discardPlan as discardPlanBff,
	StaleVersionError,
	type Architecture,
	type ArchitectureSummary,
	type FleetCheckResult,
	type PlanResult,
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

describe('architectureStore', () => {
	beforeEach(() => {
		vi.restoreAllMocks();
		vi.spyOn(toast, 'success').mockImplementation(() => {});
		vi.spyOn(toast, 'error').mockImplementation(() => {});
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

	// ─── Phase 3: fleet check store method ──────────────────────────────────

	const FLEET_VALID: FleetCheckResult = {
		status: 'valid',
		inventory_snapshot_id: 'snap-1',
		checked_at: '2026-06-15T12:00:00Z',
		findings: []
	};

	const FLEET_INVALID: FleetCheckResult = {
		status: 'invalid',
		inventory_snapshot_id: 'snap-2',
		checked_at: '2026-06-15T12:01:00Z',
		findings: [
			{
				severity: 'error',
				code: 'INSUFFICIENT_MEMORY',
				message: 'host lacks 32GB RAM',
				path: 'instances[0]',
				resource_ref: 'instance/web',
				blocking: true,
				suggestion: 'reduce instance memory or pick a larger host'
			}
		]
	};

	describe('checkFleet', () => {
		it('calls checkFleet and refreshes the architectures: pattern with detailId', async () => {
			const refreshSpy = vi
				.spyOn(liveState, 'invalidateAndRefresh')
				.mockResolvedValue(undefined);
			vi.mocked(checkFleetBff).mockResolvedValue(FLEET_INVALID);

			const res = await architectureStore.checkFleet('arch-1');

			expect(res).toEqual(FLEET_INVALID);
			expect(checkFleetBff).toHaveBeenCalledWith({ id: 'arch-1' }, 'test-token');
			expect(refreshSpy).toHaveBeenCalledWith(
				expect.objectContaining({ patterns: ['architectures:'], detailId: 'arch-1' })
			);
		});

		it('rethrows BFF errors so the panel can surface them via the toast', async () => {
			vi.spyOn(liveState, 'invalidateAndRefresh').mockResolvedValue(undefined);
			const boom = new Error('snapshot failed');
			vi.mocked(checkFleetBff).mockRejectedValue(boom);

			await expect(architectureStore.checkFleet('arch-1')).rejects.toBe(boom);
		});

		it('forwards skipRefresh so silent re-checks do not invalidate the list', async () => {
			const refreshSpy = vi
				.spyOn(liveState, 'invalidateAndRefresh')
				.mockResolvedValue(undefined);
			vi.mocked(checkFleetBff).mockResolvedValue(FLEET_VALID);

			await architectureStore.checkFleet('arch-1', { skipRefresh: true });

			expect(refreshSpy).not.toHaveBeenCalled();
		});
	});

	// ─── Phase 4: plan generation store methods ─────────────────────────────

	const PLAN_READY: PlanResult = {
		plan_id: 'plan_01HX',
		architecture_id: 'arch-1',
		architecture_version: 1,
		architecture_version_id: 'ver_01HX',
		status: 'ready_to_apply',
		mode: 'apply',
		summary: { create: 1, update: 0, delete: 0, replace: 0, no_op: 0, warnings: 0 },
		changes: [
			{
				action: 'create',
				resource_type: 'network',
				resource_name: 'tenant-a',
				resource_ref: 'network/tenant-a',
				description: 'Create network tenant-a',
				risk: 'low',
				requires_confirmation: false
			}
		],
		warnings: [],
		expires_at: '2026-06-15T12:15:00Z',
		created_at: '2026-06-15T12:00:00Z'
	};

	const PLAN_DESTROY: PlanResult = {
		...PLAN_READY,
		plan_id: 'plan_01HY',
		status: 'requires_confirmation',
		mode: 'destroy',
		summary: { create: 0, update: 0, delete: 1, replace: 0, no_op: 0, warnings: 0 },
		changes: [
			{
				action: 'delete',
				resource_type: 'network',
				resource_name: 'tenant-a',
				resource_ref: 'network/tenant-a',
				description: 'Delete network tenant-a',
				risk: 'destructive',
				requires_confirmation: true
			}
		]
	};

	describe('plan', () => {
		it('calls plan() and refreshes the architectures: pattern with detailId', async () => {
			const refreshSpy = vi
				.spyOn(liveState, 'invalidateAndRefresh')
				.mockResolvedValue(undefined);
			vi.mocked(planBff).mockResolvedValue(PLAN_READY);

			const res = await architectureStore.plan('arch-1');

			expect(res).toEqual(PLAN_READY);
			expect(planBff).toHaveBeenCalledWith(
				{ id: 'arch-1', allow_warnings: undefined, refresh_inventory: undefined },
				'test-token'
			);
			expect(refreshSpy).toHaveBeenCalledWith(
				expect.objectContaining({ patterns: ['architectures:'], detailId: 'arch-1' })
			);
		});

		it('forwards allowWarnings and refreshInventory into the BFF body', async () => {
			vi.spyOn(liveState, 'invalidateAndRefresh').mockResolvedValue(undefined);
			vi.mocked(planBff).mockResolvedValue(PLAN_READY);

			await architectureStore.plan('arch-1', {
				allowWarnings: true,
				refreshInventory: true
			});

			expect(planBff).toHaveBeenCalledWith(
				{ id: 'arch-1', allow_warnings: true, refresh_inventory: true },
				'test-token'
			);
		});

		it('rethrows BFF errors so the panel can surface them via the toast', async () => {
			vi.spyOn(liveState, 'invalidateAndRefresh').mockResolvedValue(undefined);
			const boom = new Error('plan failed');
			vi.mocked(planBff).mockRejectedValue(boom);

			await expect(architectureStore.plan('arch-1')).rejects.toBe(boom);
		});
	});

	describe('destroyPlan', () => {
		it('calls destroyPlan() and refreshes the architectures: pattern with detailId', async () => {
			const refreshSpy = vi
				.spyOn(liveState, 'invalidateAndRefresh')
				.mockResolvedValue(undefined);
			vi.mocked(destroyPlanBff).mockResolvedValue(PLAN_DESTROY);

			const res = await architectureStore.destroyPlan('arch-1');

			expect(res).toEqual(PLAN_DESTROY);
			expect(destroyPlanBff).toHaveBeenCalledWith({ id: 'arch-1' }, 'test-token');
			expect(refreshSpy).toHaveBeenCalledWith(
				expect.objectContaining({ patterns: ['architectures:'], detailId: 'arch-1' })
			);
		});

		it('rethrows BFF errors verbatim', async () => {
			vi.spyOn(liveState, 'invalidateAndRefresh').mockResolvedValue(undefined);
			const boom = new Error('destroy plan failed');
			vi.mocked(destroyPlanBff).mockRejectedValue(boom);

			await expect(architectureStore.destroyPlan('arch-1')).rejects.toBe(boom);
		});
	});

	describe('discardPlan', () => {
		it('calls discardPlan() and refreshes the architectures: pattern', async () => {
			const refreshSpy = vi
				.spyOn(liveState, 'invalidateAndRefresh')
				.mockResolvedValue(undefined);
			vi.mocked(discardPlanBff).mockResolvedValue({ status: 'discarded' });

			await architectureStore.discardPlan('plan_01HX');

			expect(discardPlanBff).toHaveBeenCalledWith({ plan_id: 'plan_01HX' }, 'test-token');
			expect(refreshSpy).toHaveBeenCalledWith(
				expect.objectContaining({ patterns: ['architectures:'] })
			);
		});

		it('rethrows BFF errors so the panel keeps the prior plan visible', async () => {
			vi.spyOn(liveState, 'invalidateAndRefresh').mockResolvedValue(undefined);
			const boom = new Error('discard failed');
			vi.mocked(discardPlanBff).mockRejectedValue(boom);

			await expect(architectureStore.discardPlan('plan_01HX')).rejects.toBe(boom);
		});
	});
});
