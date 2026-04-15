import { describe, expect, it, vi } from 'vitest';
import { load } from '../../routes/+page.server';

vi.mock('$lib/bff/overview', () => ({
	loadOverview: vi.fn()
}));

import { loadOverview } from '$lib/bff/overview';

describe('overview page server load', () => {
	it('returns overview and meta error false when loadOverview succeeds', async () => {
		const mockedLoadOverview = vi.mocked(loadOverview);
		mockedLoadOverview.mockResolvedValue({
			health_tiles: [
				{ key: 'nodes', label: 'Nodes', status: 'healthy', value: '3' }
			],
			capacity_tiles: [
				{ key: 'cpu', label: 'CPU', used: '10%', total: '32 cores' }
			],
			recent_tasks: [],
			active_alerts: []
		});

		const result = await load({
			cookies: { get: () => 'token-123' }
		} as Parameters<typeof load>[0]);

		expect(result.meta.error).toBe(false);
		expect(result.overview).not.toBeNull();
		expect(result.overview?.health_tiles).toHaveLength(1);
	});

	it('returns null overview and meta error true when loadOverview throws', async () => {
		const mockedLoadOverview = vi.mocked(loadOverview);
		mockedLoadOverview.mockRejectedValue(new Error('BFF down'));

		const result = await load({
			cookies: { get: () => 'token-123' }
		} as Parameters<typeof load>[0]);

		expect(result.meta.error).toBe(true);
		expect(result.overview).toBeNull();
	});
});
