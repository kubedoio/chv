/**
 * Native monitoring read API (query/alerts contract v1, #602).
 *
 * These calls hit the authenticated BFF only — the browser never talks
 * to agents, the VMM, or a monitoring store directly. The shapes mirror
 * the BFF handlers in `chv-webui-bff/src/handlers/monitoring.rs`:
 *
 * - Missing data is never zero: a non-valid point carries `quality` and
 *   no `value`; a series without stored data carries a `reason`.
 * - Counter integer values arrive as decimal strings (JSON numbers
 *   lose precision beyond 2^53) — parse with care, display exactly.
 */

import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';

/** One point of a history series. Valid points carry a value. */
export interface MonitoringPoint {
	timestamp_ms: number;
	window_ms: number;
	quality: string;
	value?: number;
	integer_value?: string;
}

/** One series of a history response. */
export interface MonitoringSeries {
	metric_id: string;
	source: string | null;
	dimensions: Record<string, string>;
	kind: string;
	unit: string;
	points: MonitoringPoint[];
	coverage_ratio: number;
	/** Why the series has no stored data (absence vocabulary). */
	reason?: string | null;
}

export interface MonitoringHistoryResponse {
	schema_version: number;
	target_kind: string;
	target_id: string;
	series: MonitoringSeries[];
	generated_at_ms: number;
	truncated: boolean;
}

/** The latest sample of one series. */
export interface MonitoringCurrentSample {
	metric_id: string;
	source: string;
	dimensions: Record<string, string>;
	kind: string;
	unit: string;
	observed_at_ms: number;
	received_at_ms: number;
	quality: string;
	stale: boolean;
	value?: number;
	integer_value?: string;
}

export interface MonitoringCurrentResponse {
	schema_version: number;
	target_kind: string;
	target_id: string;
	samples: MonitoringCurrentSample[];
	generated_at_ms: number;
}

export interface MonitoringOverviewTarget {
	target_id: string;
	samples: MonitoringCurrentSample[];
	age_seconds?: number | null;
}

export interface MonitoringOverviewResponse {
	schema_version: number;
	target_kind: string;
	targets: MonitoringOverviewTarget[];
	generated_at_ms: number;
}

export interface MonitoringCatalogMetric {
	metric_id: string;
	kind: string;
	unit: string;
	/** Allowed sources in preference order (most authoritative first). */
	sources: string[];
	dimensions: string[];
}

export interface MonitoringCatalogResponse {
	schema_version: number;
	metrics: MonitoringCatalogMetric[];
	qualities: string[];
	series_reasons: string[];
}

export interface MonitoringHealthResponse {
	schema_version: number;
	available: boolean;
	degraded_reason?: string | null;
	last_ingest_at_ms?: number | null;
	last_maintenance_at_ms?: number | null;
	accepted_batches?: number;
	duplicate_batches?: number;
	rejected_batches?: number;
	unavailable_batches?: number;
	headroom_bytes?: number | null;
	/** True when the last headroom probe failed (floor unverified). */
	headroom_probe_failed?: boolean;
	raw_samples?: number;
	generated_at_ms: number;
}

/** The view-selection windows (PR-2): 1h/6h/24h/7d/30d. */
export type MonitoringTimeRange = '1h' | '6h' | '24h' | '7d' | '30d';

export const MONITORING_TIME_RANGES: MonitoringTimeRange[] = [
	'1h',
	'6h',
	'24h',
	'7d',
	'30d'
];

const RANGE_MS: Record<MonitoringTimeRange, number> = {
	'1h': 60 * 60 * 1000,
	'6h': 6 * 60 * 60 * 1000,
	'24h': 24 * 60 * 60 * 1000,
	'7d': 7 * 24 * 60 * 60 * 1000,
	'30d': 30 * 24 * 60 * 60 * 1000
};

/** The `[from_ms, to_ms]` window for one view selection. */
export function monitoringRangeWindow(range: MonitoringTimeRange): {
	from_ms: number;
	to_ms: number;
} {
	const to_ms = Date.now();
	return { from_ms: to_ms - RANGE_MS[range], to_ms };
}

export async function getMonitoringCatalog(
	token?: string
): Promise<MonitoringCatalogResponse> {
	return bffFetch<MonitoringCatalogResponse>(BFFEndpoints.monitoringCatalog, {
		method: 'GET',
		token
	});
}

export async function getMonitoringHealth(
	token?: string
): Promise<MonitoringHealthResponse> {
	return bffFetch<MonitoringHealthResponse>(BFFEndpoints.monitoringHealth, {
		method: 'GET',
		token
	});
}

export async function fetchMonitoringCurrent(
	targetKind: 'node' | 'vm',
	targetId: string,
	metricIds: string[],
	token?: string
): Promise<MonitoringCurrentResponse> {
	return bffFetch<MonitoringCurrentResponse>(BFFEndpoints.monitoringCurrent, {
		method: 'POST',
		body: JSON.stringify({
			target_kind: targetKind,
			target_id: targetId,
			metric_ids: metricIds
		}),
		token
	});
}

export async function fetchMonitoringHistory(
	targetKind: 'node' | 'vm',
	targetId: string,
	metricIds: string[],
	range: MonitoringTimeRange,
	token?: string
): Promise<MonitoringHistoryResponse> {
	const { from_ms, to_ms } = monitoringRangeWindow(range);
	return bffFetch<MonitoringHistoryResponse>(BFFEndpoints.monitoringHistory, {
		method: 'POST',
		body: JSON.stringify({
			target_kind: targetKind,
			target_id: targetId,
			metric_ids: metricIds,
			from_ms,
			to_ms
		}),
		token
	});
}

export async function fetchMonitoringOverview(
	targetKind: 'node' | 'vm',
	targetIds: string[],
	metricIds: string[],
	token?: string
): Promise<MonitoringOverviewResponse> {
	return bffFetch<MonitoringOverviewResponse>(BFFEndpoints.monitoringOverview, {
		method: 'POST',
		body: JSON.stringify({
			target_kind: targetKind,
			target_ids: targetIds,
			metric_ids: metricIds
		}),
		token
	});
}
