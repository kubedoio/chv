import type { Architecture } from '$lib/bff/architectures';

/**
 * Detect a system-provided starter topology.
 *
 * Starters are seeded by the controlplane on first deployment with names like
 * `starter-01-single-vm`. They are owned by the system (`owner_user_id ===
 * null`) and start in `draft` status. Operators are expected to clone — not
 * edit — them; the detail page surfaces a Clone button and a read-only banner
 * when this returns true.
 *
 * NOTE on detection: the plan deliberately keeps `labels` off the wire (see
 * `docs/plans/2026-06-16-starter-topologies-and-auto-seed.md` §5 and the
 * `Architecture` wire type in `$lib/bff/architectures`). The two fields below
 * are the only durable signal: a `starter-` prefixed name *and* a system-owned
 * row. A user-cloned starter loses the `null` owner so this returns false; a
 * hypothetical system-owned arch without the prefix also returns false.
 */
export function isStarter(arch: Pick<Architecture, 'name' | 'owner_user_id'>): boolean {
	return arch.name.startsWith('starter-') && arch.owner_user_id === null;
}

/**
 * Build the cloned `name` and `display_name` from a starter.
 *
 * Names must be unique on the wire, so we suffix with a short id (a UUID
 * fragment — the caller passes whatever short-id scheme the page uses) to
 * avoid collisions when the same user clones the same starter twice.
 *
 * The `starter-` prefix is dropped from the slug — clones belong to the user,
 * not the system — and the display name gets a parenthetical hint so the
 * dashboard can distinguish a fresh clone from its source at a glance.
 */
export function buildCloneNames(
	starter: Pick<Architecture, 'name' | 'display_name'>,
	shortId: string
): { name: string; display_name: string } {
	const base = starter.name.replace(/^starter-/, '');
	return {
		name: `${base}-clone-${shortId}`,
		display_name: starter.display_name ? `${starter.display_name} (clone)` : `${base} clone`
	};
}
