import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$env/dynamic/public', () => ({
	env: {}
}));

import { APIError, clearToken, createAPIClient, getStoredToken } from '$lib/api/client';

function jsonHeaders(): Headers {
	return new Headers({ 'content-type': 'application/json' });
}

describe('createAPIClient', () => {
	beforeEach(() => {
		vi.restoreAllMocks();
		const storage = new Map<string, string>();
		vi.stubGlobal('localStorage', {
			getItem: (key: string) => storage.get(key) ?? null,
			setItem: (key: string, value: string) => {
				storage.set(key, value);
			},
			removeItem: (key: string) => {
				storage.delete(key);
			}
		});
		clearToken();
	});

	it('uses an explicit base URL and stores bearer tokens', async () => {
		const fetchMock = vi.fn().mockResolvedValue({
			ok: true,
			status: 200,
			headers: jsonHeaders(),
			json: async () => ({ ok: true })
		});

		vi.stubGlobal('fetch', fetchMock);

		const client = createAPIClient({
			baseUrl: 'http://controller.example'
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

	it('throws APIError on successful HTML response instead of uncaught JSON parse failure', async () => {
		const fetchMock = vi.fn().mockResolvedValue({
			ok: true,
			status: 200,
			headers: new Headers({ 'content-type': 'text/html' }),
			text: async () => '<!doctype html><html><body>fallback</body></html>',
			json: async () => {
				throw new Error('should not parse');
			}
		});
		vi.stubGlobal('fetch', fetchMock);

		const client = createAPIClient();

		await expect(client.listNodes()).rejects.toBeInstanceOf(APIError);
		await expect(client.listNodes()).rejects.toMatchObject({
			status: 200,
			code: 'INVALID_RESPONSE'
		});
	});
});
