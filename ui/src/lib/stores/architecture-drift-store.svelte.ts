import {
	getArchitectureDrift,
	type DriftReport,
	type DriftStatus
} from '$lib/bff/architectures';
import { getStoredToken } from '$lib/api/client';
import { mutateWithRefresh } from './mutation.svelte';

/**
 * Reactive Svelte 5 runes-based store for Phase 6 drift detection.
 *
 * Responsibilities
 * - Hold the most recently fetched `DriftReport` for the architecture detail
 *   page's drift tab;
 * - Expose `refresh(id, force)` and `reset()` for the panel; the refresh
 *   path goes through `mutateWithRefresh` so the BFF's persisted drift
 *   status (used by the dashboard's per-card badge) stays in sync.
 *
 * Drift is on-demand — the store does NOT poll. Callers wire the Drift tab
 * to call `refresh(id, false)` on first activation; the operator clicks
 * "Refresh" to force-recompute.
 *
 * Per-card dashboard fetches live in `architectureStore.refreshDriftStatusForList`
 * and bypass this store — they are batched read-only fetches and never
 * persist into the panel's local state.
 */

const REFRESH_PATTERNS = ['architectures:'];

interface DriftState {
	report: DriftReport | null;
	loading: boolean;
	error: string | null;
	/**
	 * The architecture id that the current `report` belongs to. Used to
	 * defend against late-arriving fetches: if the panel switches
	 * architectures mid-flight, we drop the stale result instead of
	 * letting it clobber the new arch's state. Callers SHOULD call
	 * `reset()` between architecture switches; the id check is a
	 * second line of defence.
	 */
	lastArchitectureId: string | null;
}

class ArchitectureDriftStore {
	state = $state<DriftState>({
		report: null,
		loading: false,
		error: null,
		lastArchitectureId: null
	});

	get report(): DriftReport | null {
		return this.state.report;
	}

	get loading(): boolean {
		return this.state.loading;
	}

	get error(): string | null {
		return this.state.error;
	}

	get status(): DriftStatus {
		return this.state.report?.status ?? 'unknown';
	}

	/**
	 * Fetch the drift report for an architecture.
	 *
	 * `force = false` (the default) lets the BFF return a cached row when
	 * available; `force = true` triggers a fresh inventory snapshot + recompute.
	 * The call goes through `mutateWithRefresh` because a successful compute
	 * persists `architecture_drift_reports.status`, which the dashboard reads
	 * via `refreshDriftStatusForList` — refreshing the live-state cache keeps
	 * any sibling page consistent.
	 *
	 * On `BFFError` we surface a single-line message in `state.error`; the
	 * panel banners it. The previous `state.report` is kept so the operator
	 * does not lose context on a transient blip.
	 *
	 * If the store has switched to a different architecture between the
	 * call and the resolution (a late fetch from the previous panel), the
	 * result is dropped — `state.report` and `state.lastArchitectureId`
	 * stay pinned to whatever the most recent in-flight call wrote.
	 */
	async refresh(id: string, force = false): Promise<DriftReport | null> {
		this.state.lastArchitectureId = id;
		this.state.loading = true;
		this.state.error = null;
		try {
			const result = await mutateWithRefresh<DriftReport>(
				async () => getArchitectureDrift(id, force, getStoredToken() ?? undefined),
				{
					patterns: REFRESH_PATTERNS,
					detailId: id,
					errorMessage: 'Failed to load drift report'
				}
			);
			// Drop the result if the store has switched architectures while
			// this fetch was in flight. The newer caller's loading/state
			// stays untouched.
			if (this.state.lastArchitectureId !== id) {
				return null;
			}
			this.state.report = result;
			return result;
		} catch (err) {
			if (this.state.lastArchitectureId !== id) {
				return null;
			}
			this.state.error = err instanceof Error ? err.message : 'Failed to load drift report';
			return null;
		} finally {
			if (this.state.lastArchitectureId === id) {
				this.state.loading = false;
			}
		}
	}

	/**
	 * Drop the cached report. Called from the panel's `$effect` cleanup so
	 * navigating between architectures does not flash stale findings.
	 */
	reset(): void {
		this.state.report = null;
		this.state.loading = false;
		this.state.error = null;
		this.state.lastArchitectureId = null;
	}
}

export const architectureDriftStore = new ArchitectureDriftStore();
