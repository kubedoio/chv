import { goto } from '$app/navigation';
import { getStoredToken } from '$lib/api/client';
import {
	apply as bffApply,
	destroy as bffDestroy,
	getApplyRun,
	listApplyRuns,
	isTerminalRunStatus,
	type ApplyRunDetail,
	type ApplyRunResult,
	type ConfirmationToken,
	type PlanMode
} from '$lib/bff/architectures';
import { mutateWithRefresh } from './mutation.svelte';

/**
 * Reactive Svelte 5 runes-based store for Phase 5 apply runs.
 *
 * Responsibilities
 * - kick off `apply`/`destroy` POSTs and navigate to the per-run page;
 * - hold the currently-displayed run (`currentRun`) for the `[run_id]` page;
 * - poll the run every 2s while it sits in `queued`/`running`, stopping on
 *   any terminal state or when the consumer calls `stopPolling()`.
 *
 * Mutation compliance
 * - `applyAndNavigate` / `destroyAndNavigate` go through `mutateWithRefresh`
 *   so the architecture row's `last_plan_status` (flipped to `applying` by
 *   the BFF) is reflected in the dashboard list. We deliberately omit a
 *   `successMessage`: the user lands on the runs page which is its own
 *   ack, and a toast on top would be noise.
 *
 * Lifecycle / cleanup
 * - The polling timer is created once per `pollRun` call. `stopPolling` is
 *   idempotent and safe to call from a Svelte `$effect` cleanup or from
 *   another module.
 */

const REFRESH_PATTERNS = ['architectures:'];
const DEFAULT_POLL_INTERVAL_MS = 2000;

interface RunsState {
	currentRun: ApplyRunDetail | null;
	polling: boolean;
	error: string | null;
	loading: boolean;
}

class ArchitectureRunsStore {
	state = $state<RunsState>({
		currentRun: null,
		polling: false,
		error: null,
		loading: false
	});

	private pollTimer: ReturnType<typeof setTimeout> | null = null;
	private pollAbort = false;

	get currentRun(): ApplyRunDetail | null {
		return this.state.currentRun;
	}

	get polling(): boolean {
		return this.state.polling;
	}

	get error(): string | null {
		return this.state.error;
	}

	get loading(): boolean {
		return this.state.loading;
	}

	/**
	 * Apply a plan, then navigate to the runs detail page on success.
	 *
	 * The store keeps a copy of the freshly-created run so the destination
	 * page can render immediately (without waiting for the first poll
	 * round-trip).
	 */
	async applyAndNavigate(
		architectureId: string,
		planId: string,
		confirmation: ConfirmationToken,
		acknowledgedWarnings: boolean,
		mode: PlanMode = 'apply'
	): Promise<ApplyRunResult> {
		const token = getStoredToken() ?? undefined;
		const fn = mode === 'destroy' ? bffDestroy : bffApply;
		const result = await mutateWithRefresh<ApplyRunResult>(
			async () => fn(architectureId, planId, confirmation, acknowledgedWarnings, token),
			{
				patterns: REFRESH_PATTERNS,
				detailId: architectureId,
				errorMessage:
					mode === 'destroy' ? 'Failed to apply destroy plan' : 'Failed to apply plan'
			}
		);

		// Seed currentRun so the destination page paints without a
		// round-trip. The polling loop will refresh it on the next tick.
		this.state.currentRun = {
			id: result.run_id,
			architecture_id: result.architecture_id,
			architecture_version_id: result.architecture_version_id,
			plan_id: result.plan_id,
			task_id: result.task_id,
			status: result.status,
			started_at: result.started_at,
			finished_at: null,
			requested_by: null,
			result_json: null,
			error_message: null,
			created_at: result.started_at ?? new Date().toISOString(),
			updated_at: result.started_at ?? new Date().toISOString()
		};
		this.state.error = null;

		await goto(`/architectures/${architectureId}/runs/${result.run_id}`);
		return result;
	}

	/**
	 * One-shot fetch for the run detail page's server load. Does not start
	 * polling — call `pollRun()` from the page mount once the data is
	 * rendered.
	 */
	async fetchRun(architectureId: string, runId: string): Promise<ApplyRunDetail> {
		const token = getStoredToken() ?? undefined;
		this.state.loading = true;
		this.state.error = null;
		try {
			const run = await getApplyRun(architectureId, runId, token);
			this.state.currentRun = run;
			return run;
		} catch (err) {
			this.state.error = err instanceof Error ? err.message : 'Failed to load run';
			throw err;
		} finally {
			this.state.loading = false;
		}
	}

	/**
	 * One-shot list fetch for the runs index page.
	 */
	async listRuns(architectureId: string): Promise<ApplyRunDetail[]> {
		const token = getStoredToken() ?? undefined;
		return listApplyRuns(architectureId, token);
	}

	/**
	 * Poll a run until it lands in a terminal status. Safe to call multiple
	 * times — a fresh call cancels the previous loop. Cleans up its own
	 * timer when the run terminates or `stopPolling` is invoked.
	 */
	pollRun(architectureId: string, runId: string, intervalMs = DEFAULT_POLL_INTERVAL_MS): void {
		this.stopPolling();
		this.pollAbort = false;
		this.state.polling = true;

		const tick = async () => {
			if (this.pollAbort) return;
			try {
				const token = getStoredToken() ?? undefined;
				const run = await getApplyRun(architectureId, runId, token);
				this.state.currentRun = run;
				this.state.error = null;
				if (isTerminalRunStatus(run.status)) {
					this.stopPolling();
					return;
				}
			} catch (err) {
				// Soft-fail: keep the prior currentRun so the user does not
				// lose context on a transient network blip; surface the
				// error message so the page can banner it. We keep polling
				// to recover automatically.
				this.state.error = err instanceof Error ? err.message : 'Failed to refresh run';
			}
			if (!this.pollAbort) {
				this.pollTimer = setTimeout(tick, intervalMs);
			}
		};

		this.pollTimer = setTimeout(tick, intervalMs);
	}

	/**
	 * Cancel any in-flight polling loop. Idempotent.
	 */
	stopPolling(): void {
		this.pollAbort = true;
		if (this.pollTimer !== null) {
			clearTimeout(this.pollTimer);
			this.pollTimer = null;
		}
		this.state.polling = false;
	}

	/**
	 * Reset the store to its initial state. Safe to call from page
	 * cleanup so the next visit starts fresh.
	 */
	reset(): void {
		this.stopPolling();
		this.state.currentRun = null;
		this.state.error = null;
		this.state.loading = false;
	}
}

export const architectureRunsStore = new ArchitectureRunsStore();
