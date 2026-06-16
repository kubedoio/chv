import { describe, expect, it } from 'vitest';
import type { Architecture } from '$lib/bff/architectures';
import { buildCloneNames, isStarter } from './starter';

/**
 * Coverage for the starter-detection helper.
 *
 * BFF wire-shape note: the `Architecture` type does NOT carry a `labels`
 * field — see `docs/plans/2026-06-16-starter-topologies-and-auto-seed.md` §5
 * and the `$lib/bff/architectures` module docstring. Detection must only
 * use `name` (`starter-` prefix) and `owner_user_id` (system-owned ⇒ null).
 *
 * Clone deep-copy: the BFF `createArchitecture` endpoint already accepts
 * `design_graph_json` and `latest_yaml` (see CreateArchitectureRequest), so
 * Stage C's clone flow performs a single create call and does not need a
 * follow-up `updateArchitecture` to copy the body. The detail page passes
 * the source starter's blobs directly into the create payload.
 */

function makeArch(overrides: Partial<Architecture>): Architecture {
	return {
		id: 'arch-test',
		name: 'starter-01-single-vm',
		display_name: null,
		description: null,
		environment: null,
		status: 'draft',
		owner_user_id: null,
		last_validation_status: null,
		last_fleet_check_status: null,
		version_number: 1,
		created_at: '2026-06-16T00:00:00Z',
		updated_at: '2026-06-16T00:00:00Z',
		archived_at: null,
		...overrides
	};
}

describe('isStarter', () => {
	it('returns true for a system-owned starter (prefix + null owner)', () => {
		const arch = makeArch({ name: 'starter-01-single-vm', owner_user_id: null });
		expect(isStarter(arch)).toBe(true);
	});

	it('returns false for a cloned starter (prefix dropped, user-owned)', () => {
		const arch = makeArch({ name: 'starter-01-single-vm', owner_user_id: 'u-1' });
		expect(isStarter(arch)).toBe(false);
	});

	it('returns false for a hypothetical system-owned non-starter arch', () => {
		const arch = makeArch({ name: 'my-arch', owner_user_id: null });
		expect(isStarter(arch)).toBe(false);
	});

	it('returns false for a user-owned non-starter arch', () => {
		const arch = makeArch({ name: 'my-arch', owner_user_id: 'u-2' });
		expect(isStarter(arch)).toBe(false);
	});

	it('accepts the minimal Pick shape (no need for full Architecture)', () => {
		expect(isStarter({ name: 'starter-foo', owner_user_id: null })).toBe(true);
		expect(isStarter({ name: 'foo', owner_user_id: null })).toBe(false);
	});
});

describe('buildCloneNames', () => {
	it('drops the starter- prefix and adds the -clone-<id> suffix', () => {
		const result = buildCloneNames(
			{ name: 'starter-01-single-vm', display_name: 'Single Linux Dev VM' },
			'a1b2c3d4'
		);
		expect(result.name).toBe('01-single-vm-clone-a1b2c3d4');
		expect(result.display_name).toBe('Single Linux Dev VM (clone)');
	});

	it('falls back to a slug-derived display name when display_name is null', () => {
		const result = buildCloneNames(
			{ name: 'starter-04-k8s-ha', display_name: null },
			'ff00ff00'
		);
		expect(result.name).toBe('04-k8s-ha-clone-ff00ff00');
		expect(result.display_name).toBe('04-k8s-ha clone');
	});

	it('leaves a non-prefixed name untouched in the cloned slug', () => {
		// Defensive: callers should only invoke this for actual starters, but
		// the function should still produce a sane result for non-prefixed input.
		const result = buildCloneNames({ name: 'my-arch', display_name: 'Mine' }, 'deadbeef');
		expect(result.name).toBe('my-arch-clone-deadbeef');
		expect(result.display_name).toBe('Mine (clone)');
	});
});
