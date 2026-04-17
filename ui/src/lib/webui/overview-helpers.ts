import type { ShellTone } from '$lib/shell/app-shell';

export function severityTone(severity: string): ShellTone {
	switch (severity) {
		case 'critical':
			return 'failed';
		case 'warning':
			return 'warning';
		default:
			return 'unknown';
	}
}

export function statusTone(status: string): ShellTone {
	switch (status) {
		case 'running':
			return 'warning';
		case 'failed':
			return 'failed';
		case 'succeeded':
			return 'healthy';
		default:
			return 'unknown';
	}
}

export function formatTimeAgo(ms: number): string {
	const seconds = Math.max(Math.round((Date.now() - ms) / 1000), 0);
	if (seconds < 60) return `${seconds}s ago`;
	const minutes = Math.round(seconds / 60);
	if (minutes < 60) return `${minutes}m ago`;
	const hours = Math.round(minutes / 60);
	if (hours < 24) return `${hours}h ago`;
	return `${Math.round(hours / 24)}d ago`;
}
