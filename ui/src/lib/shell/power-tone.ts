/**
 * Map a VM power state to its list-view status tone.
 *
 * Case-insensitive, mirroring the detail view's normalizeTone: the
 * BFF reports capitalized power states ('Running', 'Failed', ...).
 */
export function mapPowerTone(state: string): string {
	switch (state.toLowerCase()) {
		case 'running': return 'healthy';
		case 'stopped': return 'neutral';
		case 'paused': return 'warning';
		case 'crashed':
		case 'failed': return 'failed';
		default: return 'neutral';
	}
}
