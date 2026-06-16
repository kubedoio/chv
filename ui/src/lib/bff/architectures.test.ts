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
	validateArchitecture,
	validateYaml,
	generateYaml,
	importYaml,
	checkFleet,
	getArchitectureDrift,
	type ArchitectureSummary,
	type FleetCheckResult,
	type ValidationResult
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

// ─── Phase 1: validation + YAML wrappers ────────────────────────────────

const VALID_RESULT: ValidationResult = {
	status: 'valid',
	summary: { errors: 0, warnings: 0, info: 0 },
	findings: []
};

const INVALID_RESULT: ValidationResult = {
	status: 'invalid',
	summary: { errors: 1, warnings: 0, info: 0 },
	findings: [
		{
			severity: 'error',
			code: 'INVALID_CIDR',
			message: 'CIDR is not parseable',
			path: 'networks[0].cidr',
			resource_ref: 'networks/lan',
			blocking: true,
			suggestion: 'Use a valid IPv4 CIDR like 10.0.0.0/24'
		}
	]
};

describe('architectures BFF wrapper — Phase 1 validation + YAML', () => {
	beforeEach(() => {
		vi.mocked(bffFetch).mockReset();
	});

	describe('validateArchitecture', () => {
		it('POSTs the id and returns the ValidationResult verbatim', async () => {
			vi.mocked(bffFetch).mockResolvedValue(INVALID_RESULT);

			const res = await validateArchitecture({ id: 'arch-1' });

			expect(res).toEqual(INVALID_RESULT);
			expect(bffFetch).toHaveBeenCalledWith(
				expect.stringMatching(/architectures\/validate$/),
				expect.objectContaining({
					method: 'POST',
					body: JSON.stringify({ id: 'arch-1' })
				})
			);
		});

		it('forwards the optional token to bffFetch', async () => {
			vi.mocked(bffFetch).mockResolvedValue(VALID_RESULT);

			await validateArchitecture({ id: 'arch-1' }, 'tok');

			expect(bffFetch).toHaveBeenCalledWith(
				expect.any(String),
				expect.objectContaining({ token: 'tok' })
			);
		});

		it('rethrows BFFErrors so callers can branch on them', async () => {
			const boom = new BFFError('boom', 500, 'INTERNAL');
			vi.mocked(bffFetch).mockRejectedValue(boom);

			await expect(validateArchitecture({ id: 'arch-1' })).rejects.toBe(boom);
		});
	});

	describe('validateYaml', () => {
		it('POSTs the YAML body and returns the ValidationResult', async () => {
			vi.mocked(bffFetch).mockResolvedValue(VALID_RESULT);

			const res = await validateYaml({ yaml: 'kind: Topology\n' });

			expect(res).toEqual(VALID_RESULT);
			expect(bffFetch).toHaveBeenCalledWith(
				expect.stringMatching(/architectures\/validate-yaml$/),
				expect.objectContaining({
					method: 'POST',
					body: JSON.stringify({ yaml: 'kind: Topology\n' })
				})
			);
		});
	});

	describe('generateYaml', () => {
		it('POSTs the id and returns the YAML string', async () => {
			vi.mocked(bffFetch).mockResolvedValue({ yaml: 'kind: Topology\nname: app\n' });

			const res = await generateYaml({ id: 'arch-1' });

			expect(res.yaml).toBe('kind: Topology\nname: app\n');
			expect(bffFetch).toHaveBeenCalledWith(
				expect.stringMatching(/architectures\/generate-yaml$/),
				expect.objectContaining({
					method: 'POST',
					body: JSON.stringify({ id: 'arch-1' })
				})
			);
		});

		it('lets a 422 GRAPH_EMPTY BFFError propagate so the UI can render an empty state', async () => {
			const empty = new BFFError('Graph is empty', 422, 'GRAPH_EMPTY');
			vi.mocked(bffFetch).mockRejectedValue(empty);

			await expect(generateYaml({ id: 'arch-1' })).rejects.toBe(empty);
		});
	});

	describe('importYaml', () => {
		it('POSTs id+yaml and returns the wrapped ValidationResult', async () => {
			vi.mocked(bffFetch).mockResolvedValue({ result: INVALID_RESULT });

			const res = await importYaml({ id: 'arch-1', yaml: 'kind: Topology\n' });

			expect(res.result).toEqual(INVALID_RESULT);
			expect(bffFetch).toHaveBeenCalledWith(
				expect.stringMatching(/architectures\/import-yaml$/),
				expect.objectContaining({
					method: 'POST',
					body: JSON.stringify({ id: 'arch-1', yaml: 'kind: Topology\n' })
				})
			);
		});
	});
});

// ─── Phase 3: fleet check wrapper ─────────────────────────────────────────

const FLEET_VALID: FleetCheckResult = {
	status: 'valid',
	inventory_snapshot_id: 'snap-1',
	checked_at: '2026-06-15T12:00:00Z',
	findings: []
};

const FLEET_INVALID: FleetCheckResult = {
	status: 'invalid',
	inventory_snapshot_id: 'snap-2',
	checked_at: '2026-06-15T12:01:00Z',
	findings: [
		{
			severity: 'error',
			code: 'INSUFFICIENT_MEMORY',
			message: 'host lacks 32GB RAM',
			path: 'instances[0]',
			resource_ref: 'instance/web',
			blocking: true,
			suggestion: 'reduce instance memory or pick a larger host'
		}
	]
};

describe('architectures BFF wrapper — Phase 3 fleet check', () => {
	beforeEach(() => {
		vi.mocked(bffFetch).mockReset();
	});

	it('POSTs the id to /v1/architectures/check-fleet and returns the FleetCheckResult', async () => {
		vi.mocked(bffFetch).mockResolvedValue(FLEET_INVALID);

		const res = await checkFleet({ id: 'arch-1' });

		expect(res).toEqual(FLEET_INVALID);
		expect(bffFetch).toHaveBeenCalledWith(
			expect.stringMatching(/architectures\/check-fleet$/),
			expect.objectContaining({
				method: 'POST',
				body: JSON.stringify({ id: 'arch-1' })
			})
		);
	});

	it('forwards the optional token to bffFetch', async () => {
		vi.mocked(bffFetch).mockResolvedValue(FLEET_VALID);

		await checkFleet({ id: 'arch-1' }, 'tok');

		expect(bffFetch).toHaveBeenCalledWith(
			expect.any(String),
			expect.objectContaining({ token: 'tok' })
		);
	});

	it('rethrows BFFErrors so callers can branch on them', async () => {
		const boom = new BFFError('snapshot failed', 503, 'INVENTORY_UNAVAILABLE');
		vi.mocked(bffFetch).mockRejectedValue(boom);

		await expect(checkFleet({ id: 'arch-1' })).rejects.toBe(boom);
	});
});

// ----------------------------------------------------------------------
// Regression — getArchitectureDrift token-forward.
// ----------------------------------------------------------------------
//
// History: a user clicking a saved topology was being logged out. The
// dashboard fan-out at `routes/architectures/+page.svelte` calls
// `getArchitectureDrift(id, false, undefined, signal)` per card. The
// third positional argument was a placeholder named `_fetch` and the
// function ignored it — meaning the request went out with NO
// Authorization header, the BFF (which gates `/v1/architectures/drift`
// at operator tier) returned 401, and the global `bffFetch` 401 handler
// in `client.ts` redirected to `/login`. The `.catch(() => null)` at
// the call site does NOT suppress the redirect — it only suppresses
// the rethrown BFFError.
//
// Fix: third positional is now `token?: string` and forwarded into
// bffFetch's `init.token`. The dashboard and the runes store both pass
// `getStoredToken() ?? undefined` from the call site.

describe('architectures BFF wrapper — Phase 6 drift', () => {
	beforeEach(() => {
		vi.mocked(bffFetch).mockReset();
	});

	const NO_DRIFT = {
		status: 'no_drift' as const,
		findings: [],
		summary: { total: 0, by_type: {} },
		baseline_version_id: 'v-1',
		snapshot_at: '2026-06-16T12:00:00Z',
		computed_at: '2026-06-16T12:00:00Z',
		drift_report_id: 'rpt-1'
	};

	it('forwards the token into bffFetch so the dashboard fan-out is authenticated', async () => {
		vi.mocked(bffFetch).mockResolvedValue(NO_DRIFT);

		await getArchitectureDrift('arch-1', false, 'tok-abc');

		expect(bffFetch).toHaveBeenCalledWith(
			expect.stringMatching(/architectures\/drift$/),
			expect.objectContaining({
				method: 'POST',
				body: JSON.stringify({ id: 'arch-1', force_refresh: false }),
				token: 'tok-abc'
			})
		);
	});

	it('forwards the AbortSignal as well as the token', async () => {
		vi.mocked(bffFetch).mockResolvedValue(NO_DRIFT);
		const ac = new AbortController();

		await getArchitectureDrift('arch-1', true, 'tok-abc', ac.signal);

		expect(bffFetch).toHaveBeenCalledWith(
			expect.any(String),
			expect.objectContaining({
				token: 'tok-abc',
				signal: ac.signal,
				body: JSON.stringify({ id: 'arch-1', force_refresh: true })
			})
		);
	});

	it('omits Authorization when the caller passes undefined (server returns 401, the FIX is the call site, not this fn)', async () => {
		// This test pins the behaviour change explicitly: when token is
		// undefined, bffFetch's `init.token` is undefined and no
		// Authorization header is set. The bug surfaced because callers
		// were forced into this code path; the fix is to make sure
		// callers DO pass a token. This test exists so a future
		// "helpful" change to add `getStoredToken()` inside the fn
		// itself (which would defeat the testability of token-forward
		// in node tests) is reviewed deliberately.
		vi.mocked(bffFetch).mockResolvedValue(NO_DRIFT);

		await getArchitectureDrift('arch-1', false);

		expect(bffFetch).toHaveBeenCalledWith(
			expect.any(String),
			expect.objectContaining({ token: undefined })
		);
	});
});
