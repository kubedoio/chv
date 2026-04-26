import { browser } from '$app/environment';
import { getStoredToken } from '$lib/api/client';
import { loadOverview } from '$lib/bff/overview';
import { createOverview, toOverviewModel, type OverviewModel } from '$lib/helpers/dashboard';

const POLL_INTERVAL_MS = 10_000;

class DashboardStore {
	overview = $state<OverviewModel>(createOverview('loading'));
	isLoading = $state(false);
	error = $state<string | null>(null);
	private intervalId: ReturnType<typeof setInterval> | null = null;

	constructor(initial?: OverviewModel) {
		if (initial) {
			this.overview = initial;
		}
	}

	async refresh(): Promise<void> {
		if (!browser) return;

		const token = getStoredToken();
		this.isLoading = true;
		this.error = null;

		try {
			const payload = await loadOverview(token ?? undefined);
			this.overview = toOverviewModel(payload);
		} catch (err) {
			this.overview = createOverview('error');
			this.error = err instanceof Error ? err.message : 'Failed to load overview';
		} finally {
			this.isLoading = false;
		}
	}

	startPolling(intervalMs: number = POLL_INTERVAL_MS): void {
		this.stopPolling();
		this.intervalId = setInterval(() => {
			this.refresh();
		}, intervalMs);
	}

	stopPolling(): void {
		if (this.intervalId) {
			clearInterval(this.intervalId);
			this.intervalId = null;
		}
	}
}

export const dashboard = new DashboardStore();
