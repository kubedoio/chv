/**
 * Presentation helpers and rule templates for the native alerting
 * surfaces (campaign #602 PR-6): incident state vocabulary, spec
 * summary formatting, silence quick picks, and the seven starter rule
 * templates. Pure functions — tested in `rules.test.ts` — so the
 * alert components stay under the size budget and the vocabulary
 * stays single-sourced.
 *
 * Hard rule: every `metric_id` in a template must exist in the v1
 * metric registry (`crates/chv-monitoring-core/src/registry.rs`) —
 * the BFF rejects unknown ids with 400 UNKNOWN_METRIC, and a rule on
 * a nonexistent metric would be a silent never-firing trap. The test
 * pins the template ids against the registry allowlist.
 */

import type {
	AlertRule,
	AlertSeverity,
	CheckStatusMatch,
	MissingDataPolicy,
	RuleType,
	ThresholdOperator
} from '#lib/bff/alerting.ts';

// ---------------------------------------------------------------------------
// Incident state vocabulary
// ---------------------------------------------------------------------------

export type IncidentTone = 'warning' | 'danger' | 'success';

export interface IncidentStateView {
	label: string;
	tone: IncidentTone;
	description: string;
}

const INCIDENT_STATES: Record<string, IncidentStateView> = {
	pending: {
		label: 'Pending',
		tone: 'warning',
		description: 'The condition is holding but has not yet met the for-duration.'
	},
	firing: {
		label: 'Firing',
		tone: 'danger',
		description: 'The condition held for the for-duration; the incident is active.'
	},
	resolved: {
		label: 'Resolved',
		tone: 'success',
		description: 'The condition cleared for the recovery duration.'
	}
};

export function incidentStateView(status: string): IncidentStateView {
	return INCIDENT_STATES[status] ?? {
		label: status || 'Unknown',
		tone: 'warning',
		description: 'Off-vocabulary incident status.'
	};
}

/** Badge variant for an incident/rule severity. */
export function severityBadgeVariant(
	severity: string
): 'danger' | 'warning' | 'info' | 'default' {
	switch (severity) {
		case 'critical':
			return 'danger';
		case 'warning':
			return 'warning';
		case 'info':
			return 'info';
		default:
			return 'default';
	}
}

/** Deep link to a target's detail page (its history lives there). */
export function targetHref(targetKind: string, targetId: string): string {
	if (targetKind === 'node') return `/nodes/${encodeURIComponent(targetId)}`;
	if (targetKind === 'vm') return `/vms/${encodeURIComponent(targetId)}`;
	return '#';
}

// ---------------------------------------------------------------------------
// Rule spec summaries
// ---------------------------------------------------------------------------

function operatorSymbol(operator?: ThresholdOperator): string {
	return operator === 'less_than' ? '<' : '>';
}

/** Byte-ish magnitudes render as bytes; everything else stays numeric. */
function formatThresholdValue(value: number): string {
	if (Number.isInteger(value) && Math.abs(value) >= 1_000_000) {
		const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
		let scaled = value;
		let unit = 0;
		while (Math.abs(scaled) >= 1024 && unit < units.length - 1) {
			scaled /= 1024;
			unit += 1;
		}
		return `${Number.isInteger(scaled) ? scaled : scaled.toFixed(1)} ${units[unit]}`;
	}
	return String(value);
}

function dimensionSuffix(dimensionMatch?: Record<string, string>): string {
	if (!dimensionMatch) return '';
	const parts = Object.entries(dimensionMatch).map(([key, value]) => `${key}=${value}`);
	return ` (${parts.join(', ')})`;
}

function conditionSummary(condition: AlertRule): string {
	// A group's nested conditions carry NO `rule_type` on the wire
	// (the store serializes specs untagged; `rule_type` is derived
	// and added only at the top level by rule_wire) — so shape-sniff
	// the child instead of reading a field that is only present on
	// top-level rules.
	const kind =
		condition.rule_type ??
		(condition.threshold !== undefined
			? 'threshold'
			: condition.threshold_per_second !== undefined
				? 'rate'
				: condition.check_id !== undefined
					? 'check_status'
					: condition.conditions !== undefined
						? 'group'
						: 'availability');
	switch (kind) {
		case 'threshold':
			return `${condition.metric_id ?? '?'} ${operatorSymbol(condition.operator)} ${formatThresholdValue(condition.threshold ?? 0)}${dimensionSuffix(condition.dimension_match)}`;
		case 'rate':
			return `${condition.metric_id ?? '?'} ${operatorSymbol(condition.operator)} ${condition.threshold_per_second ?? 0}/s over ${condition.window_seconds ?? 0}s`;
		case 'availability':
			return `no data: ${condition.metric_id ?? '?'}${dimensionSuffix(condition.dimension_match)}`;
		case 'check_status':
			return `${condition.check_id ?? '?'} is ${condition.status_match ?? '?'}`;
		case 'group':
			return (condition.conditions ?? [])
				.map((child) => conditionSummary(child))
				.join(` ${(condition.op ?? 'and').toUpperCase()} `);
		default:
			return kind;
	}
}

/**
 * Human-readable one-line summary of a rule's typed spec, e.g.
 * `vm.cpu.capacity_ratio > 0.9` or `service:nginx.service is critical`.
 */
export function ruleSpecSummary(rule: AlertRule): string {
	return conditionSummary(rule);
}

// ---------------------------------------------------------------------------
// Silence quick picks
// ---------------------------------------------------------------------------

export interface SilencePick {
	label: string;
	minutes: number;
}

/** The operator's quick silence durations (bounded 1..=10080 minutes server-side). */
export const SILENCE_PICKS: SilencePick[] = [
	{ label: '30m', minutes: 30 },
	{ label: '2h', minutes: 120 },
	{ label: '24h', minutes: 1440 }
];

/** "2h left" while active; an expired silence is gone, never negative. */
export function silenceRemainingLabel(untilMs: number | null | undefined, nowMs: number): string {
	if (untilMs === null || untilMs === undefined || !Number.isFinite(untilMs)) return '';
	const seconds = Math.max(0, Math.floor((untilMs - nowMs) / 1000));
	if (seconds === 0) return 'expired';
	if (seconds < 60) return `${seconds}s left`;
	const minutes = Math.floor(seconds / 60);
	if (minutes < 60) return `${minutes}m left`;
	const hours = Math.floor(minutes / 60);
	if (hours < 24) return `${hours}h left`;
	return `${Math.floor(hours / 24)}d left`;
}

// ---------------------------------------------------------------------------
// Timestamps
// ---------------------------------------------------------------------------

/** Compact absolute timestamp for ISO strings and epoch millis. */
export function formatAlertTimestamp(
	value: string | number | null | undefined
): string {
	if (value === null || value === undefined || value === '') return '—';
	const date = typeof value === 'number' ? new Date(value) : new Date(value);
	const time = date.getTime();
	if (!Number.isFinite(time)) return '—';
	return date.toLocaleString('en-US', {
		month: 'short',
		day: 'numeric',
		hour: 'numeric',
		minute: '2-digit'
	});
}

// ---------------------------------------------------------------------------
// Starter rule templates
// ---------------------------------------------------------------------------

export interface RuleTemplateSpec {
	metric_id?: string;
	dimension_match?: Record<string, string>;
	operator?: ThresholdOperator;
	threshold?: number;
	threshold_per_second?: number;
	window_seconds?: number;
	check_id?: string;
	status_match?: CheckStatusMatch;
}

export interface RuleTemplate {
	id: string;
	title: string;
	description: string;
	target_kind: 'node' | 'vm';
	rule_type: Exclude<RuleType, 'group'>;
	severity: AlertSeverity;
	for_seconds: number;
	recovery_seconds: number;
	missing_data: MissingDataPolicy;
	spec: RuleTemplateSpec;
}

/**
 * The seven starter templates (prompt 05 task 6). Templates are
 * UI-side pre-fills ONLY: the dialog never auto-enables a created
 * rule — the operator must review the pre-filled values and flip the
 * enabled switch explicitly.
 */
export const RULE_TEMPLATES: RuleTemplate[] = [
	{
		id: 'node-unreachable',
		title: 'Node unreachable',
		description:
			'Availability on the node OS CPU capacity series — fires when the node stops reporting (stale or absent samples).',
		target_kind: 'node',
		rule_type: 'availability',
		severity: 'critical',
		for_seconds: 180,
		recovery_seconds: 300,
		missing_data: 'fire',
		spec: { metric_id: 'node.cpu.capacity_ratio' }
	},
	{
		id: 'collector-stale',
		title: 'Collector stale',
		description:
			'Availability on the node load1 series — fires when the node OS collector has not reported recently.',
		target_kind: 'node',
		rule_type: 'availability',
		severity: 'warning',
		for_seconds: 300,
		recovery_seconds: 300,
		missing_data: 'fire',
		spec: { metric_id: 'node.cpu.load1' }
	},
	{
		id: 'vm-cpu-pressure',
		title: 'VM CPU pressure',
		description: 'Threshold — the VM CPU capacity ratio stays above 90% for 5 minutes.',
		target_kind: 'vm',
		rule_type: 'threshold',
		severity: 'warning',
		for_seconds: 300,
		recovery_seconds: 120,
		missing_data: 'unknown',
		spec: { metric_id: 'vm.cpu.capacity_ratio', operator: 'greater_than', threshold: 0.9 }
	},
	{
		id: 'vm-storage-near-full',
		title: 'VM storage near full',
		description:
			'Threshold on the guest filesystem free bytes — fires when the mount has less than 2 GiB available. Adjust the mount_id dimension to the filesystem that backs this VM.',
		target_kind: 'vm',
		rule_type: 'threshold',
		severity: 'warning',
		for_seconds: 300,
		recovery_seconds: 120,
		missing_data: 'unknown',
		spec: {
			metric_id: 'vm.guest.fs.available_bytes',
			dimension_match: { mount_id: 'ext4:/' },
			operator: 'less_than',
			threshold: 2_147_483_648
		}
	},
	{
		id: 'agent-disconnected',
		title: 'Agent disconnected',
		description:
			'Availability on the derived guest-agent liveness series — fires when the enrolled agent has not been seen.',
		target_kind: 'vm',
		rule_type: 'availability',
		severity: 'warning',
		for_seconds: 300,
		recovery_seconds: 300,
		missing_data: 'fire',
		spec: { metric_id: 'monitoring.agent.last_seen_age_seconds' }
	},
	{
		id: 'guest-fs-inodes-full',
		title: 'Guest filesystem full (inodes)',
		description:
			'Threshold on the guest filesystem inode utilization — fires when a mount has consumed 90% of its inodes. Adjust the mount_id dimension to the filesystem that matters.',
		target_kind: 'vm',
		rule_type: 'threshold',
		severity: 'warning',
		for_seconds: 300,
		recovery_seconds: 120,
		missing_data: 'unknown',
		spec: {
			metric_id: 'vm.guest.fs.inodes_utilization_ratio',
			dimension_match: { mount_id: 'ext4:/' },
			operator: 'greater_than',
			threshold: 0.9
		}
	},
	{
		id: 'guest-service-down',
		title: 'Guest service down',
		description:
			'Check status — fires when the named guest service check reports critical. Replace the check id with a real check on this VM (service:, http:, tcp: or plugin: namespace).',
		target_kind: 'vm',
		rule_type: 'check_status',
		severity: 'critical',
		for_seconds: 60,
		recovery_seconds: 120,
		missing_data: 'unknown',
		spec: { check_id: 'service:nginx.service', status_match: 'critical' }
	}
];
