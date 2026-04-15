import type { ShellTone } from '$lib/shell/app-shell';

export function normalizeTone(status: string): ShellTone {
	const s = status.trim().toLowerCase();
	if (['healthy', 'online', 'ready', 'up', 'nominal', 'running'].includes(s)) return 'healthy';
	if (['warning', 'pending', 'maintenance', 'attention'].includes(s)) return 'warning';
	if (['degraded', 'offline', 'busy', 'transitional'].includes(s)) return 'degraded';
	if (['failed', 'error', 'down', 'critical'].includes(s)) return 'failed';
	return 'unknown';
}

export function formatDateTimeLabel(ts: number): string {
	return new Intl.DateTimeFormat('en-US', {
		month: 'short',
		day: 'numeric',
		hour: 'numeric',
		minute: '2-digit'
	}).format(new Date(ts));
}

export function formatDurationLabel(startedMs: number, now = Date.now()): string {
	const elapsed = Math.max(Math.round((now - startedMs) / 1000), 0);
	if (elapsed < 60) return `${elapsed}s`;
	if (elapsed < 3600) return `${Math.round(elapsed / 60)}m`;
	if (elapsed < 86400) return `${Math.round(elapsed / 3600)}h`;
	return `${Math.round(elapsed / 86400)}d`;
}

export function titleize(value: string): string {
	return value
		.replace(/[_-]+/g, ' ')
		.replace(/\s+/g, ' ')
		.trim()
		.replace(/\b\w/g, (l) => l.toUpperCase());
}
