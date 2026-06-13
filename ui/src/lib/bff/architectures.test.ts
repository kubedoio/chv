import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('./client', async () => {
	const actual = await vi.importActual<typeof import('./client')>('./client');
	return {
		...actual,
		bffFetch: vi.fn()
	};
});

import { BFFError, bffFetch } from './client';
import {
	StaleVersionError,
	updateArchitecture,
	archiveArchitecture
} from './architectures';

describe('architectures BFF wrapper — optimistic concurrency', () => {
	beforeEach(() => {
		vi.mocked(bffFetch).mockReset();
	});

	it('updateArchitecture passes through success responses', async () => {
		vi.mocked(bffFetch).mockResolvedValue({
			architecture: {
				id: 'arch-1',
				name: 'a',
				description: '',
				environment: 'development',
				status: 'draft',
				version_number: 2,
				created_at: '',
				updated_at: ''
			}
		});

		const res = await updateArchitecture({
			id: 'arch-1',
			expected_version: 1,
			patch: { name: 'a' }
		});

		expect(res.architecture.version_number).toBe(2);
	});

	it('updateArchitecture maps a 409 BFFError to StaleVersionError', async () => {
		vi.mocked(bffFetch).mockRejectedValue(
			new BFFError('Stale architecture version', 409, 'STALE_VERSION')
		);

		await expect(
			updateArchitecture({ id: 'arch-1', expected_version: 1, patch: { name: 'x' } })
		).rejects.toBeInstanceOf(StaleVersionError);
	});

	it('updateArchitecture preserves architectureId and expectedVersion on the StaleVersionError', async () => {
		vi.mocked(bffFetch).mockRejectedValue(
			new BFFError('Stale', 409, 'STALE_VERSION')
		);

		try {
			await updateArchitecture({
				id: 'arch-42',
				expected_version: 7,
				patch: { name: 'x' }
			});
			throw new Error('expected StaleVersionError to be thrown');
		} catch (err) {
			expect(err).toBeInstanceOf(StaleVersionError);
			const stale = err as StaleVersionError;
			expect(stale.architectureId).toBe('arch-42');
			expect(stale.expectedVersion).toBe(7);
			expect(stale.code).toBe('STALE_VERSION');
		}
	});

	it('updateArchitecture rethrows non-409 BFFErrors verbatim', async () => {
		const otherErr = new BFFError('Server boom', 500, 'INTERNAL');
		vi.mocked(bffFetch).mockRejectedValue(otherErr);

		await expect(
			updateArchitecture({ id: 'arch-1', expected_version: 1, patch: {} })
		).rejects.toBe(otherErr);
	});

	it('archiveArchitecture maps a 409 BFFError to StaleVersionError', async () => {
		vi.mocked(bffFetch).mockRejectedValue(
			new BFFError('Stale', 409, 'STALE_VERSION')
		);

		await expect(
			archiveArchitecture({ id: 'arch-1', expected_version: 3 })
		).rejects.toBeInstanceOf(StaleVersionError);
	});
});
