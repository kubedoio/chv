import { describe, expect, it, vi } from 'vitest';

vi.mock('$env/dynamic/public', () => ({
	env: {}
}));
vi.mock('$app/environment', () => ({
	browser: false
}));

import { loadOverviewPageData, loadTasksPageData } from '$lib/webui/load';

describe('webui load helpers', () => {
	it('defers protected data fetches during the server-side pass', async () => {
		const fetcher = vi.fn();

		const overview = await loadOverviewPageData(fetcher as typeof fetch);
		const tasks = await loadTasksPageData(
			fetcher as typeof fetch,
			new URL('https://example.test/tasks')
		);

		expect(fetcher).not.toHaveBeenCalled();
		expect(overview.meta.deferred).toBe(true);
		expect(overview.meta.attempted).toBe(0);
		expect(tasks.meta.deferred).toBe(true);
		expect(tasks.tasks.filters.current.window).toBe('7d');
	});
});
