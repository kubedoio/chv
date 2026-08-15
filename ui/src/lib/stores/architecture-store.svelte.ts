import { getStoredToken } from '$lib/api/client';
import {
	createArchitecture,
	updateArchitecture,
	validateArchitecture,
	validateYaml,
	generateYaml,
	importYaml,
	checkFleet,
	plan,
	destroyPlan,
	discardPlan,
	StaleVersionError,
	type Architecture,
	type CreateArchitectureRequest,
	type FleetCheckResult,
	type PlanResult,
	type ValidationResult
} from '$lib/bff/architectures';
import type { MutateOpts } from './mutation.svelte';
import { mutateWithRefresh } from './mutation.svelte';

/**
 * Reactive Svelte 5 store for the Architecture Designer (Phase 0).
 *
 * Mutating methods MUST go through `mutateWithRefresh` so that:
 *   1. live-state cache invalidation runs after success;
 *   2. errors surface as toasts (the same UX every other mutating page uses);
 *   3. the codebase stays compliant with `mutation-compliance.test.ts`, which
 *      forbids manual `invalidateAll()` / `invalidatePattern()` calls in
 *      page components.
 *
 * Optimistic concurrency (Q3 of the Phase 0 plan): `update` forwards the
 * caller's `expected_version`. When the BFF returns 409 the underlying client
 * throws {@link StaleVersionError} — we let it propagate so pages can render
 * the StaleVersionBanner and offer a Reload action.
 */

export type {
	Architecture,
	ArchitectureSummary,
	ArchitectureDetail,
	FleetCheckResult,
	PlanResult,
	PlanChange,
	PlanSummary,
	PlanStatus,
	PlanMode,
	PlanAction,
	PlanRisk,
	ValidationResult,
	ValidationSummary,
	ValidationStatus,
	ValidationSeverity,
	Finding
} from '$lib/bff/architectures';
export { StaleVersionError } from '$lib/bff/architectures';

/**
 * Editable subset for `update()`. Field names mirror the wire (display_name,
 * not name) so callers don't have to translate. All fields are optional —
 * partial updates are valid and the server treats `undefined` as "leave alone"
 * and `null` as "clear".
 */
export interface ArchitectureEditableFields {
	display_name?: string | null;
	description?: string | null;
	environment?: string | null;
	design_graph_json?: string | null;
	latest_yaml?: string | null;
	latest_version_id?: string | null;
}

const REFRESH_PATTERNS = ['architectures:'];

class ArchitectureStore {
	async create(
		input: CreateArchitectureRequest,
		opts: MutateOpts<Architecture> = {}
	): Promise<Architecture> {
		const token = getStoredToken() ?? undefined;
		const result = await mutateWithRefresh<Architecture>(
			async () => {
				const res = await createArchitecture(input, token);
				return res.architecture;
			},
			{
				patterns: REFRESH_PATTERNS,
				successMessage: `Architecture "${input.name}" created`,
				errorMessage: 'Failed to create architecture',
				...opts
			}
		);
		return result;
	}

	async update(
		id: string,
		expectedVersion: number,
		fields: ArchitectureEditableFields,
		opts: MutateOpts<Architecture> = {}
	): Promise<Architecture> {
		const token = getStoredToken() ?? undefined;
		return mutateWithRefresh<Architecture>(
			async () => {
				// FLAT request shape: id + expected_version + the editable fields,
				// no `patch` wrapper. Matches the BFF wire (PR review B3).
				const res = await updateArchitecture(
					{ id, expected_version: expectedVersion, ...fields },
					token
				);
				return res.architecture;
			},
			{
				patterns: REFRESH_PATTERNS,
				detailId: id,
				successMessage: 'Architecture updated',
				errorMessage: 'Failed to update architecture',
				...opts
			}
		);
	}

	/**
	 * Run server-side validation on a saved topology.
	 *
	 * Goes through `mutateWithRefresh` because the server persists
	 * `last_validation_status` on the architecture row — the dashboard list
	 * must refresh so the status pill stays in sync. We deliberately leave
	 * `successMessage` undefined so re-validation does not spam toasts; the
	 * findings panel itself surfaces the outcome.
	 */
	async validate(
		id: string,
		opts: MutateOpts<ValidationResult> = {}
	): Promise<ValidationResult> {
		const token = getStoredToken() ?? undefined;
		return mutateWithRefresh<ValidationResult>(
			async () => validateArchitecture({ id }, token),
			{
				patterns: REFRESH_PATTERNS,
				detailId: id,
				errorMessage: 'Failed to validate architecture',
				...opts
			}
		);
	}

	/**
	 * Ad-hoc validation of a YAML string with no persistent target. Used by
	 * the Import dialog to preview findings before committing. Pure read-side
	 * — does NOT go through `mutateWithRefresh` because nothing persists.
	 */
	async validateYaml(yaml: string): Promise<ValidationResult> {
		const token = getStoredToken() ?? undefined;
		return validateYaml({ yaml }, token);
	}

	/**
	 * Fetch the canonical YAML serialisation of a saved topology.
	 *
	 * Read-only — bypasses `mutateWithRefresh`. Errors propagate so the
	 * caller (YAML side panel) can branch on `BFFError.code === 'GRAPH_EMPTY'`
	 * and render a helpful empty state instead of a generic toast.
	 */
	async generateYaml(id: string): Promise<string> {
		const token = getStoredToken() ?? undefined;
		const res = await generateYaml({ id }, token);
		return res.yaml;
	}

	/**
	 * Import a YAML blob into a topology. Goes through `mutateWithRefresh`
	 * because the server persists `latest_yaml` and `last_validation_status`
	 * — the dashboard list must refresh to reflect the new pill. The unwrapped
	 * ValidationResult is returned so the dialog can surface findings inline.
	 */
	async importYaml(
		id: string,
		yaml: string,
		opts: MutateOpts<ValidationResult> = {}
	): Promise<ValidationResult> {
		const token = getStoredToken() ?? undefined;
		return mutateWithRefresh<ValidationResult>(
			async () => {
				const res = await importYaml({ id, yaml }, token);
				return res.result;
			},
			{
				patterns: REFRESH_PATTERNS,
				detailId: id,
				successMessage: 'YAML imported',
				errorMessage: 'Failed to import YAML',
				...opts
			}
		);
	}

	/**
	 * Run Layer 2 fleet-consistency checks against a saved topology.
	 *
	 * Goes through `mutateWithRefresh` because the server persists
	 * `last_fleet_check_status` on the architecture row — the dashboard list
	 * must refresh so the per-row fleet pill stays in sync. We deliberately
	 * leave `successMessage` undefined; the FleetCheckPanel surfaces the
	 * outcome inline (status pill + finding list) so a toast on every refresh
	 * would just be noise.
	 */
	async checkFleet(
		id: string,
		opts: MutateOpts<FleetCheckResult> = {}
	): Promise<FleetCheckResult> {
		const token = getStoredToken() ?? undefined;
		return mutateWithRefresh<FleetCheckResult>(
			async () => checkFleet({ id }, token),
			{
				patterns: REFRESH_PATTERNS,
				detailId: id,
				errorMessage: 'Failed to run fleet check',
				...opts
			}
		);
	}

	/**
	 * Generate an apply-mode plan for a saved topology (Phase 4).
	 *
	 * Goes through `mutateWithRefresh` because the BFF persists
	 * `last_plan_status` on the architecture row — the dashboard list must
	 * refresh so the per-row plan pill stays in sync. We deliberately leave
	 * `successMessage` undefined: the PlanReviewPanel itself surfaces the
	 * outcome (status pill + summary chips + change list), so a toast on
	 * every regeneration would just be noise.
	 *
	 * `opts.allowWarnings` lets callers proceed past `warning`-status
	 * topologies; `opts.refreshInventory` forces a fresh inventory snapshot
	 * instead of reusing the cached one.
	 */
	async plan(
		id: string,
		opts: { allowWarnings?: boolean; refreshInventory?: boolean } & MutateOpts<PlanResult> = {}
	): Promise<PlanResult> {
		const token = getStoredToken() ?? undefined;
		const { allowWarnings, refreshInventory, ...mutateOpts } = opts;
		return mutateWithRefresh<PlanResult>(
			async () =>
				plan(
					{
						id,
						allow_warnings: allowWarnings,
						refresh_inventory: refreshInventory
					},
					token
				),
			{
				patterns: REFRESH_PATTERNS,
				detailId: id,
				errorMessage: 'Failed to generate plan',
				...mutateOpts
			}
		);
	}

	/**
	 * Generate a destroy-mode plan for a saved topology (Phase 4).
	 *
	 * Same refresh semantics as {@link plan}. Every change in a destroy plan
	 * carries `requires_confirmation: true`; the panel gates the apply button
	 * behind a typed-name confirmation field — we don't enforce that here
	 * because the store is ignorant of UI state.
	 */
	async destroyPlan(
		id: string,
		opts: MutateOpts<PlanResult> = {}
	): Promise<PlanResult> {
		const token = getStoredToken() ?? undefined;
		return mutateWithRefresh<PlanResult>(
			async () => destroyPlan({ id }, token),
			{
				patterns: REFRESH_PATTERNS,
				detailId: id,
				errorMessage: 'Failed to generate destroy plan',
				...opts
			}
		);
	}

	/**
	 * Explicitly discard a plan before its TTL expires (Phase 4).
	 *
	 * Idempotent server-side. Goes through `mutateWithRefresh` because the
	 * architecture's `last_plan_status` flips to `discarded` after success,
	 * and the dashboard pill must refresh.
	 */
	async discardPlan(
		planId: string,
		opts: MutateOpts<void> = {}
	): Promise<void> {
		const token = getStoredToken() ?? undefined;
		await mutateWithRefresh<void>(
			async () => {
				await discardPlan({ plan_id: planId }, token);
			},
			{
				patterns: REFRESH_PATTERNS,
				errorMessage: 'Failed to discard plan',
				...opts
			}
		);
	}
}

export const architectureStore = new ArchitectureStore();
