/**
 * Presentation helpers for guest-side telemetry surfaces (campaign
 * #602 G4): check-status vocabulary, namespaced-id parsing, and
 * dimension grouping for the filesystem/process collector series.
 * Pure functions — tested in `guestChecks.test.ts` — so the guest
 * cards stay under the size budget and the vocabulary stays
 * single-sourced.
 */

import type {
	MonitoringCheck,
	MonitoringCurrentSample
} from '#lib/bff/monitoring.ts';

// ---------------------------------------------------------------------------
// Check status vocabulary
// ---------------------------------------------------------------------------

export type CheckStatus = 'ok' | 'warning' | 'critical' | 'unknown';

export interface CheckStatusView {
	label: string;
	/** Tone token shared with the shell's status vocabulary. */
	tone: 'success' | 'warning' | 'danger' | 'muted';
	description: string;
}

const STATUS_VIEWS: Record<CheckStatus, CheckStatusView> = {
	ok: {
		label: 'OK',
		tone: 'success',
		description: 'The check ran and passed.'
	},
	warning: {
		label: 'Warning',
		tone: 'warning',
		description: 'The check ran and reported a warning.'
	},
	critical: {
		label: 'Critical',
		tone: 'danger',
		description: 'The check ran and reported a failure.'
	},
	unknown: {
		label: 'Unknown',
		tone: 'muted',
		// Unknown is NEVER healthy: it means the check could not run
		// or its result is not known — never "passed".
		description: 'The check could not run or its result is not known.'
	}
};

export function checkStatusView(status: string): CheckStatusView {
	return STATUS_VIEWS[status as CheckStatus] ?? STATUS_VIEWS.unknown;
}

/** The `check.status` series encodes status as 0..3; else → unknown. */
export function checkStatusFromCode(code: number): CheckStatus {
	switch (code) {
		case 0:
			return 'ok';
		case 1:
			return 'warning';
		case 2:
			return 'critical';
		case 3:
			return 'unknown';
		default:
			return 'unknown';
	}
}

// ---------------------------------------------------------------------------
// Namespaced id parsing
// ---------------------------------------------------------------------------

const CHECK_KIND_LABELS: Record<string, string> = {
	service: 'Service',
	http: 'HTTP check',
	tcp: 'TCP check',
	plugin: 'Plugin'
};

export interface CheckIdParts {
	kind: string;
	/** Human label; an unknown kind falls back to the raw prefix. */
	kindLabel: string;
	name: string;
}

/** `service:nginx.service` → kind "service", name "nginx.service". */
export function parseCheckId(checkId: string): CheckIdParts {
	const colon = checkId.indexOf(':');
	if (colon === -1) {
		return { kind: '', kindLabel: 'Check', name: checkId };
	}
	const kind = checkId.slice(0, colon);
	return {
		kind,
		kindLabel: CHECK_KIND_LABELS[kind] ?? kind,
		name: checkId.slice(colon + 1)
	};
}

export interface MountIdParts {
	fstype: string;
	mountpoint: string;
}

/** `ext4:/` → fstype "ext4", mountpoint "/". */
export function parseMountId(mountId: string): MountIdParts {
	const colon = mountId.indexOf(':');
	if (colon === -1) {
		return { fstype: '', mountpoint: mountId };
	}
	return {
		fstype: mountId.slice(0, colon),
		mountpoint: mountId.slice(colon + 1)
	};
}

const INTERFACE_CLASS_LABELS: Record<string, string> = {
	phys: 'Physical',
	virt: 'Virtual',
	bridge: 'Bridge',
	loopback: 'Loopback',
	other: 'Other'
};

export interface InterfaceIdParts {
	ifClass: string;
	/** Human label; an unknown class falls back to the raw prefix. */
	classLabel: string;
	name: string;
}

/** `phys:eth0` → ifClass "phys", name "eth0". */
export function parseInterfaceId(interfaceId: string): InterfaceIdParts {
	const colon = interfaceId.indexOf(':');
	if (colon === -1) {
		return { ifClass: '', classLabel: '', name: interfaceId };
	}
	const ifClass = interfaceId.slice(0, colon);
	return {
		ifClass,
		classLabel: INTERFACE_CLASS_LABELS[ifClass] ?? ifClass,
		name: interfaceId.slice(colon + 1)
	};
}

// ---------------------------------------------------------------------------
// Age formatting
// ---------------------------------------------------------------------------

/** Compact age ("42s ago", "3m ago"); absence is honest, never "0s ago". */
export function checkAgeLabel(
	observedAtMs: number | null | undefined,
	nowMs: number
): string {
	if (
		observedAtMs === null ||
		observedAtMs === undefined ||
		!Number.isFinite(observedAtMs) ||
		observedAtMs <= 0
	) {
		return 'never observed';
	}
	const seconds = Math.max(0, Math.floor((nowMs - observedAtMs) / 1000));
	if (seconds < 60) return `${seconds}s ago`;
	const minutes = Math.floor(seconds / 60);
	if (minutes < 60) return `${minutes}m ago`;
	const hours = Math.floor(minutes / 60);
	if (hours < 24) return `${hours}h ago`;
	return `${Math.floor(hours / 24)}d ago`;
}

// ---------------------------------------------------------------------------
// Dimension grouping (guest collector series)
// ---------------------------------------------------------------------------

/** The latest sample's numeric value; missing data is null, never 0. */
function sampleNumber(sample: MonitoringCurrentSample | undefined): number | null {
	if (!sample) return null;
	if (sample.value !== undefined && Number.isFinite(sample.value)) {
		return sample.value;
	}
	if (sample.integer_value !== undefined) {
		// Integer values arrive as decimal strings (JSON numbers lose
		// precision beyond 2^53); parse exactly, display via formatBytes.
		const parsed = Number(sample.integer_value);
		if (Number.isFinite(parsed)) return parsed;
	}
	return null;
}

export const GUEST_FS_METRIC_IDS = [
	'vm.guest.fs.available_bytes',
	'vm.guest.fs.total_bytes',
	'vm.guest.fs.inodes_utilization_ratio',
	'vm.guest.fs.read_only'
] as const;

export interface GuestFsRow {
	mountId: string;
	fstype: string;
	mountpoint: string;
	totalBytes: number | null;
	availableBytes: number | null;
	usedBytes: number | null;
	/** 1 − available/total; null unless both byte samples are valid. */
	usageRatio: number | null;
	inodeRatio: number | null;
	/** True when the read_only sample is 1; null when absent. */
	readOnly: boolean | null;
	/** True when any sample of this mount is past the staleness window. */
	stale: boolean;
}

/** Group the fs collector series into one row per `mount_id` dimension. */
export function guestFsRows(samples: MonitoringCurrentSample[]): GuestFsRow[] {
	interface Bucket {
		available?: MonitoringCurrentSample;
		total?: MonitoringCurrentSample;
		inodes?: MonitoringCurrentSample;
		readOnly?: MonitoringCurrentSample;
	}
	const buckets = new Map<string, Bucket>();
	for (const sample of samples) {
		const mountId = sample.dimensions?.['mount_id'];
		if (!mountId) continue;
		const bucket = buckets.get(mountId) ?? {};
		switch (sample.metric_id) {
			case 'vm.guest.fs.available_bytes':
				bucket.available = sample;
				break;
			case 'vm.guest.fs.total_bytes':
				bucket.total = sample;
				break;
			case 'vm.guest.fs.inodes_utilization_ratio':
				bucket.inodes = sample;
				break;
			case 'vm.guest.fs.read_only':
				bucket.readOnly = sample;
				break;
		}
		buckets.set(mountId, bucket);
	}
	const rows: GuestFsRow[] = [];
	for (const [mountId, bucket] of buckets) {
		const availableBytes = sampleNumber(bucket.available);
		const totalBytes = sampleNumber(bucket.total);
		const usedBytes =
			availableBytes !== null && totalBytes !== null && totalBytes >= availableBytes
				? totalBytes - availableBytes
				: null;
		const usageRatio =
			availableBytes !== null && totalBytes !== null && totalBytes > 0
				? 1 - availableBytes / totalBytes
				: null;
		const readOnlyValue = sampleNumber(bucket.readOnly);
		rows.push({
			mountId,
			...parseMountId(mountId),
			totalBytes,
			availableBytes,
			usedBytes,
			usageRatio,
			inodeRatio: sampleNumber(bucket.inodes),
			readOnly: readOnlyValue === null ? null : readOnlyValue === 1,
			stale: [bucket.available, bucket.total, bucket.inodes, bucket.readOnly].some(
				(s) => s?.stale === true
			)
		});
	}
	rows.sort((a, b) => a.mountpoint.localeCompare(b.mountpoint));
	return rows;
}

export const GUEST_PROCESS_METRIC_IDS = [
	'vm.guest.process.count',
	'vm.guest.process.cpu_utilization_ratio',
	'vm.guest.process.rss_bytes'
] as const;

export interface GuestProcessRow {
	selector: string;
	count: number | null;
	/** 0..1 ratio of one logical core. */
	cpuRatio: number | null;
	rssBytes: number | null;
	stale: boolean;
}

/** Group the process collector series by `process_selector` dimension. */
export function guestProcessRows(
	samples: MonitoringCurrentSample[]
): GuestProcessRow[] {
	interface Bucket {
		count?: MonitoringCurrentSample;
		cpu?: MonitoringCurrentSample;
		rss?: MonitoringCurrentSample;
	}
	const buckets = new Map<string, Bucket>();
	for (const sample of samples) {
		const selector = sample.dimensions?.['process_selector'];
		if (!selector) continue;
		const bucket = buckets.get(selector) ?? {};
		switch (sample.metric_id) {
			case 'vm.guest.process.count':
				bucket.count = sample;
				break;
			case 'vm.guest.process.cpu_utilization_ratio':
				bucket.cpu = sample;
				break;
			case 'vm.guest.process.rss_bytes':
				bucket.rss = sample;
				break;
		}
		buckets.set(selector, bucket);
	}
	const rows: GuestProcessRow[] = [];
	for (const [selector, bucket] of buckets) {
		rows.push({
			selector,
			count: sampleNumber(bucket.count),
			cpuRatio: sampleNumber(bucket.cpu),
			rssBytes: sampleNumber(bucket.rss),
			stale: [bucket.count, bucket.cpu, bucket.rss].some((s) => s?.stale === true)
		});
	}
	rows.sort((a, b) => a.selector.localeCompare(b.selector));
	return rows;
}

// ---------------------------------------------------------------------------
// Check ordering
// ---------------------------------------------------------------------------

const CHECK_KIND_ORDER: Record<string, number> = {
	service: 0,
	http: 1,
	tcp: 2,
	plugin: 3
};

/** Services first, then http/tcp, then plugins, then by check_id. */
export function sortChecks(checks: MonitoringCheck[]): MonitoringCheck[] {
	return [...checks].sort((a, b) => {
		const rank =
			(CHECK_KIND_ORDER[parseCheckId(a.check_id).kind] ?? 99) -
			(CHECK_KIND_ORDER[parseCheckId(b.check_id).kind] ?? 99);
		if (rank !== 0) return rank;
		return a.check_id.localeCompare(b.check_id);
	});
}
