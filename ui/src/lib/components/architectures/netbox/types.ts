import type { NetboxRunResult } from '#lib/bff/architectures.ts';

/**
 * Shared view-model helpers for the NetBox projection panel
 * (`components/architectures/netbox/`). Wire types themselves live in
 * `#lib/bff/architectures.ts` (named after the Rust DTOs); only
 * presentation-level shapes are declared here.
 */

/**
 * Narrow a run detail's `result_json` union (parsed payload | raw string
 * | null) into something renderable: the structured per-entry outcomes
 * when the worker's JSON parsed, the raw column string (for legacy /
 * hand-written rows) otherwise.
 */
export type NetboxRunResultView = NetboxRunResult | { raw: string } | null;

export function viewNetboxRunResult(result: NetboxRunResult | string | null): NetboxRunResultView {
	if (result === null) return null;
	if (typeof result === 'string') return { raw: result };
	// Defensive: the BFF parses the column to arbitrary JSON, so a
	// non-`entries` object would mean a payload shape outside the
	// adapter's contract. Render it as raw JSON rather than crash.
	if (!Array.isArray(result.entries)) {
		return { raw: JSON.stringify(result) };
	}
	return result;
}

/** Human labels for the plan-action badges (kept in the dry-run + history rows). */
export const NETBOX_ACTION_LABELS: Record<string, string> = {
	create: 'Create',
	update: 'Update',
	no_op: 'No change',
	conflict: 'Conflict',
	stale: 'Stale'
};

/** Human labels for the projected NetBox kinds. */
export const NETBOX_KIND_LABELS: Record<string, string> = {
	vlan: 'VLAN',
	prefix: 'Prefix',
	ip_address: 'IP address',
	interface: 'Interface',
	virtual_machine: 'Virtual machine',
	device: 'Device'
};
