import { describe, expect, it } from 'vitest';

import { buildDeleteConfirmText, buildDeleteVolumePayload } from './volume-delete';

describe('volume-delete helpers (#522 DP12)', () => {
	describe('buildDeleteVolumePayload', () => {
		it('sends exactly the one-key contract', () => {
			const payload = buildDeleteVolumePayload('vol-abc');
			expect(payload).toEqual({ volume_id: 'vol-abc' });
			expect(Object.keys(payload)).toEqual(['volume_id']);
		});

		it('trims the id before sending', () => {
			expect(buildDeleteVolumePayload('  vol-abc  ')).toEqual({ volume_id: 'vol-abc' });
		});

		it('never expresses a force flag or kind override (DP5/DP6 — the guards are the server\'s story)', () => {
			const payload = buildDeleteVolumePayload('vol-abc') as Record<string, unknown>;
			expect(payload).not.toHaveProperty('force');
			expect(payload).not.toHaveProperty('kind');
			expect(payload).not.toHaveProperty('volume_kind');
		});
	});

	describe('buildDeleteConfirmText', () => {
		it('names the volume, its size, and the irreversibility', () => {
			expect(buildDeleteConfirmText('data-1', '10.0 GiB')).toBe(
				'Delete "data-1" (10.0 GiB)? The backing store on the node is destroyed; this cannot be undone.'
			);
		});

		it('trims the name and size before interpolating', () => {
			expect(buildDeleteConfirmText('  data-1  ', ' 5.0 GiB ')).toBe(
				'Delete "data-1" (5.0 GiB)? The backing store on the node is destroyed; this cannot be undone.'
			);
		});

		it('omits the size clause when the size is blank', () => {
			expect(buildDeleteConfirmText('data-1', '')).toBe(
				'Delete "data-1"? The backing store on the node is destroyed; this cannot be undone.'
			);
			expect(buildDeleteConfirmText('data-1', '   ')).not.toContain('(');
		});

		it('falls back to a generic subject when the name is blank', () => {
			expect(buildDeleteConfirmText('   ', '10.0 GiB')).toBe(
				'Delete this volume (10.0 GiB)? The backing store on the node is destroyed; this cannot be undone.'
			);
		});
	});
});
