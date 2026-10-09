import type {
	NetboxPlanSummary,
	NetboxProjectionPlan,
	NetboxRunResult
} from '#lib/bff/architectures.ts';
import type { NetboxActionError } from '#lib/stores/architecture-netbox-store.svelte.ts';

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

/**
 * The plan-summary chip strip definition (label + summary key), in the
 * contract's fixed order. Shared by the dry-run table and the run
 * detail's executed-plan row so both paint the same five-slot readout.
 */
export const NETBOX_PLAN_SUMMARY_CHIPS: ReadonlyArray<{
	key: keyof NetboxPlanSummary;
	label: string;
}> = [
	{ key: 'create', label: 'Creates' },
	{ key: 'update', label: 'Updates' },
	{ key: 'no_op', label: 'Unchanged' },
	{ key: 'conflict', label: 'Conflicts' },
	{ key: 'stale', label: 'Stale' }
];

/**
 * Narrow a run detail's `plan_json` union (parsed plan | raw string |
 * null) into the executed plan's summary counts — or null when the
 * column held anything other than a parsed plan (raw string, null, or
 * an unexpected JSON shape). Kept as the defensive fallback path of
 * {@link viewNetboxExecutedPlanSummary}.
 */
export function viewNetboxPlanSummary(
	plan: NetboxProjectionPlan | string | null
): NetboxPlanSummary | null {
	if (plan === null || typeof plan === 'string') return null;
	// Defensive: the BFF parses the column to arbitrary JSON, so a
	// payload without the plan's summary counts is not a plan shape.
	if (typeof plan.summary !== 'object' || plan.summary === null) return null;
	return plan.summary;
}

/**
 * The executed plan's summary counts for a run detail, derived from the
 * REAL data flow: the adapter outcome inside `result_json` carries the
 * executed `plan`, so `result_json.plan.summary` is the primary source
 * (both enqueue sites write `plan_json: None` and nothing populates
 * that column later — the top-level `plan_json` path is a defensive
 * fallback for any future writer). Null when neither source parses —
 * the executed-plan row is skipped entirely in that case.
 */
export function viewNetboxExecutedPlanSummary(
	result: NetboxRunResultView,
	plan: NetboxProjectionPlan | string | null
): NetboxPlanSummary | null {
	if (result !== null && !('raw' in result)) {
		// Defensive: the BFF parses the column to arbitrary JSON, so
		// the outcome's plan/summary must be shape-checked, not trusted.
		const summary = result.plan?.summary;
		if (typeof summary === 'object' && summary !== null) return summary;
	}
	return viewNetboxPlanSummary(plan);
}

/**
 * Code-specific banner text for a failed export attempt. The BFF
 * contract's stable codes each map to an actionable sentence; unknown
 * codes fall back to the server's message.
 */
export function netboxExportErrorText(error: NetboxActionError): string {
	switch (error.code) {
		case 'NETBOX_RUN_ACTIVE':
			return 'An export is already queued or running for this architecture — wait for it to finish before exporting again.';
		case 'PRODUCTION_REQUIRES_ADMIN':
			return 'Exports of production architectures require an admin — ask an admin to run the export.';
		case 'NETBOX_NOT_APPLIED':
			return 'The architecture has no succeeded apply run yet — apply it first; the projection copies the most recently applied topology.';
		case 'NETBOX_NOT_CONFIGURED':
			return 'No NetBox projection config exists — save one in the Configuration section above.';
		default:
			return error.message;
	}
}

/**
 * Code-specific banner text for a failed retry attempt (the run
 * history's inline refusal surface).
 */
export function netboxRetryErrorText(error: NetboxActionError): string {
	switch (error.code) {
		case 'PROJECTION_RUN_NOT_RETRYABLE':
			return 'This run can no longer be retried — it is not in a failed state, or the attempt cap was reached.';
		case 'NETBOX_RUN_ACTIVE':
			return 'An export is already queued or running — retry once it finishes.';
		default:
			return error.message;
	}
}

/**
 * Code-specific banner text for a failed dry-run, distinguishing the
 * two 502 shapes (NetBox side unreachable vs token rejected) from the
 * server's own messages.
 */
export function netboxDryRunErrorText(error: NetboxActionError): string {
	switch (error.code) {
		case 'NETBOX_UNREACHABLE':
			return 'NetBox is unreachable — check the endpoint and network.';
		case 'NETBOX_AUTH_FAILED':
			return 'NetBox rejected the configured token — re-save the config with a valid token.';
		default:
			return error.message;
	}
}
