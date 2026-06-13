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
	listArchitectures,
	createArchitecture,
	updateArchitecture,
	archiveArchitecture,
	type ArchitectureSummary
} from './architectures';

const SUMMARY: ArchitectureSummary = {
	id: 'arch-1',
	name: 'phase-0-test',
	display_name: 'Phase 0 Test',
	description: 'smoke',
	environment: 'development',
	status: 'draft',
	owner_user_id: null,
	last_validation_status: null,
	last_fleet_check_status: null,
	version_number: 1,
	created_at: '2026-06-13T00:00:00Z',
	updated_at: '2026-06-13T00:00:00Z',
	archived_at: null
};

describe('architectures BFF wrapper — wire shape', () => {
	beforeEach(() => {
		vi.mocked(bffFetch).mockReset();
	});

	describe('listArchitectures', () => {
		it('returns the architectures array from the new wire shape', async () => {
			vi.mocked(bffFetch).mockResolvedValue({ architectures: [SUMMARY] });

			const res = await listArchitectures();

			expect(res.architectures).toHaveLength(1);
			expect(res.architectures[0]).toEqual(SUMMARY);
		});

		it('passes include_archived through to the BFF', async () => {
			vi.mocked(bffFetch).mockResolvedValue({ architectures: [] });

			await listArchitectures({ include_archived: true });

			expect(bffFetch).toHaveBeenCalledWith(
				expect.stringMatching(/architectures\/list$/),
				expect.objectContaining({
					method: 'POST',
					body: JSON.stringify({ include_archived: true })
				})
			);
		});
	});

	describe('createArchitecture', () => {
		it('sends the create body verbatim and returns the created architecture', async () => {
			vi.mocked(bffFetch).mockResolvedValue({ architecture: SUMMARY });

			const res = await createArchitecture({
				name: 'phase-0-test',
				description: 'smoke',
				environment: 'development'
			});

			expect(res.architecture).toEqual(SUMMARY);
			expect(bffFetch).toHaveBeenCalledWith(
				expect.stringMatching(/architectures\/create$/),
				expect.objectContaining({
					method: 'POST',
					body: JSON.stringify({
						name: 'phase-0-test',
						description: 'smoke',
						environment: 'development'
					})
				})
			);
		});
	});

	describe('updateArchitecture (FLAT shape, no patch wrapper)', () => {
		it('sends a flat body with id, expected_version and the editable fields', async () => {
			vi.mocked(bffFetch).mockResolvedValue({
				architecture: { ...SUMMARY, display_name: 'renamed', version_number: 2 }
			});

			await updateArchitecture({
				id: 'arch-1',
				expected_version: 1,
				display_name: 'renamed',
				description: 'edited',
				environment: 'staging'
			});

			expect(bffFetch).toHaveBeenCalledWith(
				expect.stringMatching(/architectures\/update$/),
				expect.objectContaining({
					method: 'POST',
					body: JSON.stringify({
						id: 'arch-1',
						expected_version: 1,
						display_name: 'renamed',
						description: 'edited',
						environment: 'staging'
					})
				})
			);
		});

		it('passes through success responses verbatim', async () => {
			vi.mocked(bffFetch).mockResolvedValue({
				architecture: { ...SUMMARY, version_number: 2 }
			});

			const res = await updateArchitecture({
				id: 'arch-1',
				expected_version: 1,
				display_name: 'renamed'
			});

			expect(res.architecture.version_number).toBe(2);
		});

		it('maps a 409 BFFError to StaleVersionError', async () => {
			vi.mocked(bffFetch).mockRejectedValue(
				new BFFError('Stale architecture version', 409, 'STALE_VERSION')
			);

			await expect(
				updateArchitecture({ id: 'arch-1', expected_version: 1, display_name: 'x' })
			).rejects.toBeInstanceOf(StaleVersionError);
		});

		it('preserves architectureId and expectedVersion on the StaleVersionError', async () => {
			vi.mocked(bffFetch).mockRejectedValue(
				new BFFError('Stale', 409, 'STALE_VERSION')
			);

			try {
				await updateArchitecture({
					id: 'arch-42',
					expected_version: 7,
					display_name: 'x'
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

		it('rethrows non-409 BFFErrors verbatim', async () => {
			const otherErr = new BFFError('Server boom', 500, 'INTERNAL');
			vi.mocked(bffFetch).mockRejectedValue(otherErr);

			await expect(
				updateArchitecture({ id: 'arch-1', expected_version: 1 })
			).rejects.toBe(otherErr);
		});

		it('accepts an empty editable-fields update (partial-patch is valid)', async () => {
			vi.mocked(bffFetch).mockResolvedValue({ architecture: SUMMARY });

			const res = await updateArchitecture({ id: 'arch-1', expected_version: 1 });

			expect(res.architecture).toEqual(SUMMARY);
			expect(bffFetch).toHaveBeenCalledWith(
				expect.stringMatching(/architectures\/update$/),
				expect.objectContaining({
					body: JSON.stringify({ id: 'arch-1', expected_version: 1 })
				})
			);
		});
	});

	describe('archiveArchitecture', () => {
		it('sends id and expected_version and returns the archived row', async () => {
			const archived: ArchitectureSummary = {
				...SUMMARY,
				status: 'archived',
				version_number: 2,
				archived_at: '2026-06-13T01:00:00Z'
			};
			vi.mocked(bffFetch).mockResolvedValue({ architecture: archived });

			const res = await archiveArchitecture({ id: 'arch-1', expected_version: 1 });

			expect(res.architecture.status).toBe('archived');
			expect(res.architecture.archived_at).toBe('2026-06-13T01:00:00Z');
			expect(bffFetch).toHaveBeenCalledWith(
				expect.stringMatching(/architectures\/archive$/),
				expect.objectContaining({
					method: 'POST',
					body: JSON.stringify({ id: 'arch-1', expected_version: 1 })
				})
			);
		});

		it('maps a 409 BFFError to StaleVersionError', async () => {
			vi.mocked(bffFetch).mockRejectedValue(
				new BFFError('Stale', 409, 'STALE_VERSION')
			);

			await expect(
				archiveArchitecture({ id: 'arch-1', expected_version: 3 })
			).rejects.toBeInstanceOf(StaleVersionError);
		});

		it('rethrows non-409 BFFErrors verbatim', async () => {
			const otherErr = new BFFError('Server boom', 500, 'INTERNAL');
			vi.mocked(bffFetch).mockRejectedValue(otherErr);

			await expect(
				archiveArchitecture({ id: 'arch-1', expected_version: 1 })
			).rejects.toBe(otherErr);
		});
	});
});
