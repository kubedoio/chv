import { getStoredToken } from '$lib/api/client';
import {
	listArchitectures,
	getArchitecture,
	createArchitecture,
	updateArchitecture,
	archiveArchitecture,
	StaleVersionError,
	type Architecture,
	type ArchitectureSummary,
	type CreateArchitectureRequest,
	type UpdateArchitecturePatch
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
 * Optimistic concurrency (Q3 of the Phase 0 plan): `update` and `archive`
 * forward the caller's `expected_version`. When the BFF returns 409 the
 * underlying client throws {@link StaleVersionError} — we let it propagate so
 * pages can render the StaleVersionBanner and offer a Reload action.
 */

export type { Architecture, ArchitectureSummary } from '$lib/bff/architectures';
export { StaleVersionError } from '$lib/bff/architectures';

const REFRESH_PATTERNS = ['architectures:'];

class ArchitectureStore {
	items = $state<ArchitectureSummary[]>([]);
	loading = $state(false);
	error = $state<string | null>(null);

	async list(): Promise<ArchitectureSummary[]> {
		this.loading = true;
		this.error = null;
		try {
			const token = getStoredToken() ?? undefined;
			const res = await listArchitectures({}, token);
			this.items = res.items ?? [];
			return this.items;
		} catch (err) {
			this.error = err instanceof Error ? err.message : 'Failed to load architectures';
			throw err;
		} finally {
			this.loading = false;
		}
	}

	async get(id: string): Promise<Architecture> {
		const token = getStoredToken() ?? undefined;
		const res = await getArchitecture({ id }, token);
		return res.architecture;
	}

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
		patch: UpdateArchitecturePatch,
		opts: MutateOpts<Architecture> = {}
	): Promise<Architecture> {
		const token = getStoredToken() ?? undefined;
		return mutateWithRefresh<Architecture>(
			async () => {
				const res = await updateArchitecture(
					{ id, expected_version: expectedVersion, patch },
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

	async archive(
		id: string,
		expectedVersion: number,
		opts: MutateOpts<Architecture> = {}
	): Promise<Architecture> {
		const token = getStoredToken() ?? undefined;
		return mutateWithRefresh<Architecture>(
			async () => {
				const res = await archiveArchitecture({ id, expected_version: expectedVersion }, token);
				return res.architecture;
			},
			{
				patterns: REFRESH_PATTERNS,
				detailId: id,
				successMessage: 'Architecture archived',
				errorMessage: 'Failed to archive architecture',
				...opts
			}
		);
	}
}

export const architectureStore = new ArchitectureStore();
