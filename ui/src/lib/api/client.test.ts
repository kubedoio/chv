import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$env/dynamic/public', () => ({
	env: {}
}));

import { clearToken, createAPIClient, getStoredToken } from '$lib/api/client';

describe('createAPIClient', () => {
	beforeEach(() => {
		clearToken();
		vi.restoreAllMocks();
	});

	it('uses an explicit base URL and stores bearer tokens', async () => {
		const fetchMock = vi.fn().mockResolvedValue({
			ok: true,
			json: async () => ({ ok: true })
		});

		vi.stubGlobal('fetch', fetchMock);

		const client = createAPIClient({
			baseUrl: 'http://controller.example/api/v1'
		});

		client.setToken('token-123');
		await client.validateLogin();

		expect(getStoredToken()).toBe('token-123');
		expect(fetchMock).toHaveBeenCalledWith(
			'http://controller.example/api/v1/login/validate',
			expect.objectContaining({
				method: 'POST',
				headers: expect.any(Headers)
			})
		);
	});
});
