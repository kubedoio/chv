import { getStoredToken } from '#lib/api/client.ts';
import {
	deleteNetboxConfig as bffDeleteConfig,
	exportNetbox as bffExportNetbox,
	getNetboxConfig,
	getNetboxRun,
	listNetboxRuns,
	netboxDryRun,
	retryNetboxRun as bffRetryRun,
	upsertNetboxConfig,
	type NetboxConfig,
	type NetboxConfigUpsertRequest,
	type NetboxProjectionPlan,
	type NetboxRunDetail,
	type NetboxRunSummary
} from '#lib/bff/architectures.ts';
import { BFFError } from '#lib/bff/client.ts';
import { mutateWithRefresh } from './mutation.svelte';

/**
 * Reactive Svelte 5 runes-based store for the NetBox projection panel
 * (issue #239, PR 7 of 8).
 *
 * Responsibilities
 * - Hold the architecture's projection config (or the knowledge that none
 *   exists — `getNetboxConfig`'s null-on-`NETBOX_NOT_CONFIGURED` pattern),
 *   the latest dry-run plan, the runs list, and the selected run detail;
 * - Expose one action per BFF endpoint plus `reset()`, mirroring the
 *   drift/runs stores' shape (per-section loading/error flags, a
 *   `lastArchitectureId` guard that drops late-arriving fetches after an
 *   architecture switch, reset-on-navigation).
 *
 * Mutations (`saveConfig`, `deleteConfig`, `exportNow`, `retryRun`) go
 * through `mutateWithRefresh` with the `architectures:` pattern per the
 * repo-wide mutation rule. Reads (`loadConfig`, `runDryRun`, `loadRuns`,
 * `loadRun`) call the BFF directly — a dry-run writes nothing anywhere
 * (the contract pins it), so there is no dashboard state to invalidate.
 *
 * Secrets
 * - The NetBox token is write-only and transient: `saveConfig` receives
 *   the form's draft (which may carry `token`), forwards it once to the
 *   BFF, and never copies it into `state` — the stored config is the
 *   server's `token_set`-only summary. The draft itself lives in the
 *   form component and is cleared after dispatch.
 *
 * Conflict semantics
 * - `saveConfig` rethrows `StaleVersionError` (409 `PLAN_EXPIRED`) so the
 *   panel can raise the page's stale-version banner. `state.config` stays
 *   untouched on failure, and the form component keeps its local drafts —
 *   the same "conflict leaves typing intact" contract
 *   ArchitectureMetaPanel documents.
 */

const REFRESH_PATTERNS = ['architectures:'];

/**
 * A BFF error's code+message pair, captured in store state so the panel
 * can branch its inline banners on the contract's stable codes (the
 * original error is still rethrown after `mutateWithRefresh` has
 * toasted it — the banner is the persistent, code-specific surface).
 */
export interface NetboxActionError {
	code: string;
	message: string;
}

/** Normalize a caught error into a {@link NetboxActionError}. */
function toActionError(err: unknown, fallback: string): NetboxActionError {
	if (err instanceof BFFError) {
		return { code: err.code, message: err.message };
	}
	if (err instanceof Error) {
		return { code: 'UNKNOWN', message: err.message };
	}
	return { code: 'UNKNOWN', message: fallback };
}

/**
 * The form-side half of {@link NetboxConfigUpsertRequest} — everything
 * except `id` / `expected_version`, which the panel supplies from the
 * loaded architecture row.
 */
export type NetboxConfigDraft = Omit<NetboxConfigUpsertRequest, 'id' | 'expected_version'>;

interface NetboxState {
	config: NetboxConfig | null;
	configLoading: boolean;
	configError: string | null;
	dryRunPlan: NetboxProjectionPlan | null;
	dryRunLoading: boolean;
	dryRunError: NetboxActionError | null;
	runs: NetboxRunSummary[];
	runsLoading: boolean;
	runsError: string | null;
	/** Code+message of the last failed export attempt (banner source). */
	exportError: NetboxActionError | null;
	/** Code+message of the last failed retry attempt (banner source). */
	retryError: NetboxActionError | null;
	currentRun: NetboxRunDetail | null;
	runLoading: boolean;
	runError: string | null;
	exporting: boolean;
	retrying: boolean;
	lastArchitectureId: string | null;
}

class ArchitectureNetboxStore {
	state = $state<NetboxState>({
		config: null,
		configLoading: false,
		configError: null,
		dryRunPlan: null,
		dryRunLoading: false,
		dryRunError: null,
		runs: [],
		runsLoading: false,
		runsError: null,
		exportError: null,
		retryError: null,
		currentRun: null,
		runLoading: false,
		runError: null,
		exporting: false,
		retrying: false,
		lastArchitectureId: null
	});

	get config(): NetboxConfig | null {
		return this.state.config;
	}

	get configLoading(): boolean {
		return this.state.configLoading;
	}

	get configError(): string | null {
		return this.state.configError;
	}

	get dryRunPlan(): NetboxProjectionPlan | null {
		return this.state.dryRunPlan;
	}

	get dryRunLoading(): boolean {
		return this.state.dryRunLoading;
	}

	get dryRunError(): NetboxActionError | null {
		return this.state.dryRunError;
	}

	get runs(): NetboxRunSummary[] {
		return this.state.runs;
	}

	get runsLoading(): boolean {
		return this.state.runsLoading;
	}

	get runsError(): string | null {
		return this.state.runsError;
	}

	get currentRun(): NetboxRunDetail | null {
		return this.state.currentRun;
	}

	get runLoading(): boolean {
		return this.state.runLoading;
	}

	get runError(): string | null {
		return this.state.runError;
	}

	get exporting(): boolean {
		return this.state.exporting;
	}

	get exportError(): NetboxActionError | null {
		return this.state.exportError;
	}

	get retrying(): boolean {
		return this.state.retrying;
	}

	get retryError(): NetboxActionError | null {
		return this.state.retryError;
	}

	/**
	 * Late-fetch guard, mirroring the drift store: results are dropped
	 * when the store has switched architectures while a call was in
	 * flight. Callers SHOULD still `reset()` between architectures.
	 */
	private isCurrent(id: string): boolean {
		return this.state.lastArchitectureId === id;
	}

	/**
	 * Load the projection config. A missing config is a normal state, not
	 * an error: `config` becomes null and the form renders in create
	 * mode. Only transport failures set `configError`.
	 */
	async loadConfig(id: string): Promise<NetboxConfig | null> {
		this.state.lastArchitectureId = id;
		this.state.configLoading = true;
		this.state.configError = null;
		try {
			const config = await getNetboxConfig(id, getStoredToken() ?? undefined);
			if (!this.isCurrent(id)) return null;
			this.state.config = config;
			return config;
		} catch (err) {
			if (!this.isCurrent(id)) return null;
			this.state.configError = err instanceof Error ? err.message : 'Failed to load NetBox config';
			return null;
		} finally {
			if (this.isCurrent(id)) {
				this.state.configLoading = false;
			}
		}
	}

	/**
	 * Create/update the projection config. The draft may carry a fresh
	 * `token` — it is forwarded to the BFF exactly once and never stored
	 * here (the response is the `token_set`-only summary).
	 *
	 * Rethrows `StaleVersionError` on a 409 `PLAN_EXPIRED` conflict (the
	 * panel raises the stale-version banner; the form's drafts stay
	 * intact because they are component-local) and any other BFFError
	 * after `mutateWithRefresh` has toasted it.
	 */
	async saveConfig(id: string, expectedVersion: number, draft: NetboxConfigDraft): Promise<NetboxConfig> {
		this.state.lastArchitectureId = id;
		const token = getStoredToken() ?? undefined;
		const config = await mutateWithRefresh<NetboxConfig>(
			// Flat request — no wrapper; `id` + `expected_version` live at
			// the top level alongside the config fields (mirrors
			// architectureStore.update).
			async () => upsertNetboxConfig({ id, expected_version: expectedVersion, ...draft }, token),
			{
				patterns: REFRESH_PATTERNS,
				detailId: id,
				successMessage: 'NetBox projection config saved',
				errorMessage: 'Failed to save NetBox config'
			}
		);
		if (this.isCurrent(id)) {
			this.state.config = config;
			this.state.configError = null;
		}
		return config;
	}

	/**
	 * Remove the projection config (NetBox itself is untouched). On
	 * success the local config drops to null so the form flips back to
	 * create mode.
	 */
	async deleteConfig(id: string): Promise<void> {
		this.state.lastArchitectureId = id;
		const token = getStoredToken() ?? undefined;
		await mutateWithRefresh(
			async () => bffDeleteConfig(id, token),
			{
				patterns: REFRESH_PATTERNS,
				detailId: id,
				successMessage: 'NetBox projection config removed',
				errorMessage: 'Failed to remove NetBox config'
			}
		);
		if (this.isCurrent(id)) {
			this.state.config = null;
			this.state.dryRunPlan = null;
		}
	}

	/**
	 * Compute the dry-run plan (synchronous on the BFF, no writes). Not
	 * routed through `mutateWithRefresh`: the contract pins dry-run as
	 * write-free, so there is no dashboard state to invalidate. Errors
	 * (NETBOX_NOT_CONFIGURED / NETBOX_NOT_APPLIED / NETBOX_UNREACHABLE / …)
	 * land in `dryRunError` as a code+message pair so the panel can
	 * branch its banner on the code (unreachable vs auth vs server
	 * message); the previous plan is kept so the operator does not lose
	 * context.
	 */
	async runDryRun(id: string): Promise<NetboxProjectionPlan | null> {
		this.state.lastArchitectureId = id;
		this.state.dryRunLoading = true;
		this.state.dryRunError = null;
		try {
			const plan = await netboxDryRun(id, getStoredToken() ?? undefined);
			if (!this.isCurrent(id)) return null;
			this.state.dryRunPlan = plan;
			return plan;
		} catch (err) {
			if (!this.isCurrent(id)) return null;
			this.state.dryRunError = toActionError(err, 'NetBox dry-run failed');
			return null;
		} finally {
			if (this.isCurrent(id)) {
				this.state.dryRunLoading = false;
			}
		}
	}

	/**
	 * Enqueue an export run. Rethrows the BFFError (409
	 * `NETBOX_RUN_ACTIVE`, 403 `PRODUCTION_REQUIRES_ADMIN`, …) after
	 * `mutateWithRefresh` has toasted it, so the panel can branch on the
	 * code — the code+message pair is also captured in `state.exportError`
	 * first (cleared at the start of every attempt and on success) so
	 * the panel's inline banner survives beyond the toast. On success
	 * the runs list is refreshed so the new queued run is visible
	 * immediately — via {@link refreshRunsIfCurrent}, which never
	 * re-pins the architecture id.
	 */
	async exportNow(id: string): Promise<void> {
		this.state.lastArchitectureId = id;
		this.state.exporting = true;
		this.state.exportError = null;
		try {
			const token = getStoredToken() ?? undefined;
			await mutateWithRefresh(
				async () => bffExportNetbox(id, token),
				{
					patterns: REFRESH_PATTERNS,
					detailId: id,
					successMessage: 'NetBox export queued',
					errorMessage: 'Failed to queue NetBox export'
				}
			);
			await this.refreshRunsIfCurrent(id);
		} catch (err) {
			if (this.isCurrent(id)) {
				this.state.exportError = toActionError(err, 'Failed to queue NetBox export');
			}
			throw err;
		} finally {
			if (this.isCurrent(id)) {
				this.state.exporting = false;
			}
		}
	}

	/**
	 * Post-mutation runs refresh that deliberately does NOT re-pin
	 * `lastArchitectureId`. Unlike {@link loadRuns} (a user-initiated
	 * read of the architecture currently on screen), this refresh runs
	 * right after a mutation that was started for `id` — if the
	 * operator has since switched architectures, the fetch is skipped
	 * and any in-flight result is dropped, so architecture A's runs can
	 * never overwrite architecture B's panel state (the mutation-driven
	 * twin of the late-fetch guard).
	 */
	private async refreshRunsIfCurrent(id: string): Promise<void> {
		if (!this.isCurrent(id)) return;
		this.state.runsLoading = true;
		this.state.runsError = null;
		try {
			const runs = await listNetboxRuns(id, undefined, getStoredToken() ?? undefined);
			if (!this.isCurrent(id)) return;
			this.state.runs = runs;
		} catch (err) {
			if (!this.isCurrent(id)) return;
			this.state.runsError = err instanceof Error ? err.message : 'Failed to load NetBox runs';
		} finally {
			if (this.isCurrent(id)) {
				this.state.runsLoading = false;
			}
		}
	}

	/** List projection runs, newest first. */
	async loadRuns(id: string): Promise<NetboxRunSummary[]> {
		this.state.lastArchitectureId = id;
		this.state.runsLoading = true;
		this.state.runsError = null;
		try {
			const runs = await listNetboxRuns(id, undefined, getStoredToken() ?? undefined);
			if (!this.isCurrent(id)) return [];
			this.state.runs = runs;
			return runs;
		} catch (err) {
			if (!this.isCurrent(id)) return [];
			this.state.runsError = err instanceof Error ? err.message : 'Failed to load NetBox runs';
			return [];
		} finally {
			if (this.isCurrent(id)) {
				this.state.runsLoading = false;
			}
		}
	}

	/** Fetch one full run (plan + per-entry results) for the detail view. */
	async loadRun(id: string, runId: string): Promise<NetboxRunDetail | null> {
		this.state.lastArchitectureId = id;
		this.state.runLoading = true;
		this.state.runError = null;
		try {
			const run = await getNetboxRun(id, runId, getStoredToken() ?? undefined);
			if (!this.isCurrent(id)) return null;
			this.state.currentRun = run;
			return run;
		} catch (err) {
			if (!this.isCurrent(id)) return null;
			this.state.runError = err instanceof Error ? err.message : 'Failed to load NetBox run';
			return null;
		} finally {
			if (this.isCurrent(id)) {
				this.state.runLoading = false;
			}
		}
	}

	/**
	 * Re-enqueue a failed run. On success the runs list is refreshed so
	 * the row flips back to `queued` (via {@link refreshRunsIfCurrent}
	 * — no re-pin, same late-switch guard as `exportNow`). A 409
	 * `PROJECTION_RUN_NOT_RETRYABLE` / `NETBOX_RUN_ACTIVE` propagates
	 * (after being toasted) with its code+message captured in
	 * `state.retryError` (cleared at the start of every attempt and on
	 * success) so the history section can surface it inline.
	 */
	async retryRun(id: string, runId: string): Promise<void> {
		this.state.lastArchitectureId = id;
		this.state.retrying = true;
		this.state.retryError = null;
		try {
			const token = getStoredToken() ?? undefined;
			await mutateWithRefresh(
				async () => bffRetryRun(id, runId, token),
				{
					patterns: REFRESH_PATTERNS,
					detailId: id,
					successMessage: 'NetBox run re-queued',
					errorMessage: 'Failed to retry NetBox run'
				}
			);
			await this.refreshRunsIfCurrent(id);
		} catch (err) {
			if (this.isCurrent(id)) {
				this.state.retryError = toActionError(err, 'Failed to retry NetBox run');
			}
			throw err;
		} finally {
			if (this.isCurrent(id)) {
				this.state.retrying = false;
			}
		}
	}

	/**
	 * Drop all panel state. Called on architecture switches / panel
	 * teardown so navigation does not flash another architecture's config
	 * or runs.
	 */
	reset(): void {
		this.state.config = null;
		this.state.configLoading = false;
		this.state.configError = null;
		this.state.dryRunPlan = null;
		this.state.dryRunLoading = false;
		this.state.dryRunError = null;
		this.state.runs = [];
		this.state.runsLoading = false;
		this.state.runsError = null;
		this.state.exportError = null;
		this.state.retryError = null;
		this.state.currentRun = null;
		this.state.runLoading = false;
		this.state.runError = null;
		this.state.exporting = false;
		this.state.retrying = false;
		this.state.lastArchitectureId = null;
	}
}

export const architectureNetboxStore = new ArchitectureNetboxStore();
