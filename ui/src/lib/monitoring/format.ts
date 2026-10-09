/**
 * Honest formatting for native monitoring values (#602 PR-2).
 *
 * Rules that keep every rendered number true to its source:
 * - Units come from the metric registry (`unit` on each series), never
 *   guessed from magnitude: bytes are binary (KiB/MiB/GiB/TiB), ratios
 *   render as percent of 1, cores stay cores.
 * - Counters render as rates (delta over the point's own window) — a
 *   cumulative byte count shown raw would be meaningless to an
 *   operator scanning a chart.
 * - Missing data renders as "—" with its quality/reason, never 0.
 */

/** Binary byte formatting: 1024-based, exact for values < 2^53. */
export function formatBytes(bytes: number): string {
	if (!Number.isFinite(bytes)) return '—';
	const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB'];
	let value = bytes;
	let unit = 0;
	while (Math.abs(value) >= 1024 && unit < units.length - 1) {
		value /= 1024;
		unit += 1;
	}
	const digits = Math.abs(value) >= 100 || unit === 0 ? 0 : 1;
	return `${value.toFixed(digits)} ${units[unit]}`;
}

/** A 0..1 ratio as a percent (registry unit `ratio`). */
export function formatRatio(ratio: number): string {
	if (!Number.isFinite(ratio)) return '—';
	return `${(ratio * 100).toFixed(1)}%`;
}

/** vCPU-seconds per second (registry unit `cores`). */
export function formatCores(cores: number): string {
	if (!Number.isFinite(cores)) return '—';
	return `${cores.toFixed(2)} cores`;
}

/** A counter rate from a decimal-string integer delta and its window. */
export function formatCounterRate(
	integerValue: string | undefined,
	windowMs: number
): number | null {
	if (integerValue === undefined || windowMs <= 0) return null;
	const delta = Number(integerValue);
	if (!Number.isFinite(delta)) return null;
	return delta / (windowMs / 1000);
}

/** Render a value for the series' unit; null when not renderable. */
export function formatValue(
	value: number | undefined,
	unit: string,
	kind: string
): string {
	if (value === undefined || !Number.isFinite(value)) return '—';
	switch (unit) {
		case 'bytes':
			return kind === 'counter' ? `${formatBytes(value)}/s` : formatBytes(value);
		case 'bytes_per_second':
			return `${formatBytes(value)}/s`;
		case 'ratio':
			return formatRatio(value);
		case 'cores':
			return formatCores(value);
		case 'count':
		case 'operations':
			return new Intl.NumberFormat().format(value);
		case 'seconds':
			return `${formatDuration(value)}`;
		case 'celsius':
			return `${value.toFixed(1)} °C`;
		default:
			return `${value.toFixed(2)}`;
	}
}

function formatDuration(seconds: number): string {
	if (seconds < 1) return `${(seconds * 1000).toFixed(0)} ms`;
	if (seconds < 60) return `${seconds.toFixed(1)} s`;
	const minutes = Math.floor(seconds / 60);
	if (minutes < 60) return `${minutes}m ${Math.floor(seconds % 60)}s`;
	const hours = Math.floor(minutes / 60);
	return `${hours}h ${minutes % 60}m`;
}

/** Compact age ("12s ago", "3m ago", "2h ago") from unix ms. */
export function formatAge(unixMs: number | undefined | null, nowMs: number): string {
	if (!unixMs || unixMs <= 0) return 'never';
	const seconds = Math.max(0, Math.floor((nowMs - unixMs) / 1000));
	if (seconds < 60) return `${seconds}s ago`;
	const minutes = Math.floor(seconds / 60);
	if (minutes < 60) return `${minutes}m ago`;
	const hours = Math.floor(minutes / 60);
	if (hours < 24) return `${hours}h ago`;
	return `${Math.floor(hours / 24)}d ago`;
}

/** Friendly source label: "node_os" → "Node OS". */
export function sourceLabel(source: string | null): string {
	if (!source) return 'no source';
	switch (source) {
		case 'node_os':
			return 'Node OS';
		case 'vmm':
			return 'VMM';
		case 'vm_cgroup':
			return 'VM cgroup';
		case 'storage_provider':
			return 'Storage provider';
		case 'network_provider':
			return 'Network provider';
		case 'guest_agent':
			return 'Guest agent';
		case 'derived':
			return 'Derived';
		default:
			return source;
	}
}

/** Friendly quality label with the honesty preserved. */
export function qualityLabel(quality: string): string {
	switch (quality) {
		case 'valid':
			return 'valid';
		case 'insufficient_samples':
			return 'insufficient samples';
		case 'unsupported':
			return 'unsupported';
		case 'unavailable':
			return 'unavailable';
		case 'invalid':
			return 'invalid';
		case 'stale':
			return 'stale';
		default:
			return quality;
	}
}

/** Friendly series-absence label. */
export function reasonLabel(reason: string | null | undefined): string {
	switch (reason) {
		case 'unsupported':
			return 'Not collected on this target';
		case 'not_collected':
			return 'No data collected yet';
		case 'no_history':
			return 'No data in range';
		case 'stale':
			return 'Data is stale';
		default:
			return 'No data';
	}
}

/** Metric title: "vm.cpu.cores_used" → "CPU Cores Used". */
export function metricTitle(metricId: string): string {
	const words = metricId
		.split(/[._]/)
		.slice(1)
		.map((w) => (['cpu', 'fs', 'psi', 'net', 'vm'].includes(w) ? w.toUpperCase() : w))
		.map((w) => (w.length > 0 ? w[0].toUpperCase() + w.slice(1) : w))
		.join(' ');
	return words || metricId;
}
