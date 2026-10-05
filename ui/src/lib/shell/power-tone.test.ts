import { describe, expect, it } from 'vitest';
import { mapPowerTone } from '$lib/shell/power-tone';

describe('mapPowerTone', () => {
	it.each([
		['running', 'healthy'],
		['stopped', 'neutral'],
		['paused', 'warning'],
		['crashed', 'failed'],
		['failed', 'failed']
	])('maps %s to %s', (state, tone) => {
		expect(mapPowerTone(state)).toBe(tone);
	});

	it.each([
		['Running', 'healthy'],
		['RUNNING', 'healthy'],
		['Stopped', 'neutral'],
		['Paused', 'warning'],
		['Crashed', 'failed'],
		['Failed', 'failed'],
		['FAILED', 'failed']
	])('is case-insensitive: %s maps to %s', (state, tone) => {
		expect(mapPowerTone(state)).toBe(tone);
	});

	it.each([
		['Unknown'],
		['migrating'],
		['Pending'],
		[''],
		['not-a-state']
	])('falls back to neutral for unknown state %s', (state) => {
		expect(mapPowerTone(state)).toBe('neutral');
	});

	it('takes only the power state — last_error pass-through is not part of it', () => {
		// The BFF's Failed-wins render surfaces last_error separately from the
		// tone; this helper must stay a pure state→tone map.
		expect(mapPowerTone.length).toBe(1);
	});
});
