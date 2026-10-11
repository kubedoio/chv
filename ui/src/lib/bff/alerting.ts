/**
 * Native alerting API (query/alerts contract v1, #602 PR-6).
 *
 * These calls hit the authenticated BFF only. The shapes mirror the
 * BFF handlers in `chv-webui-bff/src/handlers/alerts.rs`:
 *
 * - Incidents live in the operational `alerts` table
 *   (`source = 'monitoring'`); `pending` is always visible
 *   (pre-notification, not pre-visibility).
 * - Rules use the contract's FLAT wire shape: common fields plus the
 *   typed spec fields at the top level. `rule_type` is derived from
 *   the spec shape and returned; clients never send it.
 * - Rule mutations carry a revision precondition; a mismatch is a
 *   409 conflict and the client must reload.
 * - Acknowledgment and silence are overlays: they never resolve an
 *   incident and never stop firing/resolved notifications.
 */

import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';

// ---------------------------------------------------------------------------
// Wire vocabulary
// ---------------------------------------------------------------------------

export type IncidentStatus = 'pending' | 'firing' | 'resolved';
export type AlertSeverity = 'critical' | 'warning' | 'info';
export type RuleType = 'threshold' | 'rate' | 'availability' | 'check_status' | 'group';
export type MissingDataPolicy = 'unknown' | 'fire' | 'ignore';
export type ThresholdOperator = 'greater_than' | 'less_than';
export type CheckStatusMatch = 'critical' | 'warning' | 'unknown';

/** One monitoring incident (the operational `alerts` row, snake_case). */
export interface Incident {
	alert_id: string;
	status: IncidentStatus;
	severity: AlertSeverity;
	rule_id: string | null;
	rule_revision: number | null;
	dedup_key: string | null;
	target_kind: string;
	target_id: string;
	node_id: string | null;
	message: string;
	/** Rendered measurement, redacted (e.g. "0.94 (vm.cpu.capacity_ratio)"). */
	last_observed: string | null;
	opened_at: string;
	acknowledged_at: string | null;
	acknowledged_by: string | null;
	silenced_until_ms: number | null;
	silenced_by: string | null;
	pending_since_ms: number | null;
	first_occurrence_ms: number | null;
	last_occurrence_ms: number | null;
	evidence_from_ms: number | null;
	evidence_to_ms: number | null;
	resolved_at: string | null;
}

/** One incident state transition (audit/history row). */
export interface IncidentTransition {
	/** Null for creation. */
	from_state: string | null;
	to_state: string;
	occurred_at_ms: number;
	reason: string;
	measured: string | null;
}

/** One notification outbox event (delivery audit row, newest first). */
export interface AlertDelivery {
	event_id: string;
	alert_id: string;
	event_type: 'firing' | 'resolved' | 'acknowledged' | 'delivery_failed' | 'test' | string;
	severity: string;
	target_kind: string;
	target_id: string;
	summary: string;
	channel: string;
	status: 'pending' | 'delivered' | 'dead' | string;
	attempts: number;
	next_attempt_at_ms: number;
	last_attempt_ms: number | null;
	last_response: string | null;
	occurred_at_ms: number;
	updated_at_ms: number;
}

/**
 * A rule in the contract's flat wire shape: common fields plus the
 * typed spec fields at the top level (which spec fields are present
 * depends on `rule_type`).
 */
export interface AlertRule {
	rule_id: string;
	name: string;
	enabled: boolean;
	target_kind: 'node' | 'vm';
	target_id: string;
	rule_type: RuleType;
	severity: AlertSeverity;
	for_seconds: number;
	recovery_seconds: number;
	missing_data: MissingDataPolicy;
	revision: number;
	created_by: string;
	created_at_ms: number;
	updated_at_ms: number;
	// ---- flat spec fields (presence follows rule_type) ----
	metric_id?: string;
	dimension_match?: Record<string, string>;
	operator?: ThresholdOperator;
	threshold?: number;
	threshold_per_second?: number;
	window_seconds?: number;
	check_id?: string;
	status_match?: CheckStatusMatch;
	op?: 'and' | 'or';
	conditions?: AlertRuleCondition[];
}

/**
 * A group rule's nested condition, as it actually rides the wire: the
 * store serializes spec children UNTAGGED (no `rule_type`, none of
 * the common rule fields — `rule_wire` adds those only at the top
 * level). Typing them as `AlertRule` would let future code read
 * `child.name` / `child.revision` and silently get `undefined`.
 */
export interface AlertRuleCondition {
	metric_id?: string;
	dimension_match?: Record<string, string>;
	operator?: ThresholdOperator;
	threshold?: number;
	threshold_per_second?: number;
	window_seconds?: number;
	check_id?: string;
	status_match?: CheckStatusMatch;
	/** The store rejects nested groups; kept for the sniff's totality. */
	op?: 'and' | 'or';
	conditions?: AlertRuleCondition[];
}

// ---------------------------------------------------------------------------
// Request bodies
// ---------------------------------------------------------------------------

export interface ListIncidentsBody {
	status?: IncidentStatus;
	target_kind?: 'node' | 'vm';
	target_id?: string;
	rule_id?: string;
	include_resolved?: boolean;
	limit?: number;
	offset?: number;
}

export interface ListRulesBody {
	enabled_only?: boolean;
	target_kind?: 'node' | 'vm';
	limit?: number;
	offset?: number;
}

/**
 * Create/update body: common rule fields plus the typed spec fields
 * flat in the same object (the contract example shape). The BFF
 * strips the common keys and parses the remainder as the spec.
 */
export interface RuleMutateBody {
	/** Update only. */
	rule_id?: string;
	/** Update only; a mismatch is a 409 conflict. */
	expected_revision?: number;
	name: string;
	target_kind: 'node' | 'vm';
	target_id: string;
	severity: AlertSeverity;
	for_seconds?: number;
	recovery_seconds?: number;
	missing_data?: MissingDataPolicy;
	enabled?: boolean;
	// ---- flat spec fields ----
	metric_id?: string;
	dimension_match?: Record<string, string>;
	operator?: ThresholdOperator;
	threshold?: number;
	threshold_per_second?: number;
	window_seconds?: number;
	check_id?: string;
	status_match?: CheckStatusMatch;
	op?: 'and' | 'or';
	conditions?: AlertRuleCondition[];
}

// ---------------------------------------------------------------------------
// Viewer reads
// ---------------------------------------------------------------------------

export async function listIncidents(
	body?: ListIncidentsBody,
	token?: string
): Promise<{ incidents: Incident[]; total: number }> {
	return bffFetch(BFFEndpoints.monitoringAlerts, {
		method: 'POST',
		body: JSON.stringify(body ?? {}),
		token
	});
}

export async function incidentDetail(
	body: { alert_id: string },
	token?: string
): Promise<{ incident: Incident; transitions: IncidentTransition[] }> {
	return bffFetch(BFFEndpoints.monitoringAlertDetail, {
		method: 'POST',
		body: JSON.stringify(body),
		token
	});
}

export async function listRules(
	body?: ListRulesBody,
	token?: string
): Promise<{ rules: AlertRule[]; total: number }> {
	return bffFetch(BFFEndpoints.monitoringAlertRules, {
		method: 'POST',
		body: JSON.stringify(body ?? {}),
		token
	});
}

export async function listDeliveries(
	body?: { limit?: number },
	token?: string
): Promise<{ deliveries: AlertDelivery[] }> {
	return bffFetch(BFFEndpoints.monitoringAlertDeliveries, {
		method: 'POST',
		body: JSON.stringify(body ?? {}),
		token
	});
}

// ---------------------------------------------------------------------------
// Operator mutations
// ---------------------------------------------------------------------------

export async function createRule(
	body: RuleMutateBody,
	token?: string
): Promise<{ rule: AlertRule }> {
	return bffFetch(BFFEndpoints.monitoringAlertRuleCreate, {
		method: 'POST',
		body: JSON.stringify(body),
		token
	});
}

export async function updateRule(
	body: RuleMutateBody,
	token?: string
): Promise<{ rule: AlertRule }> {
	return bffFetch(BFFEndpoints.monitoringAlertRuleUpdate, {
		method: 'POST',
		body: JSON.stringify(body),
		token
	});
}

export async function deleteRule(
	body: { rule_id: string; expected_revision: number },
	token?: string
): Promise<{ deleted: true; retired_incidents: number }> {
	return bffFetch(BFFEndpoints.monitoringAlertRuleDelete, {
		method: 'POST',
		body: JSON.stringify(body),
		token
	});
}

export async function acknowledgeIncident(
	body: { alert_id: string },
	token?: string
): Promise<{ acknowledged: true }> {
	return bffFetch(BFFEndpoints.monitoringAlertAcknowledge, {
		method: 'POST',
		body: JSON.stringify(body),
		token
	});
}

export async function silenceIncident(
	body: { alert_id: string; duration_minutes?: number; until_ms?: number },
	token?: string
): Promise<{ silenced: true; until_ms: number }> {
	return bffFetch(BFFEndpoints.monitoringAlertSilence, {
		method: 'POST',
		body: JSON.stringify(body),
		token
	});
}
